use std::cell::Cell;

use unicode_width::UnicodeWidthStr;

use crate::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyOutcome, Line, Render, Span, Style, Text,
    TextError,
};

use self::{
    editing::{
        next_grapheme_boundary, next_word_end, previous_grapheme_boundary, previous_word_start,
        sanitize_input, text_insertion_modifiers,
    },
    rendering::{ContentRow, CursorUnit, RenderCursor, push_rendered_fragments},
    source::{
        WrappedSourceRow, editable_wrap_width, last_rendered_grapheme_end,
        rendered_source_row_width, wrapped_source_rows,
    },
    visual::{VisualRow, visual_cursor_row, visual_rows},
};
use super::{layout::push_spaces, validation::validate_non_empty_display_text};

mod editing;
mod rendering;
mod source;
mod visual;

const DEFAULT_PROMPT: &str = "›";
const DEFAULT_GAP: usize = 1;
const DEFAULT_MAX_HEIGHT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorAffinity {
    Default,
    PreviousVisualRow { cursor: usize },
}

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
    scroll_width: Cell<Option<usize>>,
    last_rendered_width: Cell<Option<usize>>,
    preferred_visual_column: Cell<Option<usize>>,
    cursor_affinity: CursorAffinity,
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
            scroll_width: Cell::new(None),
            last_rendered_width: Cell::new(None),
            preferred_visual_column: Cell::new(None),
            cursor_affinity: CursorAffinity::Default,
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
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_scroll();
        self.reset_preferred_visual_column();
        self
    }

    /// Clears the editable buffer and cursor state.
    pub fn clear(&mut self) -> &mut Self {
        self.value.clear();
        self.cursor = 0;
        self.cursor_affinity = CursorAffinity::Default;
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
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Inserts sanitized text at the cursor.
    pub fn insert_str(&mut self, value: impl AsRef<str>) -> &mut Self {
        let value = sanitize_input(value.as_ref());
        self.value.insert_str(self.cursor, &value);
        self.cursor += value.len();
        self.cursor_affinity = CursorAffinity::Default;
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
        self.cursor_affinity = CursorAffinity::Default;
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
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor left by one grapheme.
    pub fn move_left(&mut self) -> &mut Self {
        self.move_left_with_width(self.navigation_width())
    }

    fn move_left_with_width(&mut self, width: u16) -> &mut Self {
        if self.cursor_affinity == CursorAffinity::Default
            && self.is_cursor_at_soft_wrap_boundary(width)
        {
            self.cursor_affinity = CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            };
        } else {
            if self.cursor > 0 {
                self.cursor =
                    previous_grapheme_boundary(&self.value, self.cursor.saturating_sub(1));
            }
            self.cursor_affinity = CursorAffinity::Default;
        }
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor right by one grapheme.
    pub fn move_right(&mut self) -> &mut Self {
        self.move_right_with_width(self.navigation_width())
    }

    fn move_right_with_width(&mut self, width: u16) -> &mut Self {
        if self.cursor_affinity
            == (CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            })
            && self.is_cursor_at_soft_wrap_boundary(width)
        {
            self.cursor_affinity = CursorAffinity::Default;
        } else {
            if self.cursor < self.value.len() {
                self.cursor = next_grapheme_boundary(&self.value, self.cursor);
            }
            self.set_previous_visual_row_affinity_at_soft_boundary(width);
        }
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the start of the previous word.
    pub fn move_word_left(&mut self) -> &mut Self {
        self.cursor = previous_word_start(&self.value, self.cursor);
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the next word.
    pub fn move_word_right(&mut self) -> &mut Self {
        self.cursor = next_word_end(&self.value, self.cursor);
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the start of the current source line.
    pub fn move_to_line_start(&mut self) -> &mut Self {
        self.cursor = self.value[..self.cursor]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the current source line using the last rendered width.
    pub fn move_to_line_end(&mut self) -> &mut Self {
        self.move_to_line_end_with_width(self.navigation_width())
    }

    /// Moves the cursor to the end of the current source line for the given textarea render width.
    pub fn move_to_line_end_with_width(&mut self, width: u16) -> &mut Self {
        self.cursor = self.value[self.cursor..]
            .find('\n')
            .map(|offset| self.cursor + offset)
            .unwrap_or(self.value.len());
        self.set_full_visual_row_end_affinity(width);
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the start of the buffer.
    pub fn move_to_buffer_start(&mut self) -> &mut Self {
        self.cursor = 0;
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the buffer using the last rendered width.
    pub fn move_to_buffer_end(&mut self) -> &mut Self {
        self.move_to_buffer_end_with_width(self.navigation_width())
    }

    /// Moves the cursor to the end of the buffer for the given textarea render width.
    pub fn move_to_buffer_end_with_width(&mut self, width: u16) -> &mut Self {
        self.cursor = self.value.len();
        self.set_full_visual_row_end_affinity(width);
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

    /// Moves the cursor one visual row up using the last rendered width.
    pub fn move_visual_up(&mut self) -> &mut Self {
        self.move_visual_up_with_width(self.navigation_width())
    }

    /// Moves the cursor one visual row up for the given textarea render width.
    pub fn move_visual_up_with_width(&mut self, width: u16) -> &mut Self {
        self.move_visual_rows(width, -1)
    }

    /// Moves the cursor one visual row down using the last rendered width.
    pub fn move_visual_down(&mut self) -> &mut Self {
        self.move_visual_down_with_width(self.navigation_width())
    }

    /// Moves the cursor one visual row down for the given textarea render width.
    pub fn move_visual_down_with_width(&mut self, width: u16) -> &mut Self {
        self.move_visual_rows(width, 1)
    }

    /// Moves the cursor to the start of the current visual row using the last rendered width.
    pub fn move_to_visual_row_start(&mut self) -> &mut Self {
        self.move_to_visual_row_start_with_width(self.navigation_width())
    }

    /// Moves the cursor to the start of the current visual row for the given textarea render width.
    pub fn move_to_visual_row_start_with_width(&mut self, width: u16) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        let index = self.visual_cursor_row_for_navigation(&rows);
        self.cursor = rows.get(index).map(|row| row.start).unwrap_or(0);
        self.cursor_affinity = CursorAffinity::Default;
        self.reset_preferred_visual_column();
        self
    }

    /// Moves the cursor to the end of the current visual row using the last rendered width.
    pub fn move_to_visual_row_end(&mut self) -> &mut Self {
        self.move_to_visual_row_end_with_width(self.navigation_width())
    }

    /// Moves the cursor to the end of the current visual row for the given textarea render width.
    pub fn move_to_visual_row_end_with_width(&mut self, width: u16) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        let index = self.visual_cursor_row_for_navigation(&rows);
        self.cursor = rows
            .get(index)
            .map(|row| row.visible_end)
            .unwrap_or(self.value.len());
        self.set_full_visual_row_end_affinity_for_rows(&rows, index, width);
        self.reset_preferred_visual_column();
        self
    }

    /// Applies Mural's default textarea behavior for one key event.
    ///
    /// Wrapped-row movement uses the last width passed to [`Textarea::render`].
    /// Before the first render, movement falls back to an effectively unwrapped
    /// width. Press and repeat events are handled the same way. Release events
    /// are ignored.
    ///
    /// Default behavior includes text insertion, grapheme-aware deletion,
    /// visual-row arrow movement, word movement with control/alt arrows and
    /// alt-B/alt-F terminal aliases, readline-style control-A/control-E
    /// source-line movement, tab insertion, `Enter` submission, and newline
    /// insertion with alt-enter or shift-enter.
    /// Pre-handle application shortcuts before calling this method when you need
    /// custom behavior.
    pub fn handle_key_event(&mut self, event: impl Into<KeyEvent>) -> KeyOutcome {
        self.handle_key_event_with_width(event, self.navigation_width())
    }

    /// Applies Mural's default textarea behavior for one key event using an explicit width.
    ///
    /// `width` should match the width used to render the textarea; wrapped-row
    /// movement depends on it. Prefer [`Textarea::handle_key_event`] for normal
    /// applications.
    pub fn handle_key_event_with_width(
        &mut self,
        event: impl Into<KeyEvent>,
        width: u16,
    ) -> KeyOutcome {
        let event = event.into();
        let modifiers = event.modifiers();
        let code = event.code();

        match event.kind_value() {
            KeyEventKind::Release => return KeyOutcome::Ignored,
            KeyEventKind::Press | KeyEventKind::Repeat => {}
        }

        match code {
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(|textarea| {
                    textarea.move_to_line_start();
                })
            }
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(|textarea| {
                    textarea.move_to_line_end_with_width(width);
                })
            }
            KeyCode::Char(ch) if text_insertion_modifiers(modifiers) => {
                self.changed_by(|textarea| {
                    textarea.insert_char(ch);
                })
            }
            KeyCode::Enter if modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) => self
                .changed_by(|textarea| {
                    textarea.insert_newline();
                }),
            KeyCode::Enter => KeyOutcome::Submit,
            KeyCode::Backspace => self.changed_by(|textarea| {
                textarea.backspace();
            }),
            KeyCode::Delete => self.changed_by(|textarea| {
                textarea.delete();
            }),
            KeyCode::Char('b' | 'B') if modifiers.contains(KeyModifiers::ALT) => {
                self.changed_by(|textarea| {
                    textarea.move_word_left();
                })
            }
            KeyCode::Char('f' | 'F') if modifiers.contains(KeyModifiers::ALT) => {
                self.changed_by(|textarea| {
                    textarea.move_word_right();
                })
            }
            KeyCode::Left if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.changed_by(|textarea| {
                    textarea.move_word_left();
                })
            }
            KeyCode::Right if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.changed_by(|textarea| {
                    textarea.move_word_right();
                })
            }
            KeyCode::Left => self.changed_by(|textarea| {
                textarea.move_left_with_width(width);
            }),
            KeyCode::Right => self.changed_by(|textarea| {
                textarea.move_right_with_width(width);
            }),
            KeyCode::Up => self.changed_by(|textarea| {
                textarea.move_visual_up_with_width(width);
            }),
            KeyCode::Down => self.changed_by(|textarea| {
                textarea.move_visual_down_with_width(width);
            }),
            KeyCode::Home if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(|textarea| {
                    textarea.move_to_buffer_start();
                })
            }
            KeyCode::End if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(|textarea| {
                    textarea.move_to_buffer_end_with_width(width);
                })
            }
            KeyCode::Home => self.changed_by(|textarea| {
                textarea.move_to_visual_row_start_with_width(width);
            }),
            KeyCode::End => self.changed_by(|textarea| {
                textarea.move_to_visual_row_end_with_width(width);
            }),
            KeyCode::Tab => self.changed_by(|textarea| {
                textarea.insert_char('\t');
            }),
            KeyCode::Char(_)
            | KeyCode::BackTab
            | KeyCode::Esc
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Unsupported => KeyOutcome::Ignored,
        }
    }

    fn move_visual_rows(&mut self, width: u16, delta: isize) -> &mut Self {
        let rows = self.visual_rows_for_width(width);
        if rows.is_empty() {
            return self;
        }

        let current_row = self.visual_cursor_row_for_navigation(&rows);
        let current_column = rows[current_row].column_for_cursor(self.cursor);
        let preferred_column = self.preferred_visual_column.get().unwrap_or(current_column);
        let target_row = current_row
            .saturating_add_signed(delta)
            .min(rows.len().saturating_sub(1));

        self.cursor = rows[target_row].cursor_for_column(preferred_column);
        if target_row == current_row {
            self.cursor_affinity = CursorAffinity::Default;
        } else {
            self.set_full_visual_row_end_affinity_for_rows(&rows, target_row, width);
        }
        self.preferred_visual_column.set(Some(preferred_column));
        self
    }

    fn visual_cursor_row_for_navigation(&self, rows: &[VisualRow]) -> usize {
        if self.cursor_affinity
            == (CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            })
            && let Some(index) = rows.iter().enumerate().position(|(index, row)| {
                row.end == self.cursor
                    && rows
                        .get(index + 1)
                        .is_some_and(|next| next.start == self.cursor)
            })
        {
            return index;
        }

        visual_cursor_row(rows, self.cursor)
    }

    fn visual_rows_for_width(&self, width: u16) -> Vec<VisualRow> {
        let width = usize::from(width);
        let content_width = width.saturating_sub(self.prefix_width()).max(1);
        visual_rows(&self.value, editable_wrap_width(content_width))
    }

    fn set_full_visual_row_end_affinity(&mut self, width: u16) {
        let rows = self.visual_rows_for_width(width);
        let index = visual_cursor_row(&rows, self.cursor);
        self.set_full_visual_row_end_affinity_for_rows(&rows, index, width);
    }

    fn set_previous_visual_row_affinity_at_soft_boundary(&mut self, width: u16) {
        self.cursor_affinity = if self.is_cursor_at_soft_wrap_boundary(width) {
            CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            }
        } else {
            CursorAffinity::Default
        };
    }

    fn is_cursor_at_soft_wrap_boundary(&self, width: u16) -> bool {
        let rows = self.visual_rows_for_width(width);
        rows.iter().enumerate().any(|(index, row)| {
            row.cells.last().is_some_and(|cell| cell.end == self.cursor)
                && rows
                    .get(index + 1)
                    .is_some_and(|next| next.start == self.cursor)
        })
    }

    fn set_full_visual_row_end_affinity_for_rows(
        &mut self,
        rows: &[VisualRow],
        index: usize,
        width: u16,
    ) {
        self.cursor_affinity = if self.should_prefer_previous_visual_row_end(rows, index, width) {
            CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            }
        } else {
            CursorAffinity::Default
        };
    }

    fn should_prefer_previous_visual_row_end(
        &self,
        rows: &[VisualRow],
        index: usize,
        width: u16,
    ) -> bool {
        let width = usize::from(width);
        let content_width = width.saturating_sub(self.prefix_width()).max(1);
        rows.get(index).is_some_and(|row| {
            row.end == self.cursor
                && !row.cells.is_empty()
                && (row.width >= content_width
                    || rows
                        .get(index + 1)
                        .is_some_and(|next| next.start == self.cursor))
        })
    }

    fn changed_by(&mut self, action: impl FnOnce(&mut Self)) -> KeyOutcome {
        let before_value = self.value.clone();
        let before_cursor = self.cursor;
        let before_cursor_affinity = self.cursor_affinity;
        action(self);
        if self.value == before_value
            && self.cursor == before_cursor
            && self.cursor_affinity == before_cursor_affinity
        {
            KeyOutcome::Unchanged
        } else {
            KeyOutcome::Changed
        }
    }

    fn reset_scroll(&self) {
        self.scroll_row.set(0);
        self.scroll_width.set(None);
    }

    fn navigation_width(&self) -> u16 {
        self.last_rendered_width
            .get()
            .map(|width| width as u16)
            .unwrap_or(u16::MAX)
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
        if content_width <= 1 {
            return vec![self.first_prefix_line(self.gap)];
        }

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
            self.scroll_width.set(Some(width));
            self.scroll_row.set(0);
            return 0;
        };

        if row_count <= max_height {
            self.scroll_width.set(Some(width));
            self.scroll_row.set(0);
            return 0;
        }

        let max_scroll = row_count - max_height;
        let mut scroll_row = self.scroll_row.get().min(max_scroll);
        if self.scroll_width.get() != Some(width) {
            scroll_row = scroll_row.min(max_scroll);
        }

        if cursor_row < scroll_row {
            scroll_row = cursor_row;
        } else if cursor_row >= scroll_row + max_height {
            scroll_row = cursor_row + 1 - max_height;
        }
        scroll_row = scroll_row.min(max_scroll);

        self.scroll_width.set(Some(width));
        self.scroll_row.set(scroll_row);
        scroll_row
    }

    fn content_layout(&self, content_width: usize) -> ContentLayout {
        if self.value.is_empty() {
            self.placeholder_layout(content_width)
        } else {
            self.editable_content_layout(content_width)
        }
    }

    fn placeholder_layout(&self, content_width: usize) -> ContentLayout {
        let mut rows = Vec::new();
        let mut row = ContentRow::default();
        row.push_unit(
            CursorUnit::space(self.cursor_style),
            content_width,
            &mut rows,
        );

        if let Some(placeholder) = &self.placeholder {
            self.push_placeholder_rows(placeholder, content_width, &mut rows, &mut row);
        }

        rows.push(row.finish());
        ContentLayout {
            rows,
            cursor_row: 0,
        }
    }

    fn push_placeholder_rows(
        &self,
        placeholder: &str,
        content_width: usize,
        rows: &mut Vec<Vec<Span>>,
        row: &mut ContentRow,
    ) {
        let placeholder_width = content_width.saturating_sub(1).max(1);
        let placeholder_rows = wrapped_source_rows(placeholder, placeholder_width);
        let mut placeholder_cursor = RenderCursor::new(usize::MAX, self.cursor_style);

        for (index, placeholder_row) in placeholder_rows.iter().enumerate() {
            if index > 0 {
                rows.push(std::mem::take(row).finish());
            }
            push_rendered_fragments(
                row,
                placeholder,
                &placeholder_row.fragments,
                self.placeholder_style,
                &mut placeholder_cursor,
            );
        }
    }

    fn editable_content_layout(&self, content_width: usize) -> ContentLayout {
        let source_rows = wrapped_source_rows(&self.value, editable_wrap_width(content_width));
        let mut rows = Vec::new();
        let mut render_cursor = RenderCursor::new(self.cursor, self.cursor_style);
        let mut cursor_row = 0;

        for (index, source_row) in source_rows.iter().enumerate() {
            let cursor_rendered_before_row = render_cursor.rendered;
            let prefers_previous_row = self.row_prefers_previous_visual_row(source_row);
            let mut row = self.render_source_row(source_row, content_width, &mut render_cursor);

            if !cursor_rendered_before_row && render_cursor.rendered {
                cursor_row = rows.len();
            }

            if self.should_render_cursor_after_source_row(
                source_row,
                source_rows.get(index + 1),
                prefers_previous_row,
                &render_cursor,
            ) {
                row.push_unit(
                    CursorUnit::space(self.cursor_style),
                    content_width,
                    &mut rows,
                );
                cursor_row = rows.len();
                render_cursor.rendered = true;
            }

            rows.push(row.finish());
        }

        ContentLayout { rows, cursor_row }
    }

    fn render_source_row(
        &self,
        source_row: &WrappedSourceRow,
        content_width: usize,
        render_cursor: &mut RenderCursor,
    ) -> ContentRow {
        let prefers_previous_row = self.row_prefers_previous_visual_row(source_row);
        render_cursor.overlay_grapheme_end = if prefers_previous_row
            && rendered_source_row_width(&self.value, source_row) >= content_width
        {
            last_rendered_grapheme_end(&self.value, source_row)
        } else {
            None
        };

        let mut row = ContentRow::default();
        push_rendered_fragments(
            &mut row,
            &self.value,
            &source_row.fragments,
            Style::new(),
            render_cursor,
        );
        row
    }

    fn row_prefers_previous_visual_row(&self, source_row: &WrappedSourceRow) -> bool {
        self.cursor_affinity
            == (CursorAffinity::PreviousVisualRow {
                cursor: self.cursor,
            })
            && source_row.end == self.cursor
    }

    fn should_render_cursor_after_source_row(
        &self,
        source_row: &WrappedSourceRow,
        next_row: Option<&WrappedSourceRow>,
        prefers_previous_row: bool,
        render_cursor: &RenderCursor,
    ) -> bool {
        let next_starts_at_cursor = next_row.is_some_and(|next| next.start == self.cursor);
        !render_cursor.rendered
            && self.cursor >= source_row.start
            && self.cursor <= source_row.end
            && !(self.cursor == source_row.end && next_starts_at_cursor && !prefers_previous_row)
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
            && self.scroll_width.get() == other.scroll_width.get()
            && self.last_rendered_width.get() == other.last_rendered_width.get()
            && self.preferred_visual_column.get() == other.preferred_visual_column.get()
            && self.cursor_affinity == other.cursor_affinity
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
        self.last_rendered_width.set(Some(usize::from(width)));
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
