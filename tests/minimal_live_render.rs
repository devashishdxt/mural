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
