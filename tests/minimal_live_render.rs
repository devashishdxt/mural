use brisk::{
    Line, Render, Size, StdoutBackend, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

#[test]
fn constructor_queries_size_and_hides_cursor_without_printing_or_clearing() {
    let terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[Operation::QuerySize, Operation::HideCursor]
    );
}

#[test]
fn manual_render_emits_live_text_without_clearing_and_flushes() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("hello").unwrap())
        .unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("hello").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn normal_render_prints_pinned_blocks_after_live_blocks() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("transcript").unwrap())
        .unwrap();
    terminal
        .append_pinned(Text::from_plain("status").unwrap())
        .unwrap();

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
        ]
    );
}

#[test]
fn finish_redraws_live_only_and_restores_cursor() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("transcript").unwrap())
        .unwrap();
    terminal
        .append_pinned(Text::from_plain("status").unwrap())
        .unwrap();
    terminal.render().unwrap();

    terminal.finish().unwrap();

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
            Operation::Newline,
            Operation::MoveToColumn(0),
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn finish_is_idempotent_without_extra_blank_lines() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("transcript").unwrap())
        .unwrap();

    terminal.finish().unwrap();
    terminal.finish().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Clear,
            Operation::Print(Line::from_plain("transcript").unwrap()),
            Operation::Newline,
            Operation::MoveToColumn(0),
            Operation::ShowCursor,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn finish_does_not_render_pinned_blocks() {
    struct PanicIfRendered;

    impl Render for PanicIfRendered {
        fn render(&self, _width: u16) -> Text {
            panic!("finish must not render pinned blocks")
        }
    }

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("transcript").unwrap())
        .unwrap();
    terminal.append_pinned(PanicIfRendered).unwrap();

    terminal.finish().unwrap();

    assert!(
        terminal
            .backend()
            .operations()
            .contains(&Operation::Print(Line::from_plain("transcript").unwrap()))
    );
}

#[test]
fn finished_terminal_rejects_more_rendering_and_mutation() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();

    terminal.finish().unwrap();

    assert!(terminal.render().is_err());
    assert!(
        terminal
            .append_live(Text::from_plain("late live").unwrap())
            .is_err()
    );
    assert!(
        terminal
            .append_pinned(Text::from_plain("late pinned").unwrap())
            .is_err()
    );
}

#[test]
fn text_renders_at_safe_width_with_separators_only_between_lines() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal
        .append_live(Text::from_plain("helloworld\n!").unwrap())
        .unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("hello").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("world").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("!").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn render_trait_receives_the_safe_printable_width() {
    struct WidthEcho;

    impl Render for WidthEcho {
        fn render(&self, width: u16) -> Text {
            Text::from_plain(format!("width={width}")).unwrap()
        }
    }

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(10, 24))).unwrap();
    terminal.append_live(WidthEcho).unwrap();

    terminal.render().unwrap();

    assert!(
        terminal
            .backend()
            .operations()
            .contains(&Operation::Print(Line::from_plain("width=9").unwrap()))
    );
}

#[test]
fn stdout_constructor_is_available_for_the_common_case_without_opening_it_in_tests() {
    let _constructor: fn() -> std::io::Result<Terminal<StdoutBackend<std::io::Stdout>>> =
        Terminal::stdout;
}
