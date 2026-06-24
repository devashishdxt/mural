use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    Span, Style,
    text::wrap::{SourceRange, WrappedFragment},
};

pub(super) struct RenderCursor {
    byte_index: usize,
    style: Style,
    pub(super) overlay_grapheme_end: Option<usize>,
    pub(super) rendered: bool,
}

impl RenderCursor {
    pub(super) fn new(byte_index: usize, style: Style) -> Self {
        Self {
            byte_index,
            style,
            overlay_grapheme_end: None,
            rendered: false,
        }
    }
}

pub(super) fn push_rendered_fragments(
    row: &mut ContentRow,
    source: &str,
    fragments: &[WrappedFragment],
    style: Style,
    cursor: &mut RenderCursor,
) {
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        push_rendered_range(row, source, fragment.word_range, style, cursor);
        if fragment_index + 1 < fragments.len() {
            push_rendered_range(row, source, fragment.whitespace_range, style, cursor);
        } else if !fragment.penalty.is_empty() {
            row.push_unit_unwrapped(CursorUnit::text(&fragment.penalty, style));
        }
    }
}

pub(super) fn push_rendered_fragments_wrapped(
    target: &mut WrappedRenderTarget<'_>,
    source: &str,
    fragments: &[WrappedFragment],
    style: Style,
    cursor: &mut RenderCursor,
    include_trailing_whitespace: bool,
) {
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        push_rendered_range_wrapped(target, source, fragment.word_range, style, cursor);
        if fragment_index + 1 < fragments.len()
            || (include_trailing_whitespace && !fragment.whitespace_range.is_empty())
        {
            push_rendered_range_wrapped(target, source, fragment.whitespace_range, style, cursor);
        } else if !fragment.penalty.is_empty() {
            target.push_unit(CursorUnit::text(&fragment.penalty, style), false);
        }
    }
}

pub(super) struct WrappedRenderTarget<'a> {
    row: &'a mut ContentRow,
    rows: &'a mut Vec<Vec<Span>>,
    content_width: usize,
    cursor_row: Option<usize>,
}

impl<'a> WrappedRenderTarget<'a> {
    pub(super) fn new(
        row: &'a mut ContentRow,
        rows: &'a mut Vec<Vec<Span>>,
        content_width: usize,
    ) -> Self {
        Self {
            row,
            rows,
            content_width,
            cursor_row: None,
        }
    }

    pub(super) fn push_unit(&mut self, unit: CursorUnit, marks_cursor: bool) {
        self.row.push_unit_tracked(
            unit,
            self.content_width,
            self.rows,
            marks_cursor,
            &mut self.cursor_row,
        );
    }

    pub(super) fn cursor_row(&self) -> Option<usize> {
        self.cursor_row
    }
}

fn push_rendered_range(
    row: &mut ContentRow,
    source: &str,
    range: SourceRange,
    style: Style,
    cursor: &mut RenderCursor,
) {
    for (offset, grapheme) in range.text(source).grapheme_indices(true) {
        let start = range.start + offset;
        let end = start + grapheme.len();
        let cursor_at_grapheme_end = cursor.overlay_grapheme_end == Some(end);
        let under_cursor =
            !cursor.rendered && (cursor.byte_index == start || cursor_at_grapheme_end);
        row.push_unit_unwrapped(rendered_unit(
            grapheme,
            style,
            cursor.style,
            under_cursor,
            cursor_at_grapheme_end,
        ));
        cursor.rendered |= under_cursor;
    }
}

fn push_rendered_range_wrapped(
    target: &mut WrappedRenderTarget<'_>,
    source: &str,
    range: SourceRange,
    style: Style,
    cursor: &mut RenderCursor,
) {
    for (offset, grapheme) in range.text(source).grapheme_indices(true) {
        let start = range.start + offset;
        let end = start + grapheme.len();
        let cursor_at_grapheme_end = cursor.overlay_grapheme_end == Some(end);
        let under_cursor =
            !cursor.rendered && (cursor.byte_index == start || cursor_at_grapheme_end);

        target.push_unit(
            rendered_unit(
                grapheme,
                style,
                cursor.style,
                under_cursor,
                cursor_at_grapheme_end,
            ),
            under_cursor,
        );
        cursor.rendered |= under_cursor;
    }
}

fn rendered_unit(
    grapheme: &str,
    style: Style,
    cursor_style: Style,
    under_cursor: bool,
    cursor_at_grapheme_end: bool,
) -> CursorUnit {
    if grapheme == "\t" && under_cursor && cursor_at_grapheme_end {
        CursorUnit::text("    ", cursor_style)
    } else if grapheme == "\t" && under_cursor {
        CursorUnit::tab_under_cursor(cursor_style)
    } else if grapheme == "\t" {
        CursorUnit::text("    ", Style::new())
    } else {
        CursorUnit::text(grapheme, if under_cursor { cursor_style } else { style })
    }
}

#[derive(Default)]
pub(super) struct ContentRow {
    spans: Vec<Span>,
    content: String,
    style: Style,
    width: usize,
}

impl ContentRow {
    pub(super) fn push_unit(
        &mut self,
        unit: CursorUnit,
        content_width: usize,
        rows: &mut Vec<Vec<Span>>,
    ) {
        let mut cursor_row = None;
        self.push_unit_tracked(unit, content_width, rows, false, &mut cursor_row);
    }

    pub(super) fn push_unit_tracked(
        &mut self,
        unit: CursorUnit,
        content_width: usize,
        rows: &mut Vec<Vec<Span>>,
        marks_cursor: bool,
        cursor_row: &mut Option<usize>,
    ) {
        if self.width > 0 && self.width + unit.width > content_width {
            rows.push(self.finish_and_reset());
        }

        if marks_cursor {
            *cursor_row = Some(rows.len());
        }

        for piece in unit.pieces {
            self.push_text(&piece.content, piece.style);
        }
        self.width += unit.width;
    }

    pub(super) fn push_unit_unwrapped(&mut self, unit: CursorUnit) {
        for piece in unit.pieces {
            self.push_text(&piece.content, piece.style);
        }
        self.width += unit.width;
    }

    fn push_text(&mut self, content: &str, style: Style) {
        if !self.content.is_empty() && self.style != style {
            self.flush();
        }
        self.style = style;
        self.content.push_str(content);
    }

    pub(super) fn finish(mut self) -> Vec<Span> {
        self.flush();
        self.spans
    }

    fn finish_and_reset(&mut self) -> Vec<Span> {
        self.flush();
        self.width = 0;
        std::mem::take(&mut self.spans)
    }

    fn flush(&mut self) {
        if self.content.is_empty() {
            return;
        }

        self.spans.push(Span::from_trusted_content(
            std::mem::take(&mut self.content),
            self.style,
        ));
    }
}

pub(super) struct CursorUnit {
    pieces: Vec<StyledPiece>,
    width: usize,
}

impl CursorUnit {
    pub(super) fn text(content: &str, style: Style) -> Self {
        Self {
            pieces: vec![StyledPiece {
                content: content.to_owned(),
                style,
            }],
            width: UnicodeWidthStr::width(content),
        }
    }

    pub(super) fn space(style: Style) -> Self {
        Self::text(" ", style)
    }

    fn tab_under_cursor(cursor_style: Style) -> Self {
        Self {
            pieces: vec![
                StyledPiece {
                    content: " ".to_owned(),
                    style: cursor_style,
                },
                StyledPiece {
                    content: "   ".to_owned(),
                    style: Style::new(),
                },
            ],
            width: 4,
        }
    }
}

struct StyledPiece {
    content: String,
    style: Style,
}
