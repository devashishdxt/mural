use brisk::{
    Line, Render, Size, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

struct MutableText {
    content: String,
}

impl MutableText {
    fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl Render for MutableText {
    fn render(&self, _width: u16) -> Text {
        Text::from_plain(&self.content).unwrap()
    }
}

#[test]
fn forced_full_redraw_rewrites_the_entire_current_buffer_with_recovery() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(Text::from_plain("live").unwrap())
        .unwrap();
    terminal
        .push_pinned(Text::from_plain("status").unwrap())
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal.force_full_redraw().unwrap();
    terminal.render().unwrap();

    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("live").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("status").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn changing_a_line_above_the_viewport_forces_full_rewrite_with_scrollback_purge() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 2))).unwrap();
    terminal
        .insert_live("top", MutableText::new("one"))
        .unwrap();
    terminal
        .push_live(Text::from_plain("two").unwrap())
        .unwrap();
    terminal
        .push_live(Text::from_plain("three").unwrap())
        .unwrap();
    terminal
        .push_live(Text::from_plain("four").unwrap())
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal
        .live_block_mut::<MutableText>("top")
        .unwrap()
        .content = "changed".into();
    terminal.render().unwrap();

    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("changed").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("two").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("three").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("four").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn removal_uses_partial_redraw_when_changed_line_is_still_in_viewport() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("transient", Text::from_plain("remove me").unwrap())
        .unwrap();
    terminal
        .push_live(Text::from_plain("keep me").unwrap())
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal.remove_live("transient").unwrap();
    terminal.render().unwrap();

    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveUp(1),
            Operation::MoveToColumn(0),
            Operation::ClearFromCursorDown,
            Operation::Print(Line::from_plain("keep me").unwrap()),
            Operation::Flush,
        ]
    );
}
