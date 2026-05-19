use brisk::{
    Color, Line, Modifiers, Size, Span, Style, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

#[test]
fn style_supports_colors_and_text_modifiers() {
    let style = Style::new()
        .fg(Color::Red)
        .bg(Color::Rgb(1, 2, 3))
        .bold()
        .dim()
        .italic()
        .underline()
        .reversed();

    assert_eq!(style.foreground(), Some(Color::Red));
    assert_eq!(style.background(), Some(Color::Rgb(1, 2, 3)));
    assert!(style.modifiers().contains(Modifiers::BOLD));
    assert!(style.modifiers().contains(Modifiers::DIM));
    assert!(style.modifiers().contains(Modifiers::ITALIC));
    assert!(style.modifiers().contains(Modifiers::UNDERLINE));
    assert!(style.modifiers().contains(Modifiers::REVERSED));
}

#[test]
fn line_and_span_constructors_reject_structural_content() {
    assert!(Span::new("hello", Style::new()).is_ok());
    assert!(Line::from_plain("hello").is_ok());

    assert!(Span::new("hello\nworld", Style::new()).is_err());
    assert!(Span::new("hello\tworld", Style::new()).is_err());
    assert!(Line::from_plain("hello\nworld").is_err());

    let unchecked_span = unsafe { Span::new_unchecked("hello\nworld", Style::new()) };
    assert_eq!(unchecked_span.content(), "hello\nworld");

    let unchecked_line = unsafe { Line::new_unchecked(vec![unchecked_span.clone()]) };
    assert_eq!(unchecked_line.spans(), &[unchecked_span]);
}

#[test]
fn text_represents_multiple_and_empty_lines_and_is_renderable() {
    let text = Text::from_plain("hello\n\nworld").unwrap();

    assert_eq!(text.lines().len(), 3);
    assert_eq!(text.lines()[0].plain_content(), "hello");
    assert!(text.lines()[1].spans().is_empty());
    assert_eq!(text.lines()[2].plain_content(), "world");
    assert!(Text::empty().lines().is_empty());

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal.append_live(text).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("hello").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("world").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn backend_observes_style_only_differences() {
    let plain_line = Line::from_spans(vec![Span::new("same", Style::new()).unwrap()]);
    let styled_line = Line::from_spans(vec![
        Span::new("same", Style::new().fg(Color::Green).underline()).unwrap(),
    ]);
    let text = Text::from_lines(vec![plain_line.clone(), styled_line.clone()]);

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal.append_live(text).unwrap();

    terminal.render().unwrap();

    assert_ne!(plain_line, styled_line);
    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(plain_line),
            Operation::Newline,
            Operation::Print(styled_line),
            Operation::Flush,
        ]
    );
}

#[test]
fn textwrap_wrapping_preserves_span_styles() {
    let red = Style::new().fg(Color::Red);
    let blue = Style::new().fg(Color::Blue);
    let text = Text::from_lines(vec![Line::from_spans(vec![
        Span::new("hello", red).unwrap(),
        Span::new("world", blue).unwrap(),
    ])]);

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal.append_live(text).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_spans(vec![Span::new("hello", red).unwrap()])),
            Operation::Newline,
            Operation::Print(Line::from_spans(vec![Span::new("world", blue).unwrap()])),
            Operation::Flush,
        ]
    );
}

#[test]
fn textwrap_wrapping_splits_a_span_without_losing_its_style() {
    let red = Style::new().fg(Color::Red).bold();
    let text = Text::from_lines(vec![Line::from_spans(vec![
        Span::new("helloworld", red).unwrap(),
    ])]);

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal.append_live(text).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_spans(vec![Span::new("hello", red).unwrap()])),
            Operation::Newline,
            Operation::Print(Line::from_spans(vec![Span::new("world", red).unwrap()])),
            Operation::Flush,
        ]
    );
}
