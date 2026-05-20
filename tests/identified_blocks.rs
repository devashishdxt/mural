use brisk::{
    Line, Render, Size, Terminal, TerminalError, Text,
    backend::fake::{FakeBackend, Operation},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct CounterBlock {
    label: &'static str,
    value: usize,
}

impl CounterBlock {
    fn new(label: &'static str, value: usize) -> Self {
        Self { label, value }
    }
}

impl Render for CounterBlock {
    fn render(&self, _width: u16) -> Text {
        Text::from_plain(format!("{}={}", self.label, self.value)).unwrap()
    }
}

#[test]
fn identified_live_and_pinned_blocks_reject_duplicate_ids_globally() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();

    terminal
        .insert_live("transcript", Text::from_plain("hello").unwrap())
        .unwrap();
    terminal
        .insert_pinned("status", Text::from_plain("ready").unwrap())
        .unwrap();

    assert_eq!(
        terminal.insert_live("status", Text::from_plain("duplicate").unwrap()),
        Err(TerminalError::DuplicateBlockId {
            id: "status".into()
        })
    );
    assert_eq!(
        terminal.insert_pinned("transcript", Text::from_plain("duplicate").unwrap()),
        Err(TerminalError::DuplicateBlockId {
            id: "transcript".into()
        })
    );

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("hello").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("ready").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn identified_blocks_report_specific_typed_api_errors() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", Text::from_plain("hello").unwrap())
        .unwrap();

    assert_eq!(
        terminal.live_block_mut::<Text>("missing"),
        Err(TerminalError::MissingBlockId {
            id: "missing".into()
        })
    );
    assert_eq!(
        terminal.pinned_block_mut::<Text>("transcript"),
        Err(TerminalError::ExpectedPinnedBlock {
            id: "transcript".into(),
        })
    );
    assert_eq!(
        terminal
            .live_block_mut::<CounterBlock>("transcript")
            .unwrap_err(),
        TerminalError::WrongBlockType {
            id: "transcript".into(),
            expected: std::any::type_name::<CounterBlock>(),
            actual: std::any::type_name::<Text>(),
        }
    );

    terminal.finish().unwrap();

    assert_eq!(
        terminal.insert_live("late", Text::from_plain("late").unwrap()),
        Err(TerminalError::Finished)
    );
    assert_eq!(
        terminal.live_block_mut::<Text>("transcript"),
        Err(TerminalError::Finished)
    );
}

#[test]
fn removing_identified_blocks_reports_specific_typed_api_errors() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", Text::from_plain("hello").unwrap())
        .unwrap();
    terminal
        .insert_pinned("status", Text::from_plain("ready").unwrap())
        .unwrap();

    assert_eq!(
        terminal.remove_live("missing"),
        Err(TerminalError::MissingBlockId {
            id: "missing".into()
        })
    );
    assert_eq!(
        terminal.remove_pinned("transcript"),
        Err(TerminalError::ExpectedPinnedBlock {
            id: "transcript".into(),
        })
    );
    assert_eq!(
        terminal.remove_live("status"),
        Err(TerminalError::ExpectedLiveBlock {
            id: "status".into(),
        })
    );

    terminal.finish().unwrap();

    assert_eq!(
        terminal.remove_live("transcript"),
        Err(TerminalError::Finished)
    );
}

#[test]
fn heterogeneous_identified_blocks_do_not_require_send_or_sync() {
    #[derive(Debug)]
    struct LocalOnlyBlock {
        value: Rc<RefCell<String>>,
    }

    impl Render for LocalOnlyBlock {
        fn render(&self, _width: u16) -> Text {
            Text::from_plain(self.value.borrow().as_str()).unwrap()
        }
    }

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    let value = Rc::new(RefCell::new(String::from("local")));
    terminal
        .insert_live(
            "local",
            LocalOnlyBlock {
                value: Rc::clone(&value),
            },
        )
        .unwrap();

    terminal
        .live_block_mut::<LocalOnlyBlock>("local")
        .unwrap()
        .value
        .replace(String::from("changed"));
    terminal.render().unwrap();

    assert!(
        terminal
            .backend()
            .operations()
            .contains(&Operation::Print(Line::from_plain("changed").unwrap()))
    );
}

#[test]
fn mutable_access_marks_the_block_dirty_as_a_rendering_hint() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", CounterBlock::new("live", 1))
        .unwrap();

    assert_eq!(terminal.is_block_dirty("transcript"), Ok(true));
    terminal.render().unwrap();
    assert_eq!(terminal.is_block_dirty("transcript"), Ok(false));

    terminal
        .live_block_mut::<CounterBlock>("transcript")
        .unwrap()
        .value = 2;

    assert_eq!(terminal.is_block_dirty("transcript"), Ok(true));
}

#[test]
fn removed_identified_live_blocks_disappear_on_next_render() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", Text::from_plain("transcript").unwrap())
        .unwrap();
    terminal
        .insert_pinned("status", Text::from_plain("status").unwrap())
        .unwrap();

    terminal.render().unwrap();
    terminal.remove_live("transcript").unwrap();
    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("transcript").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("status").unwrap()),
            Operation::Flush,
            Operation::Clear,
            Operation::Print(Line::from_plain("status").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn removed_identified_pinned_blocks_disappear_on_next_render() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", Text::from_plain("transcript").unwrap())
        .unwrap();
    terminal
        .insert_pinned("status", Text::from_plain("status").unwrap())
        .unwrap();

    terminal.render().unwrap();
    terminal.remove_pinned("status").unwrap();
    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("transcript").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("status").unwrap()),
            Operation::Flush,
            Operation::Clear,
            Operation::Print(Line::from_plain("transcript").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn identified_blocks_can_be_retrieved_and_mutated_as_concrete_types() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transcript", CounterBlock::new("live", 1))
        .unwrap();
    terminal
        .insert_pinned("status", CounterBlock::new("pinned", 2))
        .unwrap();

    terminal
        .live_block_mut::<CounterBlock>("transcript")
        .unwrap()
        .value = 10;
    terminal
        .pinned_block_mut::<CounterBlock>("status")
        .unwrap()
        .value = 20;

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("live=10").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("pinned=20").unwrap()),
            Operation::Flush,
        ]
    );
}
