use ansi_str::AnsiStr;
use unicode_segmentation::UnicodeSegmentation;

use crate::KeyModifiers;

pub(super) fn text_insertion_modifiers(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(
        KeyModifiers::ALT
            | KeyModifiers::CONTROL
            | KeyModifiers::SUPER
            | KeyModifiers::META
            | KeyModifiers::HYPER,
    )
}

pub(super) fn previous_grapheme_boundary(value: &str, byte_index: usize) -> usize {
    if byte_index >= value.len() {
        return value.len();
    }

    value
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= byte_index)
        .last()
        .unwrap_or(0)
}

pub(super) fn next_grapheme_boundary(value: &str, byte_index: usize) -> usize {
    value
        .grapheme_indices(true)
        .map(|(index, grapheme)| index + grapheme.len())
        .find(|index| *index > byte_index)
        .unwrap_or(value.len())
}

pub(super) fn previous_word_start(value: &str, cursor: usize) -> usize {
    value
        .unicode_word_indices()
        .map(|(start, _)| start)
        .take_while(|start| *start < cursor)
        .last()
        .unwrap_or(0)
}

pub(super) fn next_word_end(value: &str, cursor: usize) -> usize {
    value
        .unicode_word_indices()
        .map(|(start, word)| start + word.len())
        .find(|end| *end > cursor)
        .unwrap_or(value.len())
}

pub(super) fn sanitize_input(input: &str) -> String {
    let stripped = input.ansi_strip();
    let mut sanitized = String::with_capacity(stripped.len());
    let mut chars = stripped.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                sanitized.push('\n');
            }
            '\n' => {
                if chars.peek() == Some(&'\r') {
                    chars.next();
                }
                sanitized.push('\n');
            }
            '\t' => sanitized.push('\t'),
            ch if ch.is_control() => {}
            ch => sanitized.push(ch),
        }
    }

    sanitized
}
