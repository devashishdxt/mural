use std::{cell::Cell, rc::Rc};

use brisk::{
    Line, Render, Size, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

#[derive(Debug)]
struct CountingBlock {
    content: String,
    render_count: Rc<Cell<usize>>,
}

impl CountingBlock {
    fn new(content: impl Into<String>, render_count: Rc<Cell<usize>>) -> Self {
        Self {
            content: content.into(),
            render_count,
        }
    }
}

impl Render for CountingBlock {
    fn render(&self, _width: u16) -> Text {
        self.render_count.set(self.render_count.get() + 1);
        Text::from_plain(&self.content).unwrap()
    }
}

#[test]
fn clean_blocks_use_their_cached_rendered_lines_on_unchanged_frames() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(CountingBlock::new("cached", Rc::clone(&render_count)))
        .unwrap();

    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 1);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[Operation::Flush]
    );
    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("cached").unwrap()),
            Operation::Flush,
            Operation::Flush,
        ]
    );
}

#[test]
fn dirty_blocks_that_render_the_same_lines_do_not_emit_terminal_output() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live(
            "status",
            CountingBlock::new("same", Rc::clone(&render_count)),
        )
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal.live_block_mut::<CountingBlock>("status").unwrap();
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 2);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[Operation::Flush]
    );
}

#[test]
fn changed_blocks_redraw_from_the_earliest_changed_visual_line() {
    let top_render_count = Rc::new(Cell::new(0));
    let bottom_render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live(
            "top",
            CountingBlock::new("top=1", Rc::clone(&top_render_count)),
        )
        .unwrap();
    terminal
        .insert_live(
            "bottom",
            CountingBlock::new("bottom", Rc::clone(&bottom_render_count)),
        )
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal
        .live_block_mut::<CountingBlock>("top")
        .unwrap()
        .content = "top=2".into();
    terminal.render().unwrap();

    assert_eq!(top_render_count.get(), 2);
    assert_eq!(bottom_render_count.get(), 1);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveUp(1),
            Operation::MoveToColumn(0),
            Operation::ClearFromCursorDown,
            Operation::Print(Line::from_plain("top=2").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("bottom").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn failed_flush_does_not_update_the_snapshot_or_clear_dirty_flags() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live(
            "status",
            CountingBlock::new("one", Rc::clone(&render_count)),
        )
        .unwrap();
    terminal.render().unwrap();

    terminal
        .live_block_mut::<CountingBlock>("status")
        .unwrap()
        .content = "two".into();
    terminal.backend_mut().fail_next_flush();
    assert!(terminal.render().is_err());
    assert_eq!(terminal.is_block_dirty("status"), Ok(true));
    let after_failed_render = terminal.backend().operations().len();

    terminal.render().unwrap();

    assert_eq!(render_count.get(), 3);
    assert_eq!(terminal.is_block_dirty("status"), Ok(false));
    assert_eq!(
        &terminal.backend().operations()[after_failed_render..],
        &[
            Operation::MoveToColumn(0),
            Operation::ClearFromCursorDown,
            Operation::Print(Line::from_plain("two").unwrap()),
            Operation::Flush,
        ]
    );
}
