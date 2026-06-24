use mural::{
    Color, KeyCode, KeyEvent, KeyModifiers, KeyOutcome, Line, Render, Span, Style, TextError,
    Textarea, padding, textarea,
};

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
fn textarea_moves_word_left_to_start_of_current_word() {
    let mut input = Textarea::new();
    input.set_value("hello").set_cursor(2);

    input.move_word_left();

    assert_eq!(input.cursor(), 0);
}

#[test]
fn textarea_moves_word_right_to_end_of_current_word() {
    let mut input = Textarea::new();
    input.set_value("hello").set_cursor(2);

    input.move_word_right();

    assert_eq!(input.cursor(), 5);
}

#[test]
fn textarea_moves_word_left_skips_punctuation_and_whitespace() {
    let mut input = Textarea::new();
    input.set_value("hello,   world");

    input.move_word_left();

    assert_eq!(input.cursor(), 9);
    input.move_word_left();
    assert_eq!(input.cursor(), 0);
}

#[test]
fn textarea_moves_word_right_skips_punctuation_and_whitespace() {
    let mut input = Textarea::new();
    input.set_value("hello,   world").set_cursor(5);

    input.move_word_right();

    assert_eq!(input.cursor(), input.value().len());
}

#[test]
fn textarea_moves_words_across_source_lines() {
    let mut input = Textarea::new();
    input.set_value("hello\nworld").set_cursor(6);

    input.move_word_left();
    assert_eq!(input.cursor(), 0);

    input.set_cursor(5).move_word_right();
    assert_eq!(input.cursor(), input.value().len());
}

#[test]
fn textarea_moves_by_unicode_words() {
    let mut input = Textarea::new();
    input.set_value("can't jump 29.3 café");

    input.move_word_left();
    assert_eq!(input.cursor(), 16);
    input.move_word_left();
    assert_eq!(input.cursor(), 11);
    input.move_word_left();
    assert_eq!(input.cursor(), 6);
    input.move_word_left();
    assert_eq!(input.cursor(), 0);

    input.move_word_right();
    assert_eq!(input.cursor(), 5);
    input.move_word_right();
    assert_eq!(input.cursor(), 10);
    input.move_word_right();
    assert_eq!(input.cursor(), 15);
    input.move_word_right();
    assert_eq!(input.cursor(), input.value().len());
}

#[test]
fn textarea_word_movement_falls_back_to_buffer_edges_without_words() {
    let mut input = Textarea::new();
    input.set_value("!!!   ");

    input.move_word_left();
    assert_eq!(input.cursor(), 0);

    input.move_word_right();
    assert_eq!(input.cursor(), input.value().len());
}

#[test]
fn textarea_visual_navigation_follows_word_wrapped_rows() {
    let mut input = Textarea::new();
    input.set_value("hello world").set_cursor(2);

    input.move_visual_down_with_width(12);

    assert_eq!(input.cursor(), 8);
}

#[test]
fn textarea_word_movement_resets_preferred_visual_column() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);

    input.move_visual_down_with_width(5);
    input.move_word_right();
    input.move_visual_up_with_width(5);

    assert_eq!(input.cursor(), 8);
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
fn textarea_wraps_words_like_text_wrapping() {
    let mut input = Textarea::new();
    input.set_value("hello world");

    let rendered = input.render(12);

    assert_eq!(plain_lines(&rendered), vec!["› hello", "  world "]);
    assert!(
        rendered
            .lines()
            .iter()
            .all(|line| line.display_width() <= 12)
    );
}

#[test]
fn textarea_keeps_cursor_visible_on_hidden_wrap_whitespace() {
    let mut input = Textarea::new();
    input.set_value("hello world").set_cursor(5);

    let rendered = input.render(12);

    assert_eq!(plain_lines(&rendered), vec!["› hello ", "  world"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_wraps_placeholder_like_text_content() {
    let input = Textarea::new().placeholder("hello world").unwrap();

    let rendered = input.render(12);

    assert_eq!(plain_lines(&rendered), vec!["›  hello", "  world"]);
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
fn textarea_end_on_full_soft_wrapped_row_renders_cursor_on_final_grapheme() {
    let mut input = Textarea::new();
    input.set_value("abcdef").set_cursor(1);

    assert_eq!(
        input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 3);
    assert_eq!(
        input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5),
        KeyOutcome::Unchanged
    );
    assert_eq!(input.cursor(), 3);

    let rendered = input.render(5);

    assert_eq!(plain_lines(&rendered), vec!["› abc", "  def"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
    assert_eq!(
        rendered.lines()[1].spans().last().unwrap().style(),
        Style::new()
    );
}

#[test]
fn textarea_home_after_end_on_full_soft_wrapped_row_returns_to_same_visual_row_start() {
    let mut input = Textarea::new();
    input.set_value("abcdef").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);
    assert_eq!(input.cursor(), 3);

    assert_eq!(
        input.handle_key_event_with_width(KeyEvent::new(KeyCode::Home), 5),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 0);
}

#[test]
fn textarea_end_on_full_final_visual_row_renders_cursor_on_final_grapheme() {
    let mut input = Textarea::new();
    input.set_value("abc").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);

    let rendered = input.render(5);

    assert_eq!(plain_lines(&rendered), vec!["› abc"]);
    assert_eq!(input.cursor(), 3);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_public_line_end_on_full_rendered_row_renders_cursor_on_final_grapheme() {
    let mut input = Textarea::new();
    input.set_value("abc").set_cursor(1);
    input.render(5);

    input.move_to_line_end();

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(plain_lines(&rendered), vec!["› abc"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_public_buffer_end_on_full_rendered_row_renders_cursor_on_final_grapheme() {
    let mut input = Textarea::new();
    input.set_value("abc").set_cursor(1);
    input.render(5);

    input.move_to_buffer_end();

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(plain_lines(&rendered), vec!["› abc"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_control_e_on_full_final_visual_row_renders_cursor_on_final_grapheme() {
    let mut input = Textarea::new();
    input.set_value("abc").set_cursor(1);

    input.handle_key_event_with_width(
        KeyEvent::new(KeyCode::Char('e')).modifier(KeyModifiers::CONTROL),
        5,
    );

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(plain_lines(&rendered), vec!["› abc"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_end_on_row_with_hidden_wrap_whitespace_renders_cursor_space_on_previous_row() {
    let mut input = Textarea::new();
    input.set_value("ab cd").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(plain_lines(&rendered), vec!["› ab ", "  cd"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_vertical_movement_after_full_row_end_starts_from_rendered_row() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfgh").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);
    input.handle_key_event_with_width(KeyEvent::new(KeyCode::Down), 5);

    assert_eq!(input.cursor(), 5);
}

#[test]
fn textarea_up_to_full_shorter_row_renders_cursor_on_that_row_end() {
    let mut input = Textarea::new();
    input.set_value("abc\ndefghi").set_cursor(5);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);
    input.handle_key_event_with_width(KeyEvent::new(KeyCode::Up), 5);

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(plain_lines(&rendered), vec!["› abc", "  def", "  ghi"]);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_navigation_without_cursor_motion_resets_full_row_end_affinity() {
    let mut input = Textarea::new();
    input.set_value("abcdef").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);
    assert_eq!(
        input.handle_key_event_with_width(KeyEvent::new(KeyCode::Up), 5),
        KeyOutcome::Changed
    );

    let rendered = input.render(5);

    assert_eq!(input.cursor(), 3);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new()
    );
    assert_eq!(
        rendered.lines()[1].spans()[1].style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_editing_resets_full_row_end_affinity() {
    let mut input = Textarea::new();
    input.set_value("abcdef").set_cursor(1);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 5);
    input.handle_key_event_with_width(KeyEvent::new(KeyCode::Char('X')), 5);
    input.handle_key_event_with_width(KeyEvent::new(KeyCode::Backspace), 5);

    let rendered = input.render(5);

    assert_eq!(input.value(), "abcdef");
    assert_eq!(input.cursor(), 3);
    assert_eq!(
        rendered.lines()[0].spans().last().unwrap().style(),
        Style::new()
    );
    assert_eq!(
        rendered.lines()[1].spans()[1].style(),
        Style::new().reversed()
    );
}

#[test]
fn textarea_full_row_end_cursor_overlays_entire_final_tab() {
    let mut input = Textarea::new();
    input.set_value("\tabc").set_cursor(0);

    input.handle_key_event_with_width(KeyEvent::new(KeyCode::End), 6);

    assert_eq!(
        input.render(6).lines()[0],
        Line::from_spans(vec![
            Span::plain("›").unwrap(),
            Span::plain(" ").unwrap(),
            Span::new("    ", Style::new().reversed()).unwrap(),
        ])
    );
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
    input.move_to_visual_row_start_with_width(5);
    assert_eq!(input.cursor(), 3);
    input.move_to_visual_row_end_with_width(5);
    assert_eq!(input.cursor(), 5);
}

#[test]
fn textarea_moves_vertically_by_visual_rows_preserving_column() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij");
    input.set_cursor(4);

    input.move_visual_down_with_width(5);
    assert_eq!(input.cursor(), 7);
    input.move_visual_up_with_width(5);
    assert_eq!(input.cursor(), 4);
}

#[test]
fn textarea_visual_navigation_uses_width_learned_through_padding_render() {
    let mut padded = padding(Textarea::new()).left(1).right(1);
    padded.content_mut().set_value("abcde\nfghij").set_cursor(4);

    padded.render(7);

    padded.content_mut().move_visual_down();
    assert_eq!(padded.content().cursor(), 7);
}

#[test]
fn textarea_key_navigation_uses_last_rendered_width() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);
    input.render(5);

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Down)),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 7);
}

#[test]
fn textarea_keeps_navigation_width_when_value_reset_clears_scroll() {
    let mut input = Textarea::new();
    input.render(5);
    input.set_value("abcde\nfghij").set_cursor(4);

    input.move_visual_down();

    assert_eq!(input.cursor(), 7);
}

#[test]
fn textarea_navigation_before_first_render_uses_unwrapped_width() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);

    input.move_visual_down();

    assert_eq!(input.cursor(), 10);
}

#[test]
fn textarea_navigation_remembers_too_narrow_render_width() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);

    assert!(input.render(0).lines().is_empty());
    input.move_visual_down();

    assert_eq!(input.cursor(), 6);
}

#[test]
fn textarea_explicit_width_navigation_does_not_replace_rendered_width() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);
    input.render(5);

    input.move_visual_down_with_width(u16::MAX);
    assert_eq!(input.cursor(), 10);

    input.set_cursor(4).move_visual_down();
    assert_eq!(input.cursor(), 7);
}

#[test]
fn textarea_handles_basic_text_entry_and_submit_keys() {
    let mut input = Textarea::new();

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Char('a'))),
        KeyOutcome::Changed
    );
    assert_eq!(input.value(), "a");
    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Enter)),
        KeyOutcome::Submit
    );
    assert_eq!(input.value(), "a");

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Enter).modifier(KeyModifiers::ALT)),
        KeyOutcome::Changed
    );
    assert_eq!(input.value(), "a\n");
}

#[test]
fn textarea_handles_default_navigation_and_deletion_keys() {
    let mut input = Textarea::new();
    input.set_value("hello world");

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Left).modifier(KeyModifiers::CONTROL)),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 6);

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Backspace)),
        KeyOutcome::Changed
    );
    assert_eq!(input.value(), "helloworld");
    assert_eq!(input.cursor(), 5);

    input.move_to_buffer_start();
    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Left)),
        KeyOutcome::Unchanged
    );

    input.move_to_buffer_end();
    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Delete)),
        KeyOutcome::Unchanged
    );
}

#[test]
fn textarea_ignores_releases_unsupported_keys_and_command_modified_text() {
    let mut input = Textarea::new();

    assert_eq!(
        input.handle_key_event(
            KeyEvent::new(KeyCode::Char('x')).kind(mural::KeyEventKind::Release),
        ),
        KeyOutcome::Ignored
    );
    assert_eq!(input.value(), "");

    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Char('x')).modifier(KeyModifiers::ALT)),
        KeyOutcome::Ignored
    );
    assert_eq!(
        input.handle_key_event(KeyEvent::new(KeyCode::Esc)),
        KeyOutcome::Ignored
    );
    assert_eq!(input.value(), "");
}

#[test]
fn textarea_handles_line_and_visual_boundary_shortcuts() {
    let mut input = Textarea::new();
    input.set_value("abcde\nfghij").set_cursor(4);

    assert_eq!(
        input.handle_key_event_with_width(KeyEvent::new(KeyCode::Home), 5),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 3);

    assert_eq!(
        input.handle_key_event_with_width(
            KeyEvent::new(KeyCode::Char('e')).modifier(KeyModifiers::CONTROL),
            5,
        ),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 5);

    input.set_cursor(8);
    assert_eq!(
        input.handle_key_event_with_width(
            KeyEvent::new(KeyCode::Char('a')).modifier(KeyModifiers::CONTROL),
            5,
        ),
        KeyOutcome::Changed
    );
    assert_eq!(input.cursor(), 6);
}

fn plain_lines(text: &mural::Text) -> Vec<String> {
    text.lines().iter().map(Line::plain_content).collect()
}
