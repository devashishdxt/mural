use mural_core::widget::Textarea;

#[test]
fn horizontal_navigation_uses_graphemes_and_unicode_words() {
    let combining = "e\u{301}";
    let emoji = "👩\u{200d}💻";
    let value = format!("a{combining}{emoji}z");
    let mut textarea = Textarea::from(value.clone());

    textarea.move_right().move_right().move_right().move_left();
    assert_eq!(textarea.cursor(), 1 + combining.len());

    textarea.set_value("hé, 世界!").move_to_buffer_end();
    textarea.move_word_left();
    assert_eq!(textarea.cursor(), "hé, 世".len());
    textarea.move_word_left();
    assert_eq!(textarea.cursor(), "hé, ".len());
    textarea.move_word_left();
    assert_eq!(textarea.cursor(), 0);
    textarea.move_word_right();
    assert_eq!(textarea.cursor(), "hé".len());
    textarea.move_word_right();
    assert_eq!(textarea.cursor(), "hé, 世".len());
    textarea.move_word_right();
    assert_eq!(textarea.cursor(), "hé, 世界".len());
}

#[test]
fn source_buffer_and_visual_boundaries_are_distinct() {
    let mut wrapped_boundary = Textarea::from("abcd");
    wrapped_boundary.set_cursor(2).move_right_with_width(4);
    wrapped_boundary.move_to_visual_row_start_with_width(4);
    assert_eq!(wrapped_boundary.cursor(), 0);

    wrapped_boundary.set_cursor(2).move_right_with_width(4);
    wrapped_boundary.move_right_with_width(4);
    wrapped_boundary.move_to_visual_row_end_with_width(4);
    assert_eq!(wrapped_boundary.cursor(), 4);

    let mut textarea = Textarea::from("abcdef\nxy");
    textarea.set_cursor(4).move_to_line_start();
    assert_eq!(textarea.cursor(), 0);
    textarea.set_cursor(4).move_to_line_end_with_width(4);
    assert_eq!(textarea.cursor(), 6);

    textarea.move_to_buffer_start();
    assert_eq!(textarea.cursor(), 0);
    textarea.move_to_buffer_end_with_width(4);
    assert_eq!(textarea.cursor(), textarea.value().len());

    textarea.set_value("abcdef").set_cursor(4);
    textarea.move_to_visual_row_start_with_width(4);
    assert_eq!(textarea.cursor(), 3);
    textarea.move_to_visual_row_end_with_width(4);
    assert_eq!(textarea.cursor(), 6);
}

#[test]
fn vertical_navigation_preserves_columns_through_short_rows() {
    let mut textarea = Textarea::from("abcd\nx\nabcd");
    textarea.set_cursor(3).move_visual_down_with_width(10);
    assert_eq!(textarea.cursor(), "abcd\nx".len());

    textarea.move_visual_down_with_width(10);
    assert_eq!(textarea.cursor(), "abcd\nx\nabc".len());
}

#[test]
fn vertical_targets_respect_wide_cells_tabs_and_soft_wraps() {
    let mut wide = Textarea::from("abc\n界\nabc");
    wide.set_cursor(1).move_visual_down_with_width(10);
    assert_eq!(wide.cursor(), "abc\n".len());
    wide.move_visual_down_with_width(10);
    assert_eq!(wide.cursor(), "abc\n界\na".len());

    let mut tabbed = Textarea::from("abc\n\t\nabc");
    tabbed.set_cursor(3).move_visual_down_with_width(10);
    assert_eq!(tabbed.cursor(), "abc\n\t".len());
    tabbed.move_visual_down_with_width(10);
    assert_eq!(tabbed.cursor(), "abc\n\t\nabc".len());

    let mut wrapped = Textarea::from("abcdefghi");
    wrapped.set_cursor(2).move_visual_down_with_width(4);
    assert_eq!(wrapped.cursor(), 5);
    wrapped.move_visual_down_with_width(4);
    assert_eq!(wrapped.cursor(), 8);
}

#[test]
fn horizontal_editing_and_boundary_moves_reset_the_preferred_column() {
    let mut horizontal = Textarea::from("abcd\nx\nabcd");
    horizontal
        .set_cursor(3)
        .move_visual_down_with_width(10)
        .move_left_with_width(10)
        .move_visual_down_with_width(10);
    assert_eq!(horizontal.cursor(), "abcd\nx\n".len());

    let mut edited = Textarea::from("abcd\nx\nabcd");
    edited
        .set_cursor(3)
        .move_visual_down_with_width(10)
        .insert("!")
        .move_visual_down_with_width(10);
    assert_eq!(edited.cursor(), "abcd\nx!\nab".len());

    let mut boundary = Textarea::from("abcd\nx\nabcd");
    boundary
        .set_cursor(3)
        .move_visual_down_with_width(10)
        .move_to_line_start()
        .move_visual_down_with_width(10);
    assert_eq!(boundary.cursor(), "abcd\nx\n".len());
}

#[test]
fn widthless_navigation_is_unwrapped_before_a_width_is_remembered() {
    let mut textarea = Textarea::from("abcdef\nxy");
    textarea.set_cursor(2).move_visual_down();
    assert_eq!(textarea.cursor(), textarea.value().len());

    let mut no_newline = Textarea::from("abcdef");
    no_newline.set_cursor(2).move_visual_down();
    assert_eq!(no_newline.cursor(), 2);
}
