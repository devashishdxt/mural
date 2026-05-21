use brisk::{Color, Line, Render, Span, Style, TextError, Textarea, textarea};

#[test]
fn textarea_convenience_and_defaults_render_empty_input_with_prompt_and_cursor() {
    let input = Textarea::new();

    assert_eq!(textarea(), input);
    assert_eq!(input.value(), "");
    assert_eq!(input.cursor(), 0);
    assert_eq!(input.prompt_content(), "›");
    assert_eq!(input.gap_width(), 1);
    assert_eq!(input.max_height_value(), Some(6));

    let rendered = input.render(20);

    assert_eq!(rendered.lines().len(), 1);
    assert_eq!(
        rendered.lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::new(" ", Style::new().reversed()).unwrap(),
        ])
    );
}

#[test]
fn textarea_prompt_gap_and_style_are_configurable_and_validated() {
    let prompt_style = Style::new().fg(Color::BrightBlack).dim();
    let input = Textarea::new()
        .prompt("λ")
        .unwrap()
        .prompt_style(prompt_style)
        .gap(2);

    assert_eq!(input.prompt_content(), "λ");
    assert_eq!(input.prompt_style_value(), prompt_style);
    assert_eq!(input.gap_width(), 2);
    assert_eq!(
        input.render(20).lines()[0],
        Line::from_spans(vec![
            Span::new("λ", prompt_style).unwrap(),
            Span::plain("  ").unwrap(),
            Span::new(" ", Style::new().reversed()).unwrap(),
        ])
    );

    assert_eq!(
        Textarea::new().prompt("").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Textarea::new().prompt("\n").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Textarea::new().prompt("\t").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Textarea::new().prompt("\u{1b}").unwrap_err(),
        TextError::StructuralContent
    );
}

#[test]
fn textarea_sanitizes_values_while_preserving_newlines_and_tabs() {
    let mut input = Textarea::new();

    input.set_value("a\r\nb\rc\n\td\u{1b}[31m red\u{1b}[0m\u{7}");

    assert_eq!(input.value(), "a\nb\nc\n\td red");
    assert_eq!(input.cursor(), input.value().len());
    assert!(!input.is_empty());

    let taken = input.take();
    assert_eq!(taken, "a\nb\nc\n\td red");
    assert_eq!(input.value(), "");
    assert_eq!(input.cursor(), 0);
    assert!(input.is_empty());
}

#[test]
fn textarea_renders_cursor_on_existing_grapheme_or_end_cell() {
    let mut input = Textarea::new();
    input.set_value("abc").set_cursor(1);

    assert_eq!(input.cursor(), 1);
    assert_eq!(
        input.render(20).lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::plain("a").unwrap(),
            Span::new("b", Style::new().reversed()).unwrap(),
            Span::plain("c").unwrap(),
        ])
    );

    input.move_to_buffer_end();

    assert_eq!(
        input.render(20).lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::plain("abc").unwrap(),
            Span::new(" ", Style::new().reversed()).unwrap(),
        ])
    );
}

#[test]
fn textarea_edits_by_unicode_graphemes() {
    let mut input = Textarea::new();

    input.insert_str("a👨‍👩‍👧‍👦b");
    assert_eq!(input.value(), "a👨‍👩‍👧‍👦b");
    input.move_left();
    input.move_left();
    assert_eq!(input.cursor(), 1);

    input.delete();
    assert_eq!(input.value(), "ab");
    assert_eq!(input.cursor(), 1);

    input
        .insert_char('\t')
        .insert_char('\r')
        .insert_char('\u{7}');
    assert_eq!(input.value(), "a\t\nb");

    input.backspace();
    assert_eq!(input.value(), "a\tb");
    input.backspace();
    assert_eq!(input.value(), "ab");
    input.move_right().backspace();
    assert_eq!(input.value(), "a");
}

#[test]
fn textarea_renders_tab_as_four_spaces_and_cursor_styles_only_first_tab_cell() {
    let mut input = Textarea::new();
    input.set_value("a\tb").set_cursor(1);

    assert_eq!(
        input.render(20).lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::plain("a").unwrap(),
            Span::new(" ", Style::new().reversed()).unwrap(),
            Span::plain("   b").unwrap(),
        ])
    );
}

#[test]
fn textarea_renders_multiline_wrapped_content_with_hanging_prompt() {
    let mut input = Textarea::new();
    input.set_value("abcdef\n\nthird");

    let rendered = input.render(5);

    assert_eq!(rendered.lines().len(), 5);
    assert_eq!(rendered.lines()[0].plain_content(), "› abc");
    assert_eq!(rendered.lines()[1].plain_content(), "  def");
    assert_eq!(rendered.lines()[2].plain_content(), "  ");
    assert_eq!(rendered.lines()[3].plain_content(), "  thi");
    assert_eq!(rendered.lines()[4].plain_content(), "  rd ");
    assert!(
        rendered
            .lines()
            .iter()
            .all(|line| line.display_width() <= 5)
    );
}

#[test]
fn textarea_renders_placeholder_only_when_empty() {
    let placeholder_style = Style::new().fg(Color::BrightBlack).dim();
    let mut input = Textarea::new()
        .placeholder("type a message")
        .unwrap()
        .placeholder_style(placeholder_style);

    assert_eq!(input.placeholder_content(), Some("type a message"));
    assert_eq!(input.placeholder_style_value(), placeholder_style);
    assert_eq!(
        input.render(20).lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::new(" ", Style::new().reversed()).unwrap(),
            Span::new("type a message", placeholder_style).unwrap(),
        ])
    );

    input.insert_char('x');
    assert_eq!(input.render(20).lines()[0].plain_content(), "› x ");

    assert_eq!(
        Textarea::new().placeholder("has\nnewline").unwrap_err(),
        TextError::StructuralContent
    );
    assert_eq!(
        Textarea::new().placeholder("has\ttab").unwrap_err(),
        TextError::StructuralContent
    );
}

#[test]
fn textarea_limits_visible_height_with_sticky_cursor_scroll() {
    let mut input = Textarea::new().max_height(3);
    input.set_value("0\n1\n2\n3\n4");

    let bottom = input.render(20);
    assert_eq!(bottom.lines().len(), 3);
    assert_eq!(plain_lines(&bottom), vec!["  2", "  3", "  4 "]);

    input.set_cursor(4); // line 2, still inside the previous viewport.
    let still_bottom = input.render(20);
    assert_eq!(plain_lines(&still_bottom), vec!["  2", "  3", "  4"]);

    input.set_cursor(2); // line 1, above the previous viewport, so scroll up minimally.
    let scrolled_up = input.render(20);
    assert_eq!(plain_lines(&scrolled_up), vec!["  1", "  2", "  3"]);
}

#[test]
fn textarea_moves_to_source_line_and_visual_row_boundaries() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij");
    input.set_cursor(8);

    input.move_to_line_start();
    assert_eq!(input.cursor(), 6);
    input.move_to_line_end();
    assert_eq!(input.cursor(), input.value().len());

    input.set_cursor(4); // first source line, second visual row at width 5.
    input.move_to_visual_row_start(5);
    assert_eq!(input.cursor(), 3);
    input.move_to_visual_row_end(5);
    assert_eq!(input.cursor(), 5);
}

#[test]
fn textarea_moves_vertically_by_visual_rows_preserving_column() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij");
    input.set_cursor(4);

    input.move_visual_down(5);
    assert_eq!(input.cursor(), 7);
    input.move_visual_up(5);
    assert_eq!(input.cursor(), 4);
}

fn plain_lines(text: &brisk::Text) -> Vec<String> {
    text.lines().iter().map(Line::plain_content).collect()
}
