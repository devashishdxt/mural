use ansi_str::AnsiStr;
use unicode_segmentation::UnicodeSegmentation;

/// Removes terminal control sequences and normalizes externally supplied text.
pub(crate) fn sanitize(input: &str) -> String {
    let stripped = input.ansi_strip();
    let mut characters = stripped.chars().peekable();
    let mut sanitized = String::with_capacity(stripped.len());

    while let Some(character) = characters.next() {
        match character {
            '\r' | '\n' => {
                if characters
                    .peek()
                    .is_some_and(|next| matches!((character, *next), ('\r', '\n') | ('\n', '\r')))
                {
                    characters.next();
                }
                sanitized.push('\n');
            }
            '\t' => sanitized.push(character),
            _ if !character.is_control() => sanitized.push(character),
            _ => {}
        }
    }

    sanitized
}

/// Clamps a byte index to the closest preceding extended grapheme boundary.
pub(crate) fn clamp_cursor(text: &str, cursor: usize) -> usize {
    let cursor = cursor.min(text.len());
    if cursor == text.len() {
        return cursor;
    }

    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= cursor)
        .last()
        .unwrap_or(0)
}

/// Inserts sanitized text and returns the byte cursor after the insertion.
pub(crate) fn insert(buffer: &mut String, cursor: usize, input: &str) -> usize {
    let cursor = clamp_cursor(buffer, cursor);
    let input = sanitize(input);
    buffer.insert_str(cursor, &input);
    clamp_cursor(buffer, cursor + input.len())
}

/// Inserts a sanitized character and returns the byte cursor after it.
pub(crate) fn insert_character(buffer: &mut String, cursor: usize, character: char) -> usize {
    let mut encoded = [0; 4];
    insert(buffer, cursor, character.encode_utf8(&mut encoded))
}

/// Inserts one normalized line feed and returns the byte cursor after it.
pub(crate) fn insert_newline(buffer: &mut String, cursor: usize) -> usize {
    insert(buffer, cursor, "\n")
}

/// Removes the grapheme before the cursor and returns its former start.
pub(crate) fn backspace(buffer: &mut String, cursor: usize) -> usize {
    let cursor = clamp_cursor(buffer, cursor);
    let start = previous_grapheme(buffer, cursor);
    buffer.replace_range(start..cursor, "");
    clamp_cursor(buffer, start)
}

/// Removes the grapheme at the cursor and returns the unchanged cursor position.
pub(crate) fn delete_forward(buffer: &mut String, cursor: usize) -> usize {
    let cursor = clamp_cursor(buffer, cursor);
    let end = next_grapheme(buffer, cursor);
    buffer.replace_range(cursor..end, "");
    clamp_cursor(buffer, cursor)
}

/// Returns the preceding grapheme boundary, or zero at the buffer start.
pub(crate) fn previous_grapheme(text: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor(text, cursor);
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index < cursor)
        .last()
        .unwrap_or(0)
}

/// Returns the following grapheme boundary, or the buffer length at its end.
pub(crate) fn next_grapheme(text: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor(text, cursor);
    text[cursor..]
        .graphemes(true)
        .next()
        .map_or(text.len(), |grapheme| cursor + grapheme.len())
}

/// Returns the start of the current or preceding Unicode word.
pub(crate) fn previous_word(text: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor(text, cursor);
    text.unicode_word_indices()
        .map(|(start, _)| start)
        .take_while(|start| *start < cursor)
        .last()
        .unwrap_or(0)
}

/// Returns the end of the current or following Unicode word.
pub(crate) fn next_word(text: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor(text, cursor);
    text.unicode_word_indices()
        .map(|(start, word)| start + word.len())
        .find(|end| *end > cursor)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitization_strips_ansi_and_preserves_safe_unicode() {
        let cases = [
            ("", ""),
            ("plain café 世界", "plain café 世界"),
            ("\u{1b}[31mred\u{1b}[0m", "red"),
            ("left\tmiddle\tright", "left\tmiddle\tright"),
            ("a\0\u{8}\u{7f}\u{85}b", "ab"),
            ("e\u{301}", "e\u{301}"),
            ("👩\u{200d}💻", "👩\u{200d}💻"),
            ("a\u{200c}b", "a\u{200c}b"),
        ];

        for (input, expected) in cases {
            assert_eq!(sanitize(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn sanitization_normalizes_every_newline_form() {
        let cases = [
            ("a\r\nb", "a\nb"),
            ("a\n\rb", "a\nb"),
            ("a\rb", "a\nb"),
            ("a\nb", "a\nb"),
            ("a\r\r\n\nb", "a\n\n\nb"),
        ];

        for (input, expected) in cases {
            assert_eq!(sanitize(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn cursor_clamping_moves_backward_to_legal_boundaries() {
        let combining = "ae\u{301}z";
        let emoji = "a👩\u{200d}💻z";
        let cases = [
            ("", 8, 0),
            ("abc", 2, 2),
            ("abc", 99, 3),
            ("aéz", 2, 1),
            (combining, 2, 1),
            (combining, 3, 1),
            (emoji, 2, 1),
            (emoji, emoji.find('z').unwrap() - 1, 1),
            (emoji, emoji.len(), emoji.len()),
        ];

        for (text, cursor, expected) in cases {
            assert_eq!(clamp_cursor(text, cursor), expected, "text: {text:?}");
        }
    }

    #[test]
    fn grapheme_targets_treat_combining_and_emoji_sequences_as_units() {
        for grapheme in ["e\u{301}", "👩\u{200d}💻"] {
            let text = format!("a{grapheme}b");
            let start = 1;
            let end = start + grapheme.len();

            assert_eq!(next_grapheme(&text, start), end);
            assert_eq!(previous_grapheme(&text, end), start);
            assert_eq!(previous_grapheme(&text, start), 0);
            assert_eq!(next_grapheme(&text, end), text.len());
        }
    }

    #[test]
    fn insertion_sanitizes_content_and_advances_by_complete_graphemes() {
        let mut buffer = String::from("ab");
        let cursor = insert(&mut buffer, 1, "\u{1b}[31mé\r\nx\0\u{1b}[0m");
        assert_eq!(buffer, "aé\nxb");
        assert_eq!(cursor, "aé\nx".len());
        assert_boundary(&buffer, cursor);

        let cursor = insert_character(&mut buffer, cursor, '界');
        assert_eq!(buffer, "aé\nx界b");
        assert_boundary(&buffer, cursor);

        let cursor = insert_character(&mut buffer, cursor, '\u{7f}');
        assert_eq!(buffer, "aé\nx界b");
        assert_boundary(&buffer, cursor);

        let cursor = insert_newline(&mut buffer, usize::MAX);
        assert_eq!(buffer, "aé\nx界b\n");
        assert_eq!(cursor, buffer.len());
    }

    #[test]
    fn insertion_clamps_a_cursor_inside_a_grapheme() {
        let mut buffer = String::from("ae\u{301}b");
        let cursor = insert(&mut buffer, 2, "X");

        assert_eq!(buffer, "aXe\u{301}b");
        assert_eq!(cursor, 2);
    }

    #[test]
    fn insertion_retains_a_legal_cursor_when_graphemes_join() {
        let mut buffer = String::from("👩💻");
        let cursor = insert(&mut buffer, "👩".len(), "\u{200d}");

        assert_eq!(buffer, "👩\u{200d}💻");
        assert_eq!(cursor, 0);
        assert_boundary(&buffer, cursor);
    }

    #[test]
    fn backspace_and_delete_remove_whole_graphemes() {
        for grapheme in ["e\u{301}", "👩\u{200d}💻"] {
            let original = format!("a{grapheme}b");
            let end = 1 + grapheme.len();

            let mut backward = original.clone();
            let backward_cursor = backspace(&mut backward, end);
            assert_eq!(backward, "ab");
            assert_eq!(backward_cursor, 1);

            let mut forward = original;
            let forward_cursor = delete_forward(&mut forward, 1);
            assert_eq!(forward, "ab");
            assert_eq!(forward_cursor, 1);
        }
    }

    #[test]
    fn deletion_at_buffer_edges_is_unchanged() {
        let mut buffer = String::from("é");
        assert_eq!(backspace(&mut buffer, 0), 0);
        assert_eq!(buffer, "é");

        let end = buffer.len();
        assert_eq!(delete_forward(&mut buffer, end), end);
        assert_eq!(buffer, "é");
    }

    #[test]
    fn deletion_retains_a_legal_cursor_when_neighbors_join() {
        let source = "👩\u{200d}x💻";
        let after_joiner = "👩\u{200d}".len();
        let after_x = after_joiner + 1;

        let mut backward = source.to_owned();
        let cursor = backspace(&mut backward, after_x);
        assert_eq!(backward, "👩\u{200d}💻");
        assert_eq!(cursor, 0);
        assert_boundary(&backward, cursor);

        let mut forward = source.to_owned();
        let cursor = delete_forward(&mut forward, after_joiner);
        assert_eq!(forward, "👩\u{200d}💻");
        assert_eq!(cursor, 0);
        assert_boundary(&forward, cursor);
    }

    #[test]
    fn unicode_word_targets_skip_punctuation_and_whitespace() {
        let text = "élan,  κόσμος!";
        let first_end = text.find(',').unwrap();
        let second_start = text.find('κ').unwrap();
        let second_end = text.find('!').unwrap();
        let cases = [
            (0, 0, first_end),
            (2, 0, first_end),
            (first_end, 0, second_end),
            (second_start, 0, second_end),
            (second_start + 2, second_start, second_end),
            (second_end, second_start, text.len()),
            (text.len(), second_start, text.len()),
        ];

        for (cursor, expected_previous, expected_next) in cases {
            assert_eq!(
                previous_word(text, cursor),
                expected_previous,
                "previous at {cursor}"
            );
            assert_eq!(next_word(text, cursor), expected_next, "next at {cursor}");
        }
    }

    #[test]
    fn word_targets_fall_back_to_buffer_edges_when_no_word_exists() {
        for text in ["", "  \t—!?\n"] {
            for cursor in 0..=text.len() + 1 {
                assert_eq!(previous_word(text, cursor), 0);
                assert_eq!(next_word(text, cursor), text.len());
            }
        }
    }

    #[test]
    fn every_operation_produces_an_in_bounds_grapheme_boundary() {
        let text = "a e\u{301},👩\u{200d}💻 κόσμος";

        for candidate in 0..=text.len() + 3 {
            for target in [
                clamp_cursor(text, candidate),
                previous_grapheme(text, candidate),
                next_grapheme(text, candidate),
                previous_word(text, candidate),
                next_word(text, candidate),
            ] {
                assert_boundary(text, target);
            }

            let mut inserted = text.to_owned();
            let inserted_cursor = insert(&mut inserted, candidate, "界");
            assert_boundary(&inserted, inserted_cursor);

            let mut backward = text.to_owned();
            let backward_cursor = backspace(&mut backward, candidate);
            assert_boundary(&backward, backward_cursor);

            let mut forward = text.to_owned();
            let forward_cursor = delete_forward(&mut forward, candidate);
            assert_boundary(&forward, forward_cursor);
        }
    }

    fn assert_boundary(text: &str, cursor: usize) {
        assert!(cursor <= text.len(), "{cursor} exceeds {}", text.len());
        assert!(
            cursor == text.len()
                || text
                    .grapheme_indices(true)
                    .any(|(boundary, _)| boundary == cursor),
            "{cursor} is not a grapheme boundary in {text:?}"
        );
    }
}
