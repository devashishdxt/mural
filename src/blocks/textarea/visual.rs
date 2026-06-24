use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::text::wrap::SourceRange;

use super::source::{textarea_display_width, wrapped_source_rows};

#[derive(Debug)]
pub(super) struct VisualRow {
    pub(super) start: usize,
    pub(super) visible_end: usize,
    pub(super) end: usize,
    pub(super) width: usize,
    pub(super) cells: Vec<VisualCell>,
}

impl VisualRow {
    fn new(start: usize) -> Self {
        Self {
            start,
            visible_end: start,
            end: start,
            width: 0,
            cells: Vec::new(),
        }
    }

    fn push(&mut self, start: usize, end: usize, width: usize) {
        let column = self.width;
        self.cells.push(VisualCell {
            start,
            end,
            column,
            width,
        });
        self.width += width;
        self.end = end;
    }

    pub(super) fn column_for_cursor(&self, cursor: usize) -> usize {
        for cell in &self.cells {
            if cell.start == cursor {
                return cell.column;
            }
            if cell.end == cursor {
                return cell.column + cell.width;
            }
        }
        self.width
    }

    pub(super) fn cursor_for_column(&self, column: usize) -> usize {
        for cell in &self.cells {
            if column < cell.column + cell.width {
                return if column <= cell.column {
                    cell.start
                } else {
                    cell.end
                };
            }
        }
        self.visible_end
    }
}

#[derive(Debug)]
pub(super) struct VisualCell {
    start: usize,
    pub(super) end: usize,
    column: usize,
    width: usize,
}

pub(super) fn visual_rows(value: &str, content_width: usize) -> Vec<VisualRow> {
    wrapped_source_rows(value, content_width)
        .into_iter()
        .map(|source_row| {
            let mut row = VisualRow::new(source_row.start);
            for (fragment_index, fragment) in source_row.fragments.iter().enumerate() {
                push_visual_range(&mut row, value, fragment.word_range);
                if fragment_index + 1 < source_row.fragments.len() {
                    push_visual_range(&mut row, value, fragment.whitespace_range);
                } else {
                    row.width += UnicodeWidthStr::width(fragment.penalty.as_str());
                }
            }
            row.visible_end = source_row.visible_end;
            row.end = source_row.end;
            row
        })
        .collect()
}

fn push_visual_range(row: &mut VisualRow, source: &str, range: SourceRange) {
    for (offset, grapheme) in range.text(source).grapheme_indices(true) {
        let start = range.start + offset;
        let end = start + grapheme.len();
        row.push(start, end, textarea_display_width(grapheme));
    }
}

pub(super) fn visual_cursor_row(rows: &[VisualRow], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .position(|(index, row)| {
            let next_starts_at_cursor =
                rows.get(index + 1).is_some_and(|next| next.start == cursor);
            cursor >= row.start
                && cursor <= row.end
                && !(cursor == row.end && next_starts_at_cursor)
        })
        .unwrap_or_else(|| rows.len().saturating_sub(1))
}
