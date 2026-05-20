use brisk::{
    Color, Hr, Line, Render, Size, Span, Style, Terminal, TextError,
    backend::fake::{FakeBackend, Operation},
    blocks, hr,
};

#[test]
fn hr_convenience_and_default_match_new() {
    assert_eq!(hr(), Hr::new());
    assert_eq!(Hr::default(), Hr::new());
    assert_eq!(blocks::hr(), Hr::new());
    assert_eq!(blocks::Hr::new(), Hr::new());
}

#[test]
fn default_hr_renders_a_full_width_plain_rule() {
    let text = Hr::new().render(5);

    assert_eq!(text.lines().len(), 1);
    assert_eq!(text.lines()[0], Line::from_plain("─────").unwrap());
}

#[test]
fn custom_character_renders_as_many_glyphs_as_fit() {
    let hr = Hr::with_character('界').unwrap();

    let text = hr.render(5);

    assert_eq!(hr.fill_character(), '界');
    assert_eq!(hr.character_width(), 2);
    assert_eq!(text.lines().len(), 1);
    assert_eq!(text.lines()[0].plain_content(), "界界");
    assert_eq!(text.lines()[0].display_width(), 4);
}

#[test]
fn character_wider_than_render_width_renders_one_blank_line() {
    let text = Hr::with_character('界').unwrap().render(1);

    assert_eq!(text.lines().len(), 1);
    assert!(text.lines()[0].spans().is_empty());
}

#[test]
fn zero_render_width_renders_no_lines() {
    let text = Hr::new().render(0);

    assert!(text.lines().is_empty());
}

#[test]
fn styled_rule_preserves_style() {
    let style = Style::new().fg(Color::BrightBlack).dim();
    let text = Hr::new().character('═').unwrap().style(style).render(3);

    assert_eq!(text.lines().len(), 1);
    assert_eq!(
        text.lines()[0],
        Line::from_spans(vec![Span::new("═══", style).unwrap()])
    );
    assert_eq!(Hr::new().style(style).rule_style(), style);
}

#[test]
fn invalid_characters_are_rejected_as_structural_content() {
    assert_eq!(
        Hr::with_character('\n').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\t').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\u{1b}').unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Hr::with_character('\u{0301}').unwrap_err(),
        TextError::StructuralContent
    );
}

#[test]
fn terminal_renders_hr_as_a_normal_block() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(6, 24))).unwrap();
    terminal.push_live(hr()).unwrap();

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("─────").unwrap()),
            Operation::Flush,
        ]
    );
}
