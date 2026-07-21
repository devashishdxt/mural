use mural::widget::Textarea;
use unicode_segmentation::UnicodeSegmentation;

#[test]
fn every_value_entry_point_applies_the_same_sanitization() {
    let unsafe_text = "\u{1b}[31ma\r\n\0界\t\u{1b}[0m";
    let expected = "a\n界\t";

    assert_eq!(Textarea::from(unsafe_text).value(), expected);
    assert_eq!(Textarea::from(unsafe_text.to_owned()).value(), expected);

    let mut replaced = Textarea::new();
    replaced.set_value(unsafe_text);
    assert_eq!(replaced.value(), expected);

    let mut inserted = Textarea::from("prefix");
    inserted
        .set_cursor(inserted.value().len())
        .insert(unsafe_text);
    assert_eq!(inserted.value(), format!("prefix{expected}"));

    let mut characters = Textarea::new();
    characters
        .insert_char('\u{1b}')
        .insert_char('\r')
        .insert_char('界')
        .insert_char('\t');
    assert_eq!(characters.value(), "\n界\t");
}

#[test]
fn value_operations_are_immutable_chainable_and_reset_the_cursor() {
    let mut textarea = Textarea::from("first");
    let inspected: &str = textarea.value();
    assert_eq!(inspected, "first");
    assert!(!textarea.is_empty());

    textarea
        .set_cursor(usize::MAX)
        .set_value("second\rline")
        .insert("!");
    assert_eq!(textarea.value(), "!second\nline");
    assert_eq!(textarea.cursor(), 1);

    textarea.clear().insert("new");
    assert_eq!(textarea.value(), "new");
    assert_eq!(textarea.cursor(), 3);

    assert_eq!(textarea.take(), "new");
    assert!(textarea.is_empty());
    assert_eq!(textarea.cursor(), 0);
}

#[test]
fn cursor_setting_clamps_to_grapheme_boundaries() {
    let text = "ae\u{301}👩\u{200d}💻z";
    let mut textarea = Textarea::from(text);

    textarea.set_cursor(usize::MAX);
    assert_eq!(textarea.cursor(), text.len());

    textarea.set_cursor(2);
    assert_eq!(textarea.cursor(), 1);

    let emoji_start = text.find('👩').unwrap();
    textarea.set_cursor(emoji_start + 1);
    assert_eq!(textarea.cursor(), emoji_start);

    assert_boundary(&textarea);
}

#[test]
fn editing_primitives_update_value_and_cursor_without_breaking_graphemes() {
    let mut textarea = Textarea::from("ab");
    textarea
        .set_cursor(1)
        .insert("é")
        .insert_char('界')
        .insert_newline();

    assert_eq!(textarea.value(), "aé界\nb");
    assert_eq!(textarea.cursor(), "aé界\n".len());
    assert_boundary(&textarea);

    textarea.backspace().backspace();
    assert_eq!(textarea.value(), "aéb");
    assert_eq!(textarea.cursor(), "aé".len());

    textarea.set_cursor(1).delete();
    assert_eq!(textarea.value(), "ab");
    assert_eq!(textarea.cursor(), 1);
}

#[test]
fn deletion_removes_complete_clusters_and_is_stable_at_boundaries() {
    for grapheme in ["e\u{301}", "👩\u{200d}💻"] {
        let value = format!("a{grapheme}b");
        let cluster_end = 1 + grapheme.len();

        let mut backward = Textarea::from(value.clone());
        backward.set_cursor(cluster_end).backspace();
        assert_eq!(backward.value(), "ab");
        assert_eq!(backward.cursor(), 1);

        let mut forward = Textarea::from(value);
        forward.set_cursor(1).delete();
        assert_eq!(forward.value(), "ab");
        assert_eq!(forward.cursor(), 1);
    }

    let mut edges = Textarea::from("é");
    edges.backspace();
    assert_eq!(edges.value(), "é");
    edges.set_cursor(usize::MAX).delete();
    assert_eq!(edges.value(), "é");
}

#[test]
fn required_standard_traits_include_all_current_behavioral_state() {
    fn assert_traits<T: std::fmt::Debug + Clone + Default + PartialEq + Eq>() {}
    assert_traits::<Textarea>();

    let original = Textarea::from("value");
    assert_eq!(original, original.clone());
    assert_ne!(original, Textarea::new());

    let mut different_cursor = original.clone();
    different_cursor.set_cursor(usize::MAX);
    assert_ne!(original, different_cursor);
    assert_eq!(Textarea::default(), Textarea::new());
}

fn assert_boundary(textarea: &Textarea) {
    let cursor = textarea.cursor();
    let value = textarea.value();
    assert!(cursor <= value.len());
    assert!(
        cursor == value.len()
            || value
                .grapheme_indices(true)
                .any(|(boundary, _)| boundary == cursor)
    );
}
