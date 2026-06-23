use std::cell::Cell;

use ansi_str::AnsiStr;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Line, Render, Span, Style, Text, TextError};

use super::{layout::push_spaces, validation::validate_non_empty_display_text};

const DEFAULT_PROMPT: &str = "›";
const DEFAULT_GAP: usize = 1;
const DEFAULT_MAX_HEIGHT: usize = 6;

/// Creates a default textarea block.
pub fn textarea() -> Textarea {
    Textarea::new()
}

/// A multi-line editable terminal input block.
#[derive(Debug, Clone)]
pub struct Textarea {
    value: String,
    cursor: usize,
    prompt: String,
    prompt_width: usize,
    prompt_style: Style,
    gap: usize,
    cursor_style: Style,
    placeholder: Option<String>,
    placeholder_style: Style,
    max_height: Option<usize>,
    scroll_row: Cell<usize>,
    last_render_width: Cell<Option<usize>>,
    preferred_visual_column: Cell<Option<usize>>,
}

impl Textarea {
    /// Creates an empty textarea with a modern prompt and a reversed-space cursor.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            prompt: DEFAULT_PROMPT.to_owned(),
            prompt_width: UnicodeWidthStr::width(DEFAULT_PROMPT),
            prompt_style: Style::new(),
            gap: DEFAULT_GAP,
            cursor_style: Style::new().reversed(),
            placeholder: None,
            placeholder_style: Style::new(),
            max_height: Some(DEFAULT_MAX_HEIGHT),
            scroll_row: Cell::new(0),
            last_render_width: Cell::new(None),
            preferred_visual_column: Cell::new(None),
        }
    }

    /// Returns the editable buffer.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Replaces the editable buffer with sanitized input and moves the cursor to the end.
    pub fn set_value(&mut self, value: impl AsRef<str>) -> &mut Self {
        self.value = sanitize_input(value.as_ref());
        self.cursor = self.value.len();
        self.reset_scroll();
        self.reset_preferred_visual_column();
        self
    }

    /// Clears the editable buffer and cursor state.
    pub fn clear(&mut self) -> &mut Self {
        self.value.clear();
        self.cursor = 0;
        self.reset_scroll();
        self.reset_preferred_visual_column();
        self
    }

    /// Returns whether the editable buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns the current value and clears the editable buffer.
    pub fn take(&mut self) -> String {
        let value = std::mem::take(&mut self.value);
        self.clear();
        value
    }

    /// Returns the cursor byte index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to `byte_index`, clamped backward to a grapheme boundary.
    pub fn set_cursor(&mut self, byte_index: usize) -> &mut Self {
        self.cursor = previous_grapheme_boundary(&self.value, byte_index.min(self.value.len()));
        self.reset_preferred_visual_column();
        self
    }

    /// Inserts sanitized text at the cursor.
    pub fn insert_str(&mut self, value: impl AsRef<str>) -> &mut Self {
        let value = sanitize_input(value.as_ref());
        self.value.insert_str(self.cursor, &value);
        self.cursor += value.len();
        self.reset_preferred_visual_column();
        self
    }

    /// Inserts one sanitized character at the cursor.
    pub fn insert_char(&mut self, ch: char) -> &mut Self {
        let mut buffer = [0; 4];
        self.insert_str(ch.encode_utf8(&mut buffer))
    }

    /// Inserts a newline at the cursor.
    pub fn insert_newline(&mut self) -> &mut Self {
        self.insert_char('\n')
    }

    /// Deletes the grapheme before the cursor.
    pub fn backspace(&mut self) -> &mut Self {
        if self.cursor == 0 {
            return self;
        }

        let previous = previous_grapheme_boundary(&self.value, self.cursor.saturating_sub(1));
        self.value.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.reset_preferred_visual_column();
        self
    }

    /// Deletes the grapheme at the cursor.
    pub fn delete(&mut self) -> &mut Self {
        if self.cursor >= self.value.len() {
            return self;
        }

        let next = next_grapheme_boundary(&self.value, self.cursor);
        self.value.replace_range(self.cursor..next, "");
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor left by one grapheme.
    pub fn move_left(&mut self) -> &mut Self {
        if self.cursor > 0 {
            self.cursor = previous_grapheme_boundary(&self.value, self.cursor.saturating_sub(1));
        }
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor right by one grapheme.
    pub fn move_right(&mut self) -> &mut Self {
        if self.cursor < self.value.len() {
            self.cursor = next_grapheme_boundary(&self.value, self.cursor);
        }
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the start of the current source line.
    pub fn move_to_line_start(&mut self) -> &mut Self {
        self.cursor = self.value[..self.cursor]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the current source line.
    pub fn move_to_line_end(&mut self) -> &mut Self {
        self.cursor = self.value[self.cursor..]
            .find('\n')
            .map(|offset| self.cursor + offset)
            .unwrap_or(self.value.len());
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the start of the buffer.
    pub fn move_to_buffer_start(&mut self) -> &mut Self {
        self.cursor = 0;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the buffer.
    pub fn move_to_buffer_end(&mut self) -> &mut Self {
        self.cursor = self.value.len();
        self.reset_preferred_visual_column();
        self
    }

    /// Returns the prompt content.
    pub fn prompt_content(&self) -> &str {
        &self.prompt
    }

    /// Sets the prompt content.
    ///
    /// The prompt must be non-empty, have non-zero terminal display width, and
    /// contain no structural terminal content.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Result<Self, TextError> {
        let prompt = prompt.into();
        self.prompt_width = validate_non_empty_display_text(&prompt)?;
        self.prompt = prompt;
        Ok(self)
    }

    /// Sets the style applied to the prompt only.
    pub fn prompt_style(mut self, style: Style) -> Self {
        self.prompt_style = style;
        self
    }

    /// Sets the number of plain-space columns between prompt and content.
    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
        self
    }

    /// Sets the style overlaid on the cursor cell.
    pub fn cursor_style(mut self, style: Style) -> Self {
        self.cursor_style = style;
        self
    }

    /// Returns the style overlaid on the cursor cell.
    pub fn cursor_style_value(&self) -> Style {
        self.cursor_style
    }

    /// Sets placeholder text shown when the buffer is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Result<Self, TextError> {
        let placeholder = placeholder.into();
        validate_non_empty_display_text(&placeholder)?;
        self.placeholder = Some(placeholder);
        Ok(self)
    }

    /// Sets the style applied to placeholder text.
    pub fn placeholder_style(mut self, style: Style) -> Self {
        self.placeholder_style = style;
        self
    }

    /// Returns the configured placeholder content.
    pub fn placeholder_content(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    /// Returns the placeholder style.
    pub fn placeholder_style_value(&self) -> Style {
        self.placeholder_style
    }

    /// Returns the prompt style.
    pub fn prompt_style_value(&self) -> Style {
        self.prompt_style
    }

    /// Returns the number of plain-space columns between prompt and content.
    pub fn gap_width(&self) -> usize {
        self.gap
    }

    /// Sets the maximum rendered visual height in rows.
    pub fn max_height(mut self, max_height: usize) -> Self {
        self.max_height = Some(max_height.max(1));
        self
    }

    /// Allows the textarea to render all visual rows.
    pub fn unlimited_height(mut self) -> Self {
        self.max_height = None;
        self
    }

    /// Returns the maximum rendered visual height, or `None` when unlimited.
    pub fn max_height_value(&self) -> Option<usize> {
        self.max_height
    }

    /// Moves the cursor one visual row up for the given terminal width.
    pub fn move_visual_up(&mut self, width: u16) -> &mut Self {
        self.move_visual_rows(width, -1)
    }

    /// Moves the cursor one visual row down for the given terminal width.
    pub fn move_visual_down(&mut self, width: u16) -> &mut Self {
        self.move_visual_rows(width, 1)
    }

    /// Moves the cursor to the start of the current visual row.
    pub fn move_to_visual_row_start(&mut self, width: u16) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        let index = visual_cursor_row(&rows, self.cursor);
        self.cursor = rows.get(index).map(|row| row.start).unwrap_or(0);
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the current visual row.
    pub fn move_to_visual_row_end(&mut self, width: u16) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        let index = visual_cursor_row(&rows, self.cursor);
        self.cursor = rows
            .get(index)
            .map(|row| row.end)
            .unwrap_or(self.value.len());
        self.reset_preferred_visual_column();
        self
    }

    fn move_visual_rows(&mut self, width: u16, delta: isize) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        if rows.is_empty() {
            return self;
        }

        let current_row = visual_cursor_row(&rows, self.cursor);
        let current_column = rows[current_row].column_for_cursor(self.cursor);
        let preferred_column = self.preferred_visual_column.get().unwrap_or(current_column);
        let target_row = current_row
            .saturating_add_signed(delta)
            .min(rows.len().saturating_sub(1));

        self.cursor = rows[target_row].cursor_for_column(preferred_column);
        self.preferred_visual_column.set(Some(preferred_column));
        self
    }

    fn visual_rows_for_width(&self, width: u16) -> Vec<VisualRow> {
        let width = usize::from(width);
        let content_width = width.saturating_sub(self.prefix_width()).max(1);
        visual_rows(&self.value, content_width)
    }

    fn reset_scroll(&self) {
        self.scroll_row.set(0);
        self.last_render_width.set(None);
    }

    fn reset_preferred_visual_column(&self) {
        self.preferred_visual_column.set(None);
    }

    fn prefix_width(&self) -> usize {
        self.prompt_width.saturating_add(self.gap)
    }

    fn first_prefix_line(&self, fitting_gap: usize) -> Line {
        let mut spans = Vec::with_capacity(2);
        spans.push(Span::from_trusted_content(
            self.prompt.clone(),
            self.prompt_style,
        ));
        push_spaces(&mut spans, fitting_gap);
        Line::from_spans(spans)
    }

    fn first_line(&self, content: Vec<Span>) -> Line {
        let mut spans = Vec::with_capacity(content.len() + 2);
        spans.push(Span::from_trusted_content(
            self.prompt.clone(),
            self.prompt_style,
        ));
        push_spaces(&mut spans, self.gap);
        spans.extend(content);
        Line::from_spans(spans)
    }

    fn continuation_line(&self, content: Vec<Span>) -> Line {
        let mut spans = Vec::with_capacity(content.len() + 1);
        push_spaces(&mut spans, self.prefix_width());
        spans.extend(content);
        Line::from_spans(spans)
    }

    fn rendered_lines(&self, width: usize) -> Vec<Line> {
        if width == 0 || width < self.prompt_width {
            return Vec::new();
        }

        let prefix_width = self.prefix_width();
        if width <= prefix_width {
            return vec![self.first_prefix_line(width - self.prompt_width)];
        }

        let content_width = width - prefix_width;
        let layout = self.content_layout(content_width);
        let start_row = self.visible_start_row(width, layout.rows.len(), layout.cursor_row);
        let max_height = self.max_height.unwrap_or(layout.rows.len());

        layout
            .rows
            .into_iter()
            .enumerate()
            .skip(start_row)
            .take(max_height)
            .map(|(index, row)| {
                if index == 0 {
                    self.first_line(row)
                } else {
                    self.continuation_line(row)
                }
            })
            .collect()
    }

    fn visible_start_row(&self, width: usize, row_count: usize, cursor_row: usize) -> usize {
        let Some(max_height) = self.max_height else {
            self.last_render_width.set(Some(width));
            self.scroll_row.set(0);
            return 0;
        };

        if row_count <= max_height {
            self.last_render_width.set(Some(width));
            self.scroll_row.set(0);
            return 0;
        }

        let max_scroll = row_count - max_height;
        let mut scroll_row = self.scroll_row.get().min(max_scroll);
        if self.last_render_width.get() != Some(width) {
            scroll_row = scroll_row.min(max_scroll);
        }

        if cursor_row < scroll_row {
            scroll_row = cursor_row;
        } else if cursor_row >= scroll_row + max_height {
            scroll_row = cursor_row + 1 - max_height;
        }
        scroll_row = scroll_row.min(max_scroll);

        self.last_render_width.set(Some(width));
        self.scroll_row.set(scroll_row);
        scroll_row
    }

    fn content_layout(&self, content_width: usize) -> ContentLayout {
        let mut rows = Vec::new();
        let mut row = ContentRow::default();

        if self.value.is_empty() {
            row.push_unit(
                CursorUnit::space(self.cursor_style),
                content_width,
                &mut rows,
            );
            if let Some(placeholder) = &self.placeholder {
                for grapheme in placeholder.graphemes(true) {
                    row.push_unit(
                        CursorUnit::text(grapheme, self.placeholder_style),
                        content_width,
                        &mut rows,
                    );
                }
            }
            rows.push(row.finish());
            return ContentLayout {
                rows,
                cursor_row: 0,
            };
        }

        let mut cursor_rendered = false;
        let mut cursor_row = 0;

        for (start, grapheme) in self.value.grapheme_indices(true) {
            let under_cursor = self.cursor == start;
            if grapheme == "\n" {
                if under_cursor {
                    row.push_unit(
                        CursorUnit::space(self.cursor_style),
                        content_width,
                        &mut rows,
                    );
                    cursor_row = rows.len();
                    cursor_rendered = true;
                }
                rows.push(row.finish());
                row = ContentRow::default();
            } else if grapheme == "\t" {
                row.push_tab(under_cursor, self.cursor_style, content_width, &mut rows);
                if under_cursor {
                    cursor_row = rows.len();
                }
                cursor_rendered |= under_cursor;
            } else {
                row.push_unit(
                    CursorUnit::text(
                        grapheme,
                        if under_cursor {
                            self.cursor_style
                        } else {
                            Style::new()
                        },
                    ),
                    content_width,
                    &mut rows,
                );
                if under_cursor {
                    cursor_row = rows.len();
                }
                cursor_rendered |= under_cursor;
            }
        }

        if !cursor_rendered {
            row.push_unit(
                CursorUnit::space(self.cursor_style),
                content_width,
                &mut rows,
            );
            cursor_row = rows.len();
        }

        rows.push(row.finish());
        ContentLayout { rows, cursor_row }
    }
}

impl PartialEq for Textarea {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.cursor == other.cursor
            && self.prompt == other.prompt
            && self.prompt_width == other.prompt_width
            && self.prompt_style == other.prompt_style
            && self.gap == other.gap
            && self.cursor_style == other.cursor_style
            && self.placeholder == other.placeholder
            && self.placeholder_style == other.placeholder_style
            && self.max_height == other.max_height
            && self.scroll_row.get() == other.scroll_row.get()
            && self.last_render_width.get() == other.last_render_width.get()
            && self.preferred_visual_column.get() == other.preferred_visual_column.get()
    }
}

impl Eq for Textarea {}

impl Default for Textarea {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for Textarea {
    fn render(&self, width: u16) -> Text {
        let width = usize::from(width);
        if width == 0 || width < self.prompt_width {
            return Text::empty();
        }

        Text::from_lines(self.rendered_lines(width))
    }
}

struct ContentLayout {
    rows: Vec<Vec<Span>>,
    cursor_row: usize,
}

#[derive(Debug)]
struct VisualRow {
    start: usize,
    end: usize,
    width: usize,
    cells: Vec<VisualCell>,
}

impl VisualRow {
    fn new(start: usize) -> Self {
        Self {
            start,
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

    fn column_for_cursor(&self, cursor: usize) -> usize {
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

    fn cursor_for_column(&self, column: usize) -> usize {
        for cell in &self.cells {
            if column < cell.column + cell.width {
                return if column <= cell.column {
                    cell.start
                } else {
                    cell.end
                };
            }
        }
        self.end
    }
}

#[derive(Debug)]
struct VisualCell {
    start: usize,
    end: usize,
    column: usize,
    width: usize,
}

#[derive(Default)]
struct ContentRow {
    spans: Vec<Span>,
    content: String,
    style: Style,
    width: usize,
}

impl ContentRow {
    fn push_unit(&mut self, unit: CursorUnit, content_width: usize, rows: &mut Vec<Vec<Span>>) {
        if self.width > 0 && self.width + unit.width > content_width {
            rows.push(self.finish_and_reset());
        }

        for piece in unit.pieces {
            self.push_text(&piece.content, piece.style);
        }
        self.width += unit.width;
    }

    fn push_tab(
        &mut self,
        under_cursor: bool,
        cursor_style: Style,
        content_width: usize,
        rows: &mut Vec<Vec<Span>>,
    ) {
        if under_cursor {
            self.push_unit(
                CursorUnit::tab_under_cursor(cursor_style),
                content_width,
                rows,
            );
        } else {
            self.push_unit(CursorUnit::text("    ", Style::new()), content_width, rows);
        }
    }

    fn push_text(&mut self, content: &str, style: Style) {
        if !self.content.is_empty() && self.style != style {
            self.flush();
        }
        self.style = style;
        self.content.push_str(content);
    }

    fn finish(mut self) -> Vec<Span> {
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

struct CursorUnit {
    pieces: Vec<StyledPiece>,
    width: usize,
}

impl CursorUnit {
    fn text(content: &str, style: Style) -> Self {
        Self {
            pieces: vec![StyledPiece {
                content: content.to_owned(),
                style,
            }],
            width: UnicodeWidthStr::width(content),
        }
    }

    fn space(style: Style) -> Self {
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

fn visual_rows(value: &str, content_width: usize) -> Vec<VisualRow> {
    let mut rows = Vec::new();
    let mut row = VisualRow::new(0);

    for (start, grapheme) in value.grapheme_indices(true) {
        let end = start + grapheme.len();
        if grapheme == "\n" {
            row.end = start;
            rows.push(row);
            row = VisualRow::new(end);
            continue;
        }

        let width = if grapheme == "\t" {
            4
        } else {
            UnicodeWidthStr::width(grapheme)
        };
        if row.width > 0 && row.width + width > content_width {
            row.end = start;
            rows.push(row);
            row = VisualRow::new(start);
        }
        row.push(start, end, width);
    }

    row.end = value.len();
    rows.push(row);
    rows
}

fn visual_cursor_row(rows: &[VisualRow], cursor: usize) -> usize {
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

fn previous_grapheme_boundary(value: &str, byte_index: usize) -> usize {
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

fn next_grapheme_boundary(value: &str, byte_index: usize) -> usize {
    value
        .grapheme_indices(true)
        .map(|(index, grapheme)| index + grapheme.len())
        .find(|index| *index > byte_index)
        .unwrap_or(value.len())
}

fn sanitize_input(input: &str) -> String {
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
