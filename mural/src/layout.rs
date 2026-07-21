//! Source-mapped layout for editable text.

use std::ops::Range;

use textwrap::{WordSeparator, WordSplitter, core::Fragment, wrap_algorithms::wrap_first_fit};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::editing;

const TAB_WIDTH: usize = 4;

/// Selects one of the two visual positions at a soft-wrap boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum WrapAffinity {
    /// Use the start of the following visual row.
    #[default]
    NextRow,
    /// Use the end of the preceding visual row.
    PreviousRow,
}

/// A zero-based location in the visual layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisualPosition {
    pub(crate) row: usize,
    pub(crate) column: usize,
}

/// A legal source cursor and the affinity needed to display it at the target position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CursorTarget {
    pub(crate) cursor: usize,
    pub(crate) affinity: WrapAffinity,
}

/// One source-mapped visible cell run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cell<'a> {
    text: &'a str,
    source: Range<usize>,
    width: usize,
    kind: CellKind,
}

impl<'a> Cell<'a> {
    pub(crate) fn text(&self) -> &'a str {
        self.text
    }

    pub(crate) fn source(&self) -> Range<usize> {
        self.source.clone()
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn is_tab(&self) -> bool {
        matches!(self.kind, CellKind::Tab { .. })
    }

    pub(crate) fn tab_offset(&self) -> Option<usize> {
        match self.kind {
            CellKind::Text => None,
            CellKind::Tab { offset } => Some(offset),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellKind {
    Text,
    Tab { offset: usize },
}

/// One visual row and its source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row<'a> {
    cells: Vec<Cell<'a>>,
    content_width: usize,
    occupied_width: usize,
    hidden: Option<Range<usize>>,
    soft_wrap: bool,
    soft_boundary: Option<usize>,
    empty_cursor: Option<usize>,
}

impl<'a> Row<'a> {
    pub(crate) fn cells(&self) -> &[Cell<'a>] {
        &self.cells
    }

    pub(crate) fn content_width(&self) -> usize {
        self.content_width
    }

    /// Includes the ordinarily reserved software-cursor column.
    pub(crate) fn occupied_width(&self) -> usize {
        self.occupied_width
    }

    pub(crate) fn hidden_source(&self) -> Option<Range<usize>> {
        self.hidden.clone()
    }

    pub(crate) fn is_soft_wrapped(&self) -> bool {
        self.soft_wrap
    }

    pub(crate) fn soft_boundary(&self) -> Option<usize> {
        self.soft_boundary
    }
}

/// A recomputed, cache-free editable-text layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout<'a> {
    text: &'a str,
    width: usize,
    rows: Vec<Row<'a>>,
}

impl<'a> Layout<'a> {
    pub(crate) fn new(text: &'a str, width: usize) -> Self {
        let mut layout = Self {
            text,
            width,
            rows: Vec::new(),
        };
        layout.build_rows();
        layout
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn rows(&self) -> &[Row<'a>] {
        &self.rows
    }

    /// Maps a legal source byte cursor to its visual position.
    pub(crate) fn source_to_visual(
        &self,
        cursor: usize,
        affinity: WrapAffinity,
    ) -> Option<VisualPosition> {
        if self.rows.is_empty() {
            return None;
        }
        let cursor = editing::clamp_cursor(self.text, cursor);

        if let Some(position) = self.soft_wrap_position(cursor, affinity) {
            return Some(position);
        }

        self.rows.iter().enumerate().find_map(|(row, _)| {
            self.candidates(row)
                .into_iter()
                .find(|candidate| candidate.canonical && candidate.target.cursor == cursor)
                .map(|candidate| VisualPosition {
                    row,
                    column: candidate.column.max(0) as usize,
                })
        })
    }

    /// Maps a visual column to the nearest legal source cursor without splitting a cell.
    pub(crate) fn visual_to_source(&self, row: usize, column: usize) -> Option<CursorTarget> {
        let row_data = self.rows.get(row)?;
        let column = column.min(row_data.content_width) as i64;
        nearest_candidate(self.candidates(row), column).map(|candidate| candidate.target)
    }

    pub(crate) fn row_start_target(&self, row: usize) -> Option<CursorTarget> {
        if row > 0 {
            let previous = &self.rows[row - 1];
            if previous.soft_wrap
                && let Some(cursor) = previous.soft_boundary
            {
                return Some(CursorTarget {
                    cursor,
                    affinity: WrapAffinity::NextRow,
                });
            }
        }
        self.visual_to_source(row, 0)
    }

    pub(crate) fn row_end_target(&self, row: usize) -> Option<CursorTarget> {
        let row_data = self.rows.get(row)?;
        if row_data.soft_wrap
            && let Some(cursor) = row_data.soft_boundary
        {
            return Some(CursorTarget {
                cursor,
                affinity: WrapAffinity::PreviousRow,
            });
        }
        self.visual_to_source(row, row_data.content_width)
    }

    fn build_rows(&mut self) {
        if self.width == 0 {
            return;
        }

        let mut line_start = 0;
        for (newline, _) in self.text.match_indices('\n') {
            self.build_explicit_line(line_start, newline);
            line_start = newline + 1;
        }
        self.build_explicit_line(line_start, self.text.len());
    }

    fn build_explicit_line(&mut self, start: usize, end: usize) {
        if self.width == 1 {
            self.rows.push(Row {
                cells: Vec::new(),
                content_width: 0,
                occupied_width: 1,
                hidden: (start < end).then_some(start..end),
                soft_wrap: false,
                soft_boundary: None,
                empty_cursor: Some(start),
            });
            return;
        }

        let content_limit = self.width - 1;
        let fragments = source_fragments(self.text, start..end, content_limit);
        let wrapped = wrap_first_fit(&fragments, &[content_limit as f64]);
        let first_row = self.rows.len();

        for (index, fragments) in wrapped.iter().enumerate() {
            let soft_wrap = index + 1 < wrapped.len();
            self.rows
                .push(row_from_fragments(fragments, soft_wrap, self.width, start));
        }

        self.finish_soft_wraps(first_row);
        self.add_full_width_continuation(end);
    }

    fn finish_soft_wraps(&mut self, first_row: usize) {
        let last_row = self.rows.len().saturating_sub(1);
        for row_index in first_row..last_row {
            if !self.rows[row_index].soft_wrap {
                continue;
            }
            let boundary = if let Some(hidden) = &self.rows[row_index].hidden {
                Some(hidden.end)
            } else {
                boundary_between(&self.rows[row_index], &self.rows[row_index + 1])
            };
            self.rows[row_index].soft_boundary = boundary;
        }
    }

    fn add_full_width_continuation(&mut self, line_end: usize) {
        let Some(last) = self.rows.last_mut() else {
            return;
        };
        if last.content_width != self.width || last.soft_wrap {
            return;
        }

        last.soft_wrap = true;
        last.soft_boundary = Some(line_end);
        self.rows.push(empty_row(line_end));
    }

    fn soft_wrap_position(&self, cursor: usize, affinity: WrapAffinity) -> Option<VisualPosition> {
        self.rows.iter().enumerate().find_map(|(row, data)| {
            (data.soft_boundary == Some(cursor)).then(|| match affinity {
                WrapAffinity::PreviousRow => VisualPosition {
                    row,
                    column: data.content_width,
                },
                WrapAffinity::NextRow => VisualPosition {
                    row: row + 1,
                    column: 0,
                },
            })
        })
    }

    fn candidates(&self, row_index: usize) -> Vec<Candidate> {
        let row = &self.rows[row_index];
        let mut candidates = Vec::new();

        add_soft_candidates(&self.rows, row_index, &mut candidates);
        add_cell_candidates(row, &mut candidates);
        self.add_hidden_candidates(row, &mut candidates);

        if let Some(cursor) = row.empty_cursor {
            candidates.push(Candidate::new(0, cursor, WrapAffinity::NextRow));
        }
        candidates
    }

    fn add_hidden_candidates(&self, row: &Row<'_>, candidates: &mut Vec<Candidate>) {
        let Some(hidden) = &row.hidden else {
            return;
        };
        for cursor in grapheme_boundaries(&self.text[hidden.clone()], hidden.start) {
            candidates.push(Candidate::new(
                row.content_width as i64,
                cursor,
                WrapAffinity::PreviousRow,
            ));
        }
    }
}

#[derive(Debug, Clone)]
struct WrapFragment<'a> {
    cells: Vec<Cell<'a>>,
    whitespace: Vec<Cell<'a>>,
    width: usize,
    whitespace_width: usize,
}

impl Fragment for WrapFragment<'_> {
    fn width(&self) -> f64 {
        self.width as f64
    }

    fn whitespace_width(&self) -> f64 {
        self.whitespace_width as f64
    }

    fn penalty_width(&self) -> f64 {
        0.0
    }
}

#[derive(Debug)]
struct SourcePart {
    content: Range<usize>,
    whitespace: Range<usize>,
}

fn source_fragments<'a>(
    text: &'a str,
    source: Range<usize>,
    content_limit: usize,
) -> Vec<WrapFragment<'a>> {
    let line = &text[source.clone()];
    let parts = source_parts(line, source.start);
    let part_count = parts.len();
    let mut fragments = Vec::new();

    for (index, part) in parts.into_iter().enumerate() {
        let mut cells = cells_for_range(text, part.content);
        let mut whitespace = cells_for_range(text, part.whitespace);
        if index + 1 == part_count {
            cells.append(&mut whitespace);
        }
        push_fragment(cells, whitespace, content_limit, &mut fragments);
    }
    fragments
}

fn source_parts(line: &str, source_start: usize) -> Vec<SourcePart> {
    let mut parts = Vec::new();
    let mut consumed = 0;

    for word in WordSeparator::new().find_words(line) {
        let word_start = consumed;
        let word_end = word_start + word.word.len();
        let full_end = word_end + word.whitespace.len();
        let mut piece_start = word_start;

        for split in WordSplitter::HyphenSplitter.split_points(word.word) {
            parts.push(SourcePart {
                content: source_start + piece_start..source_start + word_start + split,
                whitespace: source_start + word_start + split..source_start + word_start + split,
            });
            piece_start = word_start + split;
        }

        parts.push(SourcePart {
            content: source_start + piece_start..source_start + word_end,
            whitespace: source_start + word_end..source_start + full_end,
        });
        consumed = full_end;
    }

    if parts.is_empty() {
        parts.push(SourcePart {
            content: source_start..source_start,
            whitespace: source_start..source_start,
        });
    }
    parts
}

fn push_fragment<'a>(
    cells: Vec<Cell<'a>>,
    whitespace: Vec<Cell<'a>>,
    content_limit: usize,
    fragments: &mut Vec<WrapFragment<'a>>,
) {
    let width = cells_width(&cells);
    if width <= content_limit {
        fragments.push(WrapFragment {
            cells,
            whitespace_width: cells_width(&whitespace),
            whitespace,
            width,
        });
        return;
    }

    let cell_count = cells.len();
    for (index, cell) in cells.into_iter().enumerate() {
        let is_last = index + 1 == cell_count;
        let (fragment_whitespace, whitespace_width) = if is_last {
            (whitespace.clone(), cells_width(&whitespace))
        } else {
            (Vec::new(), 0)
        };
        fragments.push(WrapFragment {
            width: cell.width,
            cells: vec![cell],
            whitespace_width,
            whitespace: fragment_whitespace,
        });
    }
}

fn cells_for_range<'a>(text: &'a str, source: Range<usize>) -> Vec<Cell<'a>> {
    let mut cells = Vec::new();
    for (offset, grapheme) in text[source.clone()].grapheme_indices(true) {
        let start = source.start + offset;
        let range = start..start + grapheme.len();
        if grapheme == "\t" {
            for tab_offset in 0..TAB_WIDTH {
                cells.push(Cell {
                    text: " ",
                    source: range.clone(),
                    width: 1,
                    kind: CellKind::Tab { offset: tab_offset },
                });
            }
        } else {
            cells.push(Cell {
                text: grapheme,
                source: range,
                width: UnicodeWidthStr::width(grapheme),
                kind: CellKind::Text,
            });
        }
    }
    cells
}

fn row_from_fragments<'a>(
    fragments: &[WrapFragment<'a>],
    soft_wrap: bool,
    width: usize,
    empty_cursor: usize,
) -> Row<'a> {
    let mut cells = Vec::new();
    let mut hidden = None;

    for (index, fragment) in fragments.iter().enumerate() {
        cells.extend(fragment.cells.iter().cloned());
        let hide_whitespace = soft_wrap && index + 1 == fragments.len();
        if hide_whitespace && !fragment.whitespace.is_empty() {
            hidden = cell_range(&fragment.whitespace);
        } else {
            cells.extend(fragment.whitespace.iter().cloned());
        }
    }

    let content_width = cells_width(&cells);
    let occupied_width = if content_width == width {
        width
    } else {
        content_width + 1
    };
    Row {
        cells,
        content_width,
        occupied_width,
        hidden,
        soft_wrap,
        soft_boundary: None,
        empty_cursor: (fragments.is_empty() || content_width == 0).then_some(empty_cursor),
    }
}

fn empty_row(cursor: usize) -> Row<'static> {
    Row {
        cells: Vec::new(),
        content_width: 0,
        occupied_width: 1,
        hidden: None,
        soft_wrap: false,
        soft_boundary: None,
        empty_cursor: Some(cursor),
    }
}

fn boundary_between(previous: &Row<'_>, next: &Row<'_>) -> Option<usize> {
    let last = previous.cells.last()?;
    let first = next.cells.first()?;
    if last.is_tab() && first.is_tab() && last.source == first.source {
        None
    } else if last.source.end == first.source.start {
        Some(last.source.end)
    } else {
        None
    }
}

fn cell_range(cells: &[Cell<'_>]) -> Option<Range<usize>> {
    Some(cells.first()?.source.start..cells.last()?.source.end)
}

fn cells_width(cells: &[Cell<'_>]) -> usize {
    cells.iter().map(|cell| cell.width).sum()
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    column: i64,
    target: CursorTarget,
    canonical: bool,
}

impl Candidate {
    fn new(column: i64, cursor: usize, affinity: WrapAffinity) -> Self {
        Self {
            column,
            target: CursorTarget { cursor, affinity },
            canonical: true,
        }
    }

    fn virtual_target(column: i64, cursor: usize) -> Self {
        Self {
            column,
            target: CursorTarget {
                cursor,
                affinity: WrapAffinity::NextRow,
            },
            canonical: false,
        }
    }
}

fn add_soft_candidates(rows: &[Row<'_>], row: usize, candidates: &mut Vec<Candidate>) {
    if let Some(cursor) = rows[row].soft_boundary {
        candidates.push(Candidate::new(
            rows[row].content_width as i64,
            cursor,
            WrapAffinity::PreviousRow,
        ));
    }
    if row > 0
        && let Some(cursor) = rows[row - 1].soft_boundary
    {
        candidates.push(Candidate::new(0, cursor, WrapAffinity::NextRow));
    }
}

fn add_cell_candidates(row: &Row<'_>, candidates: &mut Vec<Candidate>) {
    let mut column = 0_i64;
    for cell in &row.cells {
        match cell.kind {
            CellKind::Text => {
                candidates.push(Candidate::new(
                    column,
                    cell.source.start,
                    WrapAffinity::NextRow,
                ));
                candidates.push(Candidate::new(
                    column + cell.width as i64,
                    cell.source.end,
                    WrapAffinity::NextRow,
                ));
            }
            CellKind::Tab { offset } => {
                let start_column = column - offset as i64;
                let end_column = column + (TAB_WIDTH - offset) as i64;
                candidates.push(if offset == 0 {
                    Candidate::new(start_column, cell.source.start, WrapAffinity::NextRow)
                } else {
                    Candidate::virtual_target(start_column, cell.source.start)
                });
                candidates.push(if offset + 1 == TAB_WIDTH {
                    Candidate::new(end_column, cell.source.end, WrapAffinity::NextRow)
                } else {
                    Candidate::virtual_target(end_column, cell.source.end)
                });
            }
        }
        column += cell.width as i64;
    }
}

fn grapheme_boundaries(text: &str, source_start: usize) -> Vec<usize> {
    let mut boundaries = vec![source_start];
    boundaries.extend(
        text.grapheme_indices(true)
            .map(|(offset, grapheme)| source_start + offset + grapheme.len()),
    );
    boundaries
}

fn nearest_candidate(candidates: Vec<Candidate>, column: i64) -> Option<Candidate> {
    candidates
        .into_iter()
        .min_by_key(|candidate| candidate.column.abs_diff(column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_lines_and_first_fit_wrapping_preserve_source_structure() {
        let cases = [
            ("", 6, vec![""]),
            ("a\n\nb\n", 6, vec!["a", "", "b", ""]),
            ("one two", 6, vec!["one", "two"]),
            ("one two", 8, vec!["one two"]),
            ("foo-bar", 5, vec!["foo-", "bar"]),
            ("你好世界", 5, vec!["你好", "世界"]),
            ("trail  ", 6, vec!["trail", "  "]),
        ];

        for (text, width, expected) in cases {
            let layout = Layout::new(text, width);
            assert_eq!(
                visible_rows(&layout),
                expected,
                "text: {text:?}, width: {width}"
            );
        }
    }

    #[test]
    fn separator_spaces_are_hidden_only_when_the_following_word_wraps() {
        let wrapped = Layout::new("one   two", 7);
        assert_eq!(visible_rows(&wrapped), ["one", "two"]);
        assert_eq!(wrapped.rows()[0].hidden_source(), Some(3..6));
        assert_eq!(wrapped.rows()[0].soft_boundary(), Some(6));

        let unwrapped = Layout::new("one   two", 10);
        assert_eq!(visible_rows(&unwrapped), ["one   two"]);
        assert_eq!(unwrapped.rows()[0].hidden_source(), None);
    }

    #[test]
    fn hard_wrapping_never_splits_extended_graphemes() {
        let clusters = ["e\u{301}", "👩\u{200d}💻", "界", "\u{200b}"];
        for cluster in clusters {
            let text = format!("a{cluster}b");
            let layout = Layout::new(&text, 2);
            let ranges = layout
                .rows()
                .iter()
                .flat_map(Row::cells)
                .filter(|cell| !cell.is_tab())
                .map(Cell::source)
                .collect::<Vec<_>>();

            assert!(
                ranges.contains(&(1..1 + cluster.len())),
                "cluster {cluster:?} was split: {ranges:?}"
            );
        }
    }

    #[test]
    fn tabs_are_four_cells_and_can_continue_across_narrow_rows() {
        let layout = Layout::new("a\tb", 3);
        assert_eq!(visible_rows(&layout), ["a ", "  ", " b"]);

        let tab_cells = layout
            .rows()
            .iter()
            .flat_map(Row::cells)
            .filter(|cell| cell.is_tab())
            .collect::<Vec<_>>();
        assert_eq!(tab_cells.len(), 4);
        assert!(tab_cells.iter().all(|cell| cell.width() == 1));
        assert!(tab_cells.iter().all(|cell| cell.source() == (1..2)));
        assert_eq!(
            tab_cells
                .iter()
                .map(|cell| cell.tab_offset().unwrap())
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn source_mapping_distinguishes_both_sides_of_soft_wraps() {
        let layout = Layout::new("one two", 6);
        assert_eq!(layout.width(), 6);
        assert!(layout.rows()[0].is_soft_wrapped());
        assert_eq!(
            layout.source_to_visual(4, WrapAffinity::PreviousRow),
            Some(VisualPosition { row: 0, column: 3 })
        );
        assert_eq!(
            layout.source_to_visual(4, WrapAffinity::NextRow),
            Some(VisualPosition { row: 1, column: 0 })
        );
        assert_eq!(
            layout.row_end_target(0),
            Some(CursorTarget {
                cursor: 4,
                affinity: WrapAffinity::PreviousRow,
            })
        );
        assert_eq!(
            layout.row_start_target(1),
            Some(CursorTarget {
                cursor: 4,
                affinity: WrapAffinity::NextRow,
            })
        );
    }

    #[test]
    fn every_hidden_separator_cursor_has_a_visible_position() {
        let layout = Layout::new("one   two", 7);
        for cursor in 3..=6 {
            assert_eq!(
                layout.source_to_visual(cursor, WrapAffinity::PreviousRow),
                Some(VisualPosition { row: 0, column: 3 }),
                "cursor {cursor}"
            );
        }
    }

    #[test]
    fn visual_targets_choose_only_legal_wide_and_tab_boundaries() {
        let wide = Layout::new("界x", 4);
        assert_eq!(wide.visual_to_source(0, 1).unwrap().cursor, 0);
        assert_eq!(wide.visual_to_source(0, 2).unwrap().cursor, "界".len());

        let tab = Layout::new("\t", 3);
        for row in 0..tab.rows().len() {
            for column in 0..=tab.rows()[row].content_width() {
                assert!(matches!(
                    tab.visual_to_source(row, column).unwrap().cursor,
                    0 | 1
                ));
            }
        }
        assert_eq!(
            tab.source_to_visual(0, WrapAffinity::NextRow),
            Some(VisualPosition { row: 0, column: 0 })
        );
        assert_eq!(
            tab.source_to_visual(1, WrapAffinity::NextRow),
            Some(VisualPosition { row: 1, column: 2 })
        );
    }

    #[test]
    fn rows_honor_reservation_and_the_full_width_wide_exception() {
        for (text, width) in [("abcdef", 4), ("界", 2), ("\t", 2), ("a界b", 3)] {
            let layout = Layout::new(text, width);
            assert!(
                layout
                    .rows()
                    .iter()
                    .all(|row| row.occupied_width() <= width)
            );
        }

        let wide = Layout::new("界", 2);
        assert_eq!(visible_rows(&wide), ["界", ""]);
        assert_eq!(wide.rows()[0].content_width(), 2);
        assert_eq!(wide.rows()[0].occupied_width(), 2);
        assert_eq!(
            wide.source_to_visual("界".len(), WrapAffinity::NextRow),
            Some(VisualPosition { row: 1, column: 0 })
        );

        let one_column = Layout::new("content", 1);
        assert_eq!(visible_rows(&one_column), [""]);
        assert_eq!(one_column.rows()[0].occupied_width(), 1);
        assert!(Layout::new("content", 0).rows().is_empty());
    }

    #[test]
    fn zero_width_graphemes_keep_distinct_legal_cursor_boundaries() {
        let text = "a\u{200b}b";
        let layout = Layout::new(text, 4);
        let before = layout.source_to_visual(1, WrapAffinity::NextRow).unwrap();
        let after = layout
            .source_to_visual(1 + "\u{200b}".len(), WrapAffinity::NextRow)
            .unwrap();
        assert_eq!(before, after);
        assert_ne!(1, 1 + "\u{200b}".len());
    }

    fn visible_rows(layout: &Layout<'_>) -> Vec<String> {
        layout
            .rows()
            .iter()
            .map(|row| row.cells().iter().map(Cell::text).collect())
            .collect()
    }
}
