use brisk::{
    Render, Size, StdoutBackend, Terminal, TextBlock,
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
    terminal.append_live(TextBlock::new("hello")).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print("hello".to_owned()),
            Operation::Flush,
        ]
    );
}

#[test]
fn text_blocks_render_at_safe_width_with_separators_only_between_lines() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal
        .append_live(TextBlock::new("helloworld\n!"))
        .unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print("hello".to_owned()),
            Operation::Newline,
            Operation::Print("world".to_owned()),
            Operation::Newline,
            Operation::Print("!".to_owned()),
            Operation::Flush,
        ]
    );
}

#[test]
fn render_trait_receives_the_safe_printable_width() {
    struct WidthEcho;

    impl Render for WidthEcho {
        fn render(&self, width: u16) -> Vec<String> {
            vec![format!("width={width}")]
        }
    }

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(10, 24))).unwrap();
    terminal.append_live(WidthEcho).unwrap();

    terminal.render().unwrap();

    assert!(
        terminal
            .backend()
            .operations()
            .contains(&Operation::Print("width=9".to_owned()))
    );
}

#[test]
fn stdout_constructor_is_available_for_the_common_case_without_opening_it_in_tests() {
    let _constructor: fn() -> std::io::Result<Terminal<StdoutBackend<std::io::Stdout>>> =
        Terminal::stdout;
}
