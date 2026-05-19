use brisk::{Color, Line, Modifiers, Span, Style, Text, TextError};

#[test]
fn ansi_text_construction_converts_supported_sgr_styles() {
    let text = Text::from_ansi(
        "plain \x1b[31;1mred bold\x1b[0m \x1b[48;5;22mgreen bg\x1b[38;2;1;2;3;3m rgb italic\x1b[39;49;22;23m plain",
    )
    .unwrap();

    assert_eq!(text.lines().len(), 1);
    assert_eq!(
        text.lines()[0].spans(),
        &[
            Span::new("plain ", Style::new()).unwrap(),
            Span::new("red bold", Style::new().fg(Color::Red).bold()).unwrap(),
            Span::new(" ", Style::new()).unwrap(),
            Span::new("green bg", Style::new().bg(Color::Indexed(22))).unwrap(),
            Span::new(
                " rgb italic",
                Style::new()
                    .fg(Color::Rgb(1, 2, 3))
                    .bg(Color::Indexed(22))
                    .italic(),
            )
            .unwrap(),
            Span::new(" plain", Style::new()).unwrap(),
        ]
    );

    assert!(
        text.lines()[0].spans()[1]
            .style()
            .modifiers()
            .contains(Modifiers::BOLD)
    );
}

#[test]
fn ansi_text_construction_supports_bright_colors_and_modifiers() {
    let text = Text::from_ansi("\x1b[96;45;2;4;7mdim under reverse\x1b[22;24;27m colors").unwrap();

    assert_eq!(
        text.lines()[0].spans(),
        &[
            Span::new(
                "dim under reverse",
                Style::new()
                    .fg(Color::BrightCyan)
                    .bg(Color::Magenta)
                    .dim()
                    .underline()
                    .reversed(),
            )
            .unwrap(),
            Span::new(
                " colors",
                Style::new().fg(Color::BrightCyan).bg(Color::Magenta)
            )
            .unwrap(),
        ]
    );
}

#[test]
fn raw_text_construction_strips_ansi_and_controls_without_styling() {
    let text = Text::from_raw(
        "hi\t\x1b[31mred\x1b[0m\x07\x1b]8;;https://example.com\x07link\x1b]8;;\x07\r\nnext",
    )
    .unwrap();

    assert_eq!(text.lines().len(), 2);
    assert_eq!(
        text.lines()[0].spans(),
        &[Span::plain("hi    redlink").unwrap()]
    );
    assert_eq!(text.lines()[1].spans(), &[Span::plain("next").unwrap()]);
}

#[test]
fn unsupported_controls_are_stripped_from_ansi_text() {
    let text = Text::from_ansi(
        "start\x1b[2J\x1b[10;20H\u{9b}31m\x1bPignored\x1b\\\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\u{202e}\u{200b}\x07end",
    )
    .unwrap();

    assert_eq!(text.lines().len(), 1);
    assert_eq!(
        text.lines()[0].spans(),
        &[Span::plain("startlinkend").unwrap()]
    );
}

#[test]
fn ansi_text_preserves_style_across_newline_splits() {
    let text = Text::from_ansi("\x1b[31mred\nred too\x1b[0m").unwrap();

    assert_eq!(
        text.lines()[0].spans(),
        &[Span::new("red", Style::new().fg(Color::Red)).unwrap()]
    );
    assert_eq!(
        text.lines()[1].spans(),
        &[Span::new("red too", Style::new().fg(Color::Red)).unwrap()]
    );
}

#[test]
fn tabs_and_newline_combinations_normalize_at_text_level() {
    let text = Text::from_ansi("a\tb\rc\n\rd\r\ne").unwrap();

    assert_eq!(text.lines().len(), 4);
    assert_eq!(text.lines()[0].spans(), &[Span::plain("a    b").unwrap()]);
    assert_eq!(text.lines()[1].spans(), &[Span::plain("c").unwrap()]);
    assert_eq!(text.lines()[2].spans(), &[Span::plain("d").unwrap()]);
    assert_eq!(text.lines()[3].spans(), &[Span::plain("e").unwrap()]);
}

#[test]
fn line_and_span_ansi_constructors_enforce_their_invariants() {
    assert_eq!(
        Line::from_ansi("one \x1b[34mblue\x1b[0m").unwrap().spans(),
        &[
            Span::plain("one ").unwrap(),
            Span::new("blue", Style::new().fg(Color::Blue)).unwrap(),
        ]
    );
    assert_eq!(
        Span::from_ansi("\x1b[4munderlined\x1b[0m").unwrap(),
        Span::new("underlined", Style::new().underline()).unwrap()
    );

    assert_eq!(
        Line::from_ansi("one\ntwo").unwrap_err(),
        TextError::MultipleLines
    );
    assert_eq!(
        Span::from_ansi("one\ntwo").unwrap_err(),
        TextError::MultipleLines
    );
    assert_eq!(
        Span::from_ansi("plain \x1b[31mred").unwrap_err(),
        TextError::MultipleStyles
    );
}
