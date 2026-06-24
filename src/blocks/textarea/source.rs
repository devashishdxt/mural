use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::text::wrap::{self, SourceRange, WrappedFragment};

#[derive(Debug)]
pub(super) struct WrappedSourceRow {
    pub(super) start: usize,
    pub(super) visible_end: usize,
    pub(super) end: usize,
    pub(super) fragments: Vec<WrappedFragment>,
}

pub(super) fn editable_wrap_width(content_width: usize) -> usize {
    content_width.saturating_sub(1).max(1)
}

pub(super) fn wrapped_source_rows(value: &str, content_width: usize) -> Vec<WrappedSourceRow> {
    let mut rows = Vec::new();
    let mut line_start = 0;

    for (index, ch) in value.char_indices() {
        if ch == '\n' {
            push_wrapped_source_line(value, line_start, index, content_width, &mut rows);
            line_start = index + ch.len_utf8();
        }
    }
    push_wrapped_source_line(value, line_start, value.len(), content_width, &mut rows);

    rows
}

fn push_wrapped_source_line(
    value: &str,
    start: usize,
    end: usize,
    content_width: usize,
    rows: &mut Vec<WrappedSourceRow>,
) {
    let line = &value[start..end];
    if line.is_empty() {
        rows.push(WrappedSourceRow {
            start,
            visible_end: end,
            end,
            fragments: Vec::new(),
        });
        return;
    }

    let wrapped_lines = wrap::offset_wrapped_lines(
        wrap::wrap_source_by(line, content_width, textarea_display_width),
        start,
    );

    if wrapped_lines.is_empty() {
        rows.push(WrappedSourceRow {
            start,
            visible_end: end,
            end,
            fragments: Vec::new(),
        });
        return;
    }

    for fragments in wrapped_lines {
        let row_start = fragments
            .first()
            .map(|fragment| fragment.word_range.start)
            .unwrap_or(start);
        let row_end = fragments
            .last()
            .map(|fragment| fragment.whitespace_range.end)
            .unwrap_or(end);
        let visible_end = visible_end_for_fragments(&fragments).unwrap_or(row_start);
        rows.push(WrappedSourceRow {
            start: row_start,
            visible_end,
            end: row_end,
            fragments,
        });
    }
}

fn visible_end_for_fragments(fragments: &[WrappedFragment]) -> Option<usize> {
    fragments
        .iter()
        .enumerate()
        .flat_map(|(index, fragment)| {
            let include_whitespace = index + 1 < fragments.len();
            std::iter::once(fragment.word_range.end)
                .chain(include_whitespace.then_some(fragment.whitespace_range.end))
        })
        .max()
}

pub(super) fn last_rendered_grapheme_end(source: &str, row: &WrappedSourceRow) -> Option<usize> {
    rendered_ranges(row)
        .flat_map(|range| {
            range
                .text(source)
                .grapheme_indices(true)
                .map(move |(offset, grapheme)| range.start + offset + grapheme.len())
        })
        .last()
}

pub(super) fn rendered_source_row_width(source: &str, row: &WrappedSourceRow) -> usize {
    rendered_ranges(row)
        .map(|range| range.display_width_by(source, textarea_display_width))
        .sum()
}

fn rendered_ranges(row: &WrappedSourceRow) -> impl Iterator<Item = SourceRange> + '_ {
    row.fragments
        .iter()
        .enumerate()
        .flat_map(|(index, fragment)| {
            let include_whitespace = index + 1 < row.fragments.len();
            std::iter::once(fragment.word_range)
                .chain(include_whitespace.then_some(fragment.whitespace_range))
        })
}

pub(super) fn textarea_display_width(content: &str) -> usize {
    if content == "\t" {
        4
    } else {
        UnicodeWidthStr::width(content)
    }
}
