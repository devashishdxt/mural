//! Safe software-cursor rendering and sticky viewport selection.

use std::ops::Range;

use crate::layout::{Layout, Row, WrapAffinity};

pub(crate) const DEFAULT_MAX_HEIGHT: usize = 6;
const CURSOR_ON: &str = "\x1b[7m";
const CURSOR_OFF: &str = "\x1b[27m";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaximumHeight {
    Limited(usize),
    Unlimited,
}

impl Default for MaximumHeight {
    fn default() -> Self {
        Self::Limited(DEFAULT_MAX_HEIGHT)
    }
}

pub(crate) struct Rendered {
    pub(crate) lines: Vec<String>,
    pub(crate) scroll_row: usize,
    pub(crate) scroll_width: Option<usize>,
}

pub(crate) fn render(
    layout: &Layout<'_>,
    cursor: usize,
    affinity: WrapAffinity,
    maximum_height: MaximumHeight,
    previous_scroll_row: usize,
    previous_scroll_width: Option<usize>,
) -> Rendered {
    let Some(cursor_position) = layout.source_to_visual(cursor, affinity) else {
        return Rendered {
            lines: Vec::new(),
            scroll_row: 0,
            scroll_width: Some(layout.width()),
        };
    };

    let viewport = viewport(
        layout.rows().len(),
        cursor_position.row,
        maximum_height,
        previous_scroll_row,
        previous_scroll_width,
        layout.width(),
    );
    let lines = layout.rows()[viewport.clone()]
        .iter()
        .enumerate()
        .map(|(offset, row)| {
            let row_index = viewport.start + offset;
            let placement = (row_index == cursor_position.row).then(|| {
                cursor_placement(
                    row,
                    cursor,
                    affinity,
                    cursor_position.column,
                    layout.width(),
                )
            });
            render_row(row, placement)
        })
        .collect();

    Rendered {
        lines,
        scroll_row: viewport.start,
        scroll_width: Some(layout.width()),
    }
}

fn viewport(
    row_count: usize,
    cursor_row: usize,
    maximum_height: MaximumHeight,
    previous_scroll_row: usize,
    previous_scroll_width: Option<usize>,
    width: usize,
) -> Range<usize> {
    let height = match maximum_height {
        MaximumHeight::Limited(height) => height.min(row_count),
        MaximumHeight::Unlimited => row_count,
    };
    if height == row_count {
        return 0..row_count;
    }

    let maximum_start = row_count.saturating_sub(height);
    let mut start = previous_scroll_row.min(maximum_start);
    if previous_scroll_width != Some(width) {
        start = start.min(maximum_start);
    }

    if cursor_row < start {
        start = cursor_row;
    } else if cursor_row >= start + height {
        start = cursor_row + 1 - height;
    }
    start..start + height
}

#[derive(Debug, Clone, Copy)]
enum CursorPlacement {
    Cell(usize),
    AfterCell(usize),
    Column(usize),
}

fn cursor_placement(
    row: &Row<'_>,
    cursor: usize,
    affinity: WrapAffinity,
    column: usize,
    width: usize,
) -> CursorPlacement {
    if affinity == WrapAffinity::PreviousRow
        && let Some(index) = row.cells().iter().rposition(|cell| {
            cell.is_tab() && cell.tab_offset() == Some(3) && cell.source().end == cursor
        })
    {
        return CursorPlacement::Cell(index);
    }

    if let Some((index, cell)) = row.cells().iter().enumerate().find(|(_, cell)| {
        cell.source().start == cursor && (!cell.is_tab() || cell.tab_offset() == Some(0))
    }) {
        return if cell.width() == 0 {
            CursorPlacement::AfterCell(index)
        } else {
            CursorPlacement::Cell(index)
        };
    }

    if row.content_width() == width
        && let Some(index) = row.cells().len().checked_sub(1)
    {
        CursorPlacement::Cell(index)
    } else {
        CursorPlacement::Column(column)
    }
}

fn render_row(row: &Row<'_>, placement: Option<CursorPlacement>) -> String {
    let mut output = String::new();
    let mut column = 0;
    let mut column_cursor_written = false;

    for (index, cell) in row.cells().iter().enumerate() {
        if !column_cursor_written
            && matches!(placement, Some(CursorPlacement::Column(target)) if target == column)
        {
            push_cursor(&mut output, " ");
            column_cursor_written = true;
        }

        if matches!(placement, Some(CursorPlacement::Cell(target)) if target == index) {
            push_cursor(&mut output, cell.text());
        } else {
            output.push_str(cell.text());
        }

        if matches!(placement, Some(CursorPlacement::AfterCell(target)) if target == index) {
            push_cursor(&mut output, " ");
        }
        column += cell.width();
    }

    if matches!(placement, Some(CursorPlacement::Column(target)) if target == column)
        && !column_cursor_written
    {
        push_cursor(&mut output, " ");
    }
    output
}

fn push_cursor(output: &mut String, content: &str) {
    output.push_str(CURSOR_ON);
    output.push_str(content);
    output.push_str(CURSOR_OFF);
}

#[cfg(test)]
mod tests {
    use ansi_str::AnsiStr;
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    use super::*;

    #[test]
    fn cursor_ansi_is_exact_and_balanced_on_its_line() {
        let layout = Layout::new("ab", 4);
        let rendered = render(
            &layout,
            0,
            WrapAffinity::NextRow,
            MaximumHeight::Unlimited,
            0,
            None,
        );

        assert_eq!(rendered.lines, ["\x1b[7ma\x1b[27mb"]);
        assert_eq!(rendered.lines[0].matches(CURSOR_ON).count(), 1);
        assert_eq!(rendered.lines[0].matches(CURSOR_OFF).count(), 1);
        assert!(rendered.lines[0].ends_with("b"));
    }

    #[test]
    fn zero_width_runs_emit_only_one_cursor_cell() {
        let layout = Layout::new("\u{200b}\u{200b}", 2);
        let rendered = render(
            &layout,
            "\u{200b}\u{200b}".len(),
            WrapAffinity::NextRow,
            MaximumHeight::Unlimited,
            0,
            None,
        );

        assert_eq!(rendered.lines[0].matches(CURSOR_ON).count(), 1);
        assert_eq!(rendered.lines[0].matches(CURSOR_OFF).count(), 1);
        assert_eq!(
            UnicodeWidthStr::width(rendered.lines[0].ansi_strip().as_ref()),
            1
        );
    }

    #[test]
    fn every_line_is_terminal_safe_and_within_width() {
        let cases = [
            ("one   two", 7),
            ("界x", 3),
            ("a\tb", 3),
            ("a\u{200b}b", 4),
            ("a\nb", 2),
        ];

        for (text, width) in cases {
            let layout = Layout::new(text, width);
            let cursors = std::iter::once(0).chain(
                text.grapheme_indices(true)
                    .map(|(offset, grapheme)| offset + grapheme.len()),
            );
            for cursor in cursors {
                let rendered = render(
                    &layout,
                    cursor,
                    WrapAffinity::NextRow,
                    MaximumHeight::Unlimited,
                    0,
                    None,
                );
                for line in rendered.lines {
                    assert!(!line.contains(['\r', '\n']), "line: {line:?}");
                    let plain = line.ansi_strip();
                    assert!(
                        UnicodeWidthStr::width(plain.as_ref()) <= width,
                        "text: {text:?}, cursor: {cursor}, width: {width}, line: {line:?}"
                    );
                }
            }
        }
    }
}
