use std::borrow::Cow;

use mural::{
    Color, Line, Render, Size, Span, Style, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

#[test]
fn display_measurement_uses_terminal_widths() {
    let line = Line::from_spans(vec![
        Span::new("a", Style::new()).unwrap(),
        Span::new("界", Style::new()).unwrap(),
        Span::new("e\u{301}", Style::new()).unwrap(),
        Span::new("👩‍🚀", Style::new()).unwrap(),
    ]);
    let text = Text::from_lines(vec![line.clone(), Line::from_plain("").unwrap()]);

    assert_eq!(line.spans()[0].display_width(), 1);
    assert_eq!(line.spans()[1].display_width(), 2);
    assert_eq!(line.spans()[2].display_width(), 1);
    assert_eq!(line.spans()[3].display_width(), 2);
    assert_eq!(line.display_width(), 6);
    assert_eq!(text.display_width(), 6);
    assert_eq!(text.display_height(), 2);
    assert_eq!(Text::empty().display_height(), 0);
}

#[test]
fn borrowed_wrap_preserves_styles_empty_lines_and_graphemes() {
    let red = Style::new().fg(Color::Red);
    let blue = Style::new().fg(Color::Blue);
    let text = Text::from_lines(vec![
        Line::from_spans(vec![
            Span::new("a界", red).unwrap(),
            Span::new("e\u{301}👩‍🚀z", blue).unwrap(),
        ]),
        Line::from_plain("").unwrap(),
    ]);

    let wrapped = text.wrap(4);

    assert!(matches!(wrapped, Cow::Owned(_)));
    assert_eq!(
        wrapped.lines(),
        &[
            Line::from_spans(vec![
                Span::new("a界", red).unwrap(),
                Span::new("e\u{301}", blue).unwrap(),
            ]),
            Line::from_spans(vec![Span::new("👩‍🚀z", blue).unwrap()]),
            Line::from_plain("").unwrap(),
        ]
    );
    assert!(wrapped.lines().iter().all(|line| line.display_width() <= 4));
}

#[test]
fn wrap_has_borrowed_fast_path_and_consuming_api() {
    let text = Text::from_plain("short\n").unwrap();

    assert!(matches!(text.wrap(80), Cow::Borrowed(_)));
    assert_eq!(text.clone().into_wrapped(80), text);
    assert_eq!(
        Text::from_plain("abcdef").unwrap().into_wrapped(3).lines(),
        &[
            Line::from_plain("abc").unwrap(),
            Line::from_plain("def").unwrap(),
        ]
    );
    assert_eq!(
        Text::from_plain("").unwrap().wrap(80).lines(),
        &[Line::from_plain("").unwrap()]
    );
    assert!(Text::empty().wrap(80).lines().is_empty());
    assert!(Text::from_plain("abc").unwrap().wrap(0).lines().is_empty());
}

#[test]
fn wrapping_uses_textwrap_break_points_for_whitespace_hyphens_and_slashes() {
    assert_eq!(
        Text::from_plain("a  b  c").unwrap().wrap(4).lines(),
        &[
            Line::from_plain("a  b").unwrap(),
            Line::from_plain("c").unwrap(),
        ]
    );
    assert_eq!(
        Text::from_raw_lossy("a\tb").unwrap().wrap(5).lines(),
        &[
            Line::from_plain("a").unwrap(),
            Line::from_plain("b").unwrap()
        ]
    );
    assert_eq!(
        Text::from_plain("hello-world").unwrap().wrap(6).lines(),
        &[
            Line::from_plain("hello-").unwrap(),
            Line::from_plain("world").unwrap(),
        ]
    );
    assert_eq!(
        Text::from_plain("hello/world").unwrap().wrap(6).lines(),
        &[
            Line::from_plain("hello/").unwrap(),
            Line::from_plain("world").unwrap(),
        ]
    );
}

#[test]
fn wrapping_breaks_long_words_without_losing_or_reordering_repeated_text() {
    let red = Style::new().fg(Color::Red);
    let blue = Style::new().fg(Color::Blue);
    let text = Text::from_lines(vec![Line::from_spans(vec![
        Span::new("abcabc", red).unwrap(),
        Span::new("abc", blue).unwrap(),
    ])]);

    assert_eq!(
        text.wrap(3).lines(),
        &[
            Line::from_spans(vec![Span::new("abc", red).unwrap()]),
            Line::from_spans(vec![Span::new("abc", red).unwrap()]),
            Line::from_spans(vec![Span::new("abc", blue).unwrap()]),
        ]
    );
}

#[test]
fn wrapping_long_styled_sentence_preserves_style_on_every_line() {
    let style = Style::new().fg(Color::Magenta).bold().underline();
    let text = Text::from_lines(vec![Line::from_spans(vec![
        Span::new(
            "This is a long styled sentence that should wrap across several terminal lines cleanly.",
            style,
        )
        .unwrap(),
    ])]);

    let wrapped = text.wrap(18);

    assert!(wrapped.lines().len() > 3);
    assert!(
        wrapped
            .lines()
            .iter()
            .all(|line| line.display_width() <= 18)
    );
    for line in wrapped.lines() {
        assert_eq!(line.spans().len(), 1);
        assert_eq!(line.spans()[0].style(), style);
    }
    assert_eq!(
        wrapped
            .lines()
            .iter()
            .map(Line::plain_content)
            .collect::<Vec<_>>(),
        vec![
            "This is a long",
            "styled sentence",
            "that should wrap",
            "across several",
            "terminal lines",
            "cleanly.",
        ]
    );
}

#[test]
fn wrapping_preserves_styles_when_breaking_inside_and_between_spans() {
    let red = Style::new().fg(Color::Red);
    let blue = Style::new().fg(Color::Blue);
    let green = Style::new().fg(Color::Green);
    let text = Text::from_lines(vec![Line::from_spans(vec![
        Span::new("ab", red).unwrap(),
        Span::new("cd", blue).unwrap(),
        Span::new("ef", green).unwrap(),
    ])]);

    assert_eq!(
        text.wrap(3).lines(),
        &[
            Line::from_spans(vec![
                Span::new("ab", red).unwrap(),
                Span::new("c", blue).unwrap(),
            ]),
            Line::from_spans(vec![
                Span::new("d", blue).unwrap(),
                Span::new("ef", green).unwrap(),
            ]),
        ]
    );
}

#[test]
fn wrapping_never_splits_combining_marks_cjk_or_zwj_emoji_clusters() {
    assert_eq!(
        Text::from_plain("e\u{301}e\u{301}👩‍🚀z")
            .unwrap()
            .wrap(2)
            .lines(),
        &[
            Line::from_plain("e\u{301}e\u{301}").unwrap(),
            Line::from_plain("👩‍🚀").unwrap(),
            Line::from_plain("z").unwrap(),
        ]
    );
    assert_eq!(
        Text::from_plain("界界界").unwrap().wrap(4).lines(),
        &[
            Line::from_plain("界界").unwrap(),
            Line::from_plain("界").unwrap(),
        ]
    );
}

#[test]
fn wrapping_respects_existing_line_boundaries_blank_lines_and_zero_width() {
    let text = Text::from_plain("abcd\n\nef").unwrap();

    assert_eq!(
        text.wrap(2).lines(),
        &[
            Line::from_plain("ab").unwrap(),
            Line::from_plain("cd").unwrap(),
            Line::from_plain("").unwrap(),
            Line::from_plain("ef").unwrap(),
        ]
    );
    assert!(text.wrap(0).lines().is_empty());
}

#[test]
fn terminal_defensively_wraps_blocks_that_ignore_the_requested_width() {
    struct IgnoresWidth;

    impl Render for IgnoresWidth {
        fn render(&self, _width: u16) -> Text {
            Text::from_plain("abcdef").unwrap()
        }
    }

    let mut terminal = Terminal::new(FakeBackend::new(Size::new(4, 24))).unwrap();
    terminal.push_live(IgnoresWidth).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("abc").unwrap()),
            Operation::Newline,
            Operation::Print(Line::from_plain("def").unwrap()),
            Operation::Flush,
        ]
    );
}
