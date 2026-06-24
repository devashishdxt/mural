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
        if grapheme == "\t" && under_cursor && cursor_at_grapheme_end {
            row.push_unit_unwrapped(CursorUnit::text("    ", cursor.style));
        } else if grapheme == "\t" {
            row.push_tab_unwrapped(under_cursor, cursor.style);
        } else {
            row.push_unit_unwrapped(CursorUnit::text(
                grapheme,
                if under_cursor { cursor.style } else { style },
            ));
        }
        cursor.rendered |= under_cursor;
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
        if self.width > 0 && self.width + unit.width > content_width {
            rows.push(self.finish_and_reset());
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

    fn push_tab_unwrapped(&mut self, under_cursor: bool, cursor_style: Style) {
        if under_cursor {
            self.push_unit_unwrapped(CursorUnit::tab_under_cursor(cursor_style));
        } else {
            self.push_unit_unwrapped(CursorUnit::text("    ", Style::new()));
        }
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
