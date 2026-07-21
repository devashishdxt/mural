//! High-level terminal widgets.

use std::cell::Cell;

use crate::{
    editing,
    layout::{CursorTarget, Layout, WrapAffinity},
};

#[derive(Debug, Clone, Copy)]
enum VisualBoundary {
    Start,
    End,
}

/// A sanitized multiline text buffer with a grapheme-safe cursor.
///
/// The value never contains terminal escape sequences or control characters other
/// than line feeds and tabs. Carriage-return line endings are normalized to line
/// feeds. The cursor is a UTF-8 byte index that is always at an extended grapheme
/// cluster boundary and never exceeds the value's length.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Textarea {
    value: String,
    cursor: usize,
    cursor_affinity: WrapAffinity,
    preferred_visual_column: Option<usize>,
    remembered_render_width: Cell<Option<usize>>,
}

impl Textarea {
    /// Creates an empty textarea with its cursor at byte index zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the sanitized textarea value without permitting invariant-breaking mutation.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Replaces the value with sanitized `value` and resets the cursor to zero.
    ///
    /// ANSI escape sequences and unsupported controls are removed, line endings
    /// are normalized to `\n`, and tabs are preserved.
    pub fn set_value(&mut self, value: impl AsRef<str>) -> &mut Self {
        self.value = editing::sanitize(value.as_ref());
        self.cursor = 0;
        self.reset_after_replacement();
        self
    }

    /// Removes the value and resets the cursor to zero.
    pub fn clear(&mut self) -> &mut Self {
        self.value.clear();
        self.cursor = 0;
        self.reset_after_replacement();
        self
    }

    /// Removes and returns the sanitized value, resetting the cursor to zero.
    pub fn take(&mut self) -> String {
        let value = std::mem::take(&mut self.value);
        self.cursor = 0;
        self.reset_after_replacement();
        value
    }

    /// Returns whether the textarea value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns the cursor as a UTF-8 byte index into [`Self::value`].
    ///
    /// The returned index is always an extended grapheme cluster boundary.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Sets the cursor, clamping backward to a legal grapheme boundary.
    ///
    /// Indices past the value clamp to its end. An in-bounds index inside an
    /// extended grapheme cluster clamps to that cluster's start.
    pub fn set_cursor(&mut self, cursor: usize) -> &mut Self {
        self.cursor = editing::clamp_cursor(&self.value, cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Inserts sanitized text at the cursor and leaves it on a legal boundary.
    ///
    /// Sanitization follows the same contract as [`Self::set_value`].
    pub fn insert(&mut self, input: impl AsRef<str>) -> &mut Self {
        self.cursor = editing::insert(&mut self.value, self.cursor, input.as_ref());
        self.reset_after_direct_cursor_change();
        self
    }

    /// Inserts a sanitized character at the cursor and leaves it on a legal boundary.
    ///
    /// Unsupported control characters are ignored. A carriage return or line feed
    /// is normalized to a line feed; use [`Self::insert_newline`] when that intent
    /// should be explicit.
    pub fn insert_char(&mut self, character: char) -> &mut Self {
        self.cursor = editing::insert_character(&mut self.value, self.cursor, character);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Inserts one normalized line feed at the cursor.
    pub fn insert_newline(&mut self) -> &mut Self {
        self.cursor = editing::insert_newline(&mut self.value, self.cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Removes the complete grapheme cluster before the cursor.
    ///
    /// This is a no-op when the cursor is at the start of the value.
    pub fn backspace(&mut self) -> &mut Self {
        self.cursor = editing::backspace(&mut self.value, self.cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Removes the complete grapheme cluster at the cursor.
    ///
    /// This forward-delete operation is a no-op at the end of the value.
    pub fn delete(&mut self) -> &mut Self {
        self.cursor = editing::delete_forward(&mut self.value, self.cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Moves the cursor left by one grapheme using the last remembered render width.
    ///
    /// Before a width has been remembered, navigation is unwrapped except at
    /// explicit newlines. A remembered zero width uses a one-column fallback.
    pub fn move_left(&mut self) -> &mut Self {
        self.move_left_with_width(self.navigation_width())
    }

    /// Moves the cursor left by one grapheme using only `width` for layout.
    pub fn move_left_with_width(&mut self, width: usize) -> &mut Self {
        let layout = Layout::new(&self.value, Self::navigation_layout_width(width));
        if self.cursor_affinity == WrapAffinity::NextRow
            && Self::is_soft_wrap_boundary(&layout, self.cursor)
        {
            self.cursor_affinity = WrapAffinity::PreviousRow;
        } else {
            self.cursor = editing::previous_grapheme(&self.value, self.cursor);
            self.cursor_affinity = WrapAffinity::NextRow;
        }
        self.preferred_visual_column = None;
        self
    }

    /// Moves the cursor right by one grapheme using the last remembered render width.
    ///
    /// At a soft wrap this may first move from the preceding row's end to the
    /// following row's start without changing the public byte cursor.
    pub fn move_right(&mut self) -> &mut Self {
        self.move_right_with_width(self.navigation_width())
    }

    /// Moves the cursor right by one grapheme using only `width` for layout.
    pub fn move_right_with_width(&mut self, width: usize) -> &mut Self {
        let layout = Layout::new(&self.value, Self::navigation_layout_width(width));
        if self.cursor_affinity == WrapAffinity::PreviousRow
            && Self::is_soft_wrap_boundary(&layout, self.cursor)
        {
            self.cursor_affinity = WrapAffinity::NextRow;
        } else if self.cursor < self.value.len() {
            self.cursor = editing::next_grapheme(&self.value, self.cursor);
            self.cursor_affinity = if Self::is_soft_wrap_boundary(&layout, self.cursor) {
                WrapAffinity::PreviousRow
            } else {
                WrapAffinity::NextRow
            };
        }
        self.preferred_visual_column = None;
        self
    }

    /// Moves the cursor to the start of the current or preceding Unicode word.
    pub fn move_word_left(&mut self) -> &mut Self {
        self.cursor = editing::previous_word(&self.value, self.cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Moves the cursor to the end of the current or following Unicode word.
    pub fn move_word_right(&mut self) -> &mut Self {
        self.cursor = editing::next_word(&self.value, self.cursor);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Moves the cursor to the start of its current explicit source line.
    pub fn move_to_line_start(&mut self) -> &mut Self {
        self.cursor = self.value[..self.cursor]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        self.reset_after_direct_cursor_change();
        self
    }

    /// Moves the cursor to the end of its current explicit source line.
    pub fn move_to_line_end(&mut self) -> &mut Self {
        self.move_to_line_end_with_width(self.navigation_width())
    }

    /// Moves to the current source-line end using only `width` for layout.
    pub fn move_to_line_end_with_width(&mut self, width: usize) -> &mut Self {
        self.cursor = self.value[self.cursor..]
            .find('\n')
            .map_or(self.value.len(), |offset| self.cursor + offset);
        self.set_boundary_affinity(width);
        self.preferred_visual_column = None;
        self
    }

    /// Moves the cursor to the start of the complete buffer.
    pub fn move_to_buffer_start(&mut self) -> &mut Self {
        self.cursor = 0;
        self.reset_after_direct_cursor_change();
        self
    }

    /// Moves the cursor to the end of the complete buffer.
    pub fn move_to_buffer_end(&mut self) -> &mut Self {
        self.move_to_buffer_end_with_width(self.navigation_width())
    }

    /// Moves to the complete-buffer end using only `width` for layout.
    pub fn move_to_buffer_end_with_width(&mut self, width: usize) -> &mut Self {
        self.cursor = self.value.len();
        self.set_boundary_affinity(width);
        self.preferred_visual_column = None;
        self
    }

    /// Moves the cursor one visual row up using the last remembered render width.
    pub fn move_visual_up(&mut self) -> &mut Self {
        self.move_visual_up_with_width(self.navigation_width())
    }

    /// Moves the cursor one visual row up using only `width` for layout.
    pub fn move_visual_up_with_width(&mut self, width: usize) -> &mut Self {
        self.move_visual_rows(width, -1)
    }

    /// Moves the cursor one visual row down using the last remembered render width.
    pub fn move_visual_down(&mut self) -> &mut Self {
        self.move_visual_down_with_width(self.navigation_width())
    }

    /// Moves the cursor one visual row down using only `width` for layout.
    pub fn move_visual_down_with_width(&mut self, width: usize) -> &mut Self {
        self.move_visual_rows(width, 1)
    }

    /// Moves the cursor to the start of its current visual row.
    pub fn move_to_visual_row_start(&mut self) -> &mut Self {
        self.move_to_visual_row_start_with_width(self.navigation_width())
    }

    /// Moves to the current visual-row start using only `width` for layout.
    pub fn move_to_visual_row_start_with_width(&mut self, width: usize) -> &mut Self {
        self.move_to_visual_boundary(width, VisualBoundary::Start)
    }

    /// Moves the cursor to the end of its current visual row.
    pub fn move_to_visual_row_end(&mut self) -> &mut Self {
        self.move_to_visual_row_end_with_width(self.navigation_width())
    }

    /// Moves to the current visual-row end using only `width` for layout.
    pub fn move_to_visual_row_end_with_width(&mut self, width: usize) -> &mut Self {
        self.move_to_visual_boundary(width, VisualBoundary::End)
    }

    fn move_visual_rows(&mut self, width: usize, delta: isize) -> &mut Self {
        let layout = Layout::new(&self.value, Self::navigation_layout_width(width));
        let Some(position) = layout.source_to_visual(self.cursor, self.cursor_affinity) else {
            return self;
        };
        let preferred_column = self.preferred_visual_column.unwrap_or(position.column);
        let target_row = position
            .row
            .saturating_add_signed(delta)
            .min(layout.rows().len().saturating_sub(1));

        if target_row != position.row
            && let Some(target) = layout.visual_to_source(target_row, preferred_column)
        {
            self.apply_target(target);
        }
        self.preferred_visual_column = Some(preferred_column);
        self
    }

    fn move_to_visual_boundary(&mut self, width: usize, boundary: VisualBoundary) -> &mut Self {
        let layout = Layout::new(&self.value, Self::navigation_layout_width(width));
        if let Some(position) = layout.source_to_visual(self.cursor, self.cursor_affinity) {
            let target = match boundary {
                VisualBoundary::Start => layout.row_start_target(position.row),
                VisualBoundary::End => layout.row_end_target(position.row),
            };
            if let Some(target) = target {
                self.apply_target(target);
            }
        }
        self.preferred_visual_column = None;
        self
    }

    fn apply_target(&mut self, target: CursorTarget) {
        self.cursor = target.cursor;
        self.cursor_affinity = target.affinity;
    }

    fn set_boundary_affinity(&mut self, width: usize) {
        let layout = Layout::new(&self.value, Self::navigation_layout_width(width));
        self.cursor_affinity = if Self::is_soft_wrap_boundary(&layout, self.cursor) {
            WrapAffinity::PreviousRow
        } else {
            WrapAffinity::NextRow
        };
    }

    fn is_soft_wrap_boundary(layout: &Layout<'_>, cursor: usize) -> bool {
        layout
            .rows()
            .iter()
            .any(|row| row.soft_boundary() == Some(cursor))
    }

    fn navigation_layout_width(width: usize) -> usize {
        width.max(1)
    }

    fn navigation_width(&self) -> usize {
        self.remembered_render_width.get().unwrap_or(usize::MAX)
    }

    #[allow(
        dead_code,
        reason = "rendering is delivered by the dependent software-cursor ticket"
    )]
    pub(crate) fn remember_render_width(&self, width: usize) {
        self.remembered_render_width.set(Some(width));
    }

    fn reset_after_direct_cursor_change(&mut self) {
        self.cursor_affinity = WrapAffinity::NextRow;
        self.preferred_visual_column = None;
    }

    fn reset_after_replacement(&mut self) {
        self.reset_after_direct_cursor_change();
    }
}

impl From<String> for Textarea {
    fn from(value: String) -> Self {
        let mut textarea = Self::new();
        textarea.set_value(value);
        textarea
    }
}

impl From<&str> for Textarea {
    fn from(value: &str) -> Self {
        let mut textarea = Self::new();
        textarea.set_value(value);
        textarea
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_navigation_exposes_both_soft_wrap_affinities() {
        let mut textarea = Textarea::from("abcd");
        textarea.set_cursor(2).move_right_with_width(4);
        assert_cursor(&textarea, 3, WrapAffinity::PreviousRow);

        textarea.move_right_with_width(4);
        assert_cursor(&textarea, 3, WrapAffinity::NextRow);

        textarea.move_left_with_width(4);
        assert_cursor(&textarea, 3, WrapAffinity::PreviousRow);

        textarea.move_left_with_width(4);
        assert_cursor(&textarea, 2, WrapAffinity::NextRow);

        let mut full_width_end = Textarea::from("界");
        full_width_end.set_cursor(usize::MAX);
        let unchanged = full_width_end.clone();
        full_width_end.move_right_with_width(2);
        assert_eq!(full_width_end, unchanged);

        full_width_end.move_to_buffer_end_with_width(2);
        assert_cursor(&full_width_end, "界".len(), WrapAffinity::PreviousRow);
        full_width_end.move_right_with_width(2);
        assert_cursor(&full_width_end, "界".len(), WrapAffinity::NextRow);
    }

    #[test]
    fn remembered_width_drives_widthless_navigation_and_survives_values() {
        let mut textarea = Textarea::from("abcdef");
        textarea.remember_render_width(4);
        textarea.move_visual_down_with_width(7);
        assert_eq!(textarea.cursor(), 0);
        assert_eq!(textarea.remembered_render_width.get(), Some(4));
        textarea.move_visual_down();
        assert_eq!(textarea.cursor(), 3);
        textarea.move_visual_up();
        assert_eq!(textarea.cursor(), 0);

        textarea.set_cursor(4).move_to_visual_row_start();
        assert_eq!(textarea.cursor(), 3);
        textarea.move_to_visual_row_end();
        assert_eq!(textarea.cursor(), 6);

        textarea.set_value("abcdef\nx").move_to_line_end();
        assert_eq!(textarea.cursor(), 6);
        textarea.set_value("abcdef").move_visual_down();
        assert_eq!(textarea.cursor(), 3);
        textarea.clear();
        assert_eq!(textarea.remembered_render_width.get(), Some(4));

        textarea.set_value("abcdef");
        assert_eq!(textarea.take(), "abcdef");
        assert_eq!(textarea.remembered_render_width.get(), Some(4));
    }

    #[test]
    fn remembered_zero_width_uses_one_column_only_for_navigation() {
        let mut textarea = Textarea::from("a\nb");
        textarea.remember_render_width(0);
        textarea.move_visual_down();

        assert_eq!(textarea.cursor(), 2);
        assert_eq!(textarea.remembered_render_width.get(), Some(0));
    }

    #[test]
    fn equality_includes_affinity_preferred_column_and_remembered_width() {
        let mut baseline = Textarea::from("abcd");
        baseline.set_cursor(2);

        let mut affinity = baseline.clone();
        affinity.move_right_with_width(4);
        let mut next_row = affinity.clone();
        next_row.move_right_with_width(4);
        assert_eq!(affinity.cursor(), next_row.cursor());
        assert_ne!(affinity, next_row);

        let mut preferred = baseline.clone();
        preferred.move_visual_up_with_width(4);
        assert_eq!(baseline.cursor(), preferred.cursor());
        assert_eq!(baseline.cursor_affinity, preferred.cursor_affinity);
        assert_ne!(baseline, preferred);

        let remembered = baseline.clone();
        remembered.remember_render_width(4);
        assert_ne!(baseline, remembered);
    }

    fn assert_cursor(textarea: &Textarea, cursor: usize, affinity: WrapAffinity) {
        assert_eq!(textarea.cursor(), cursor);
        assert_eq!(textarea.cursor_affinity, affinity);
    }
}
