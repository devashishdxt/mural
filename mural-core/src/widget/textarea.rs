//! High-level terminal widgets.
//!
//! [`Textarea`] separates logical editing from rendering. Widthless visual operations use the
//! width remembered by its most recent [`Block::render`] call; each `_with_width` variant uses its
//! argument for one operation and leaves the remembered width unchanged. Before the first render,
//! widthless navigation treats text as unwrapped except at explicit line feeds.

mod editing;
mod layout;
mod rendering;

use std::{borrow::Cow, cell::Cell};

use self::{
    layout::{CursorTarget, Layout, WrapAffinity},
    rendering::MaximumHeight,
};
use crate::{
    Block, RenderContext,
    key::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyOutcome},
};

const COMMAND_MODIFIERS: KeyModifiers = KeyModifiers::ALT
    .union(KeyModifiers::CONTROL)
    .union(KeyModifiers::SUPER)
    .union(KeyModifiers::META)
    .union(KeyModifiers::HYPER);
const CONTROL_SHORTCUT_CONFLICTS: KeyModifiers = KeyModifiers::ALT
    .union(KeyModifiers::SUPER)
    .union(KeyModifiers::META)
    .union(KeyModifiers::HYPER);
const ALT_SHORTCUT_CONFLICTS: KeyModifiers = KeyModifiers::CONTROL
    .union(KeyModifiers::SUPER)
    .union(KeyModifiers::META)
    .union(KeyModifiers::HYPER);

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
///
/// As a [`Block`], the textarea wraps to the render context's width, remembers that width for later
/// widthless navigation and key handling, and limits output to six visual rows by default. The
/// viewport follows the cursor. [`Self::max_height`] changes that limit and
/// [`Self::unlimited_height`] removes it.
///
/// Rendering includes a fixed reverse-video software block cursor. The cursor covers the grapheme
/// under the byte cursor, or a reserved space at an empty or end-of-value position. The effect is
/// reset on the same output line and user-supplied terminal escapes cannot reach rendered output
/// because all value entry points sanitize their input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Textarea {
    value: String,
    cursor: usize,
    cursor_affinity: WrapAffinity,
    preferred_visual_column: Option<usize>,
    maximum_height: MaximumHeight,
    scroll_row: Cell<usize>,
    scroll_width: Cell<Option<usize>>,
    remembered_render_width: Cell<Option<usize>>,
}

impl Textarea {
    /// Creates an empty textarea with its cursor at byte index zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constrains rendering to at most `maximum` visual rows.
    ///
    /// A maximum of zero is clamped to one.
    #[must_use]
    pub fn max_height(mut self, maximum: usize) -> Self {
        self.maximum_height = MaximumHeight::Limited(maximum.max(1));
        self.reset_viewport();
        self
    }

    /// Removes the visual-row limit so every laid-out row is rendered.
    #[must_use]
    pub fn unlimited_height(mut self) -> Self {
        self.maximum_height = MaximumHeight::Unlimited;
        self.reset_viewport();
        self
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

    /// Applies the fixed default textarea behavior for a semantic key event.
    ///
    /// Press and repeat events are handled identically, while releases are ignored. Width-aware
    /// movement uses the most recently rendered width. Applications can pre-handle custom
    /// shortcuts and call the public editing or navigation primitives instead.
    pub fn handle_key_event(&mut self, event: impl Into<KeyEvent>) -> KeyOutcome {
        let event = event.into();
        if event.kind() == KeyEventKind::Release {
            return KeyOutcome::Ignored;
        }

        self.handle_pressed_key(event.code(), event.modifiers())
    }

    /// Moves the cursor left by one grapheme using the last remembered render width.
    ///
    /// Before a width has been remembered, navigation is unwrapped except at
    /// explicit newlines. A remembered zero width uses a one-column fallback.
    pub fn move_left(&mut self) -> &mut Self {
        self.move_left_with_width(self.navigation_width())
    }

    /// Moves the cursor left by one grapheme using only `width` for layout.
    ///
    /// This one-off calculation does not replace the remembered render width.
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
    ///
    /// This one-off calculation does not replace the remembered render width.
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
    ///
    /// This one-off calculation does not replace the remembered render width.
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
    ///
    /// This one-off calculation does not replace the remembered render width.
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
    ///
    /// This one-off calculation does not replace the remembered render width.
    pub fn move_visual_up_with_width(&mut self, width: usize) -> &mut Self {
        self.move_visual_rows(width, -1)
    }

    /// Moves the cursor one visual row down using the last remembered render width.
    pub fn move_visual_down(&mut self) -> &mut Self {
        self.move_visual_down_with_width(self.navigation_width())
    }

    /// Moves the cursor one visual row down using only `width` for layout.
    ///
    /// This one-off calculation does not replace the remembered render width.
    pub fn move_visual_down_with_width(&mut self, width: usize) -> &mut Self {
        self.move_visual_rows(width, 1)
    }

    /// Moves the cursor to the start of its current visual row.
    pub fn move_to_visual_row_start(&mut self) -> &mut Self {
        self.move_to_visual_row_start_with_width(self.navigation_width())
    }

    /// Moves to the current visual-row start using only `width` for layout.
    ///
    /// This one-off calculation does not replace the remembered render width.
    pub fn move_to_visual_row_start_with_width(&mut self, width: usize) -> &mut Self {
        self.move_to_visual_boundary(width, VisualBoundary::Start)
    }

    /// Moves the cursor to the end of its current visual row.
    pub fn move_to_visual_row_end(&mut self) -> &mut Self {
        self.move_to_visual_row_end_with_width(self.navigation_width())
    }

    /// Moves to the current visual-row end using only `width` for layout.
    ///
    /// This one-off calculation does not replace the remembered render width.
    pub fn move_to_visual_row_end_with_width(&mut self, width: usize) -> &mut Self {
        self.move_to_visual_boundary(width, VisualBoundary::End)
    }

    fn handle_pressed_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> KeyOutcome {
        match code {
            KeyCode::Char('a' | 'A') if is_control_shortcut(modifiers) => {
                self.changed_by(Self::move_to_line_start)
            }
            KeyCode::Char('e' | 'E') if is_control_shortcut(modifiers) => {
                self.changed_by(Self::move_to_line_end)
            }
            KeyCode::Char('b' | 'B') if is_alt_shortcut(modifiers) => {
                self.changed_by(Self::move_word_left)
            }
            KeyCode::Char('f' | 'F') if is_alt_shortcut(modifiers) => {
                self.changed_by(Self::move_word_right)
            }
            KeyCode::Char(character) if !modifiers.intersects(COMMAND_MODIFIERS) => {
                self.changed_by(|textarea| textarea.insert_char(character))
            }
            KeyCode::Enter if modifiers.is_empty() => KeyOutcome::Submit,
            KeyCode::Enter
                if !modifiers.intersects(
                    KeyModifiers::CONTROL
                        .union(KeyModifiers::SUPER)
                        .union(KeyModifiers::META)
                        .union(KeyModifiers::HYPER),
                ) && modifiers.intersects(KeyModifiers::SHIFT.union(KeyModifiers::ALT)) =>
            {
                self.changed_by(Self::insert_newline)
            }
            KeyCode::Backspace => self.changed_by(Self::backspace),
            KeyCode::Delete => self.changed_by(Self::delete),
            KeyCode::Left
                if modifiers.intersects(KeyModifiers::CONTROL.union(KeyModifiers::ALT)) =>
            {
                self.changed_by(Self::move_word_left)
            }
            KeyCode::Right
                if modifiers.intersects(KeyModifiers::CONTROL.union(KeyModifiers::ALT)) =>
            {
                self.changed_by(Self::move_word_right)
            }
            KeyCode::Left => self.changed_by(Self::move_left),
            KeyCode::Right => self.changed_by(Self::move_right),
            KeyCode::Up => self.changed_by(Self::move_visual_up),
            KeyCode::Down => self.changed_by(Self::move_visual_down),
            KeyCode::Home if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(Self::move_to_buffer_start)
            }
            KeyCode::End if modifiers.contains(KeyModifiers::CONTROL) => {
                self.changed_by(Self::move_to_buffer_end)
            }
            KeyCode::Home => self.changed_by(Self::move_to_visual_row_start),
            KeyCode::End => self.changed_by(Self::move_to_visual_row_end),
            KeyCode::Tab => self.changed_by(|textarea| textarea.insert_char('\t')),
            KeyCode::Char(_) | KeyCode::Enter => KeyOutcome::Ignored,
            _ => KeyOutcome::Ignored,
        }
    }

    fn changed_by(&mut self, action: impl FnOnce(&mut Self) -> &mut Self) -> KeyOutcome {
        let previous_value = self.value.clone();
        let previous_cursor = self.cursor;
        let previous_affinity = self.cursor_affinity;
        action(self);

        if self.value == previous_value
            && self.cursor == previous_cursor
            && self.cursor_affinity == previous_affinity
        {
            KeyOutcome::Unchanged
        } else {
            KeyOutcome::Changed
        }
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

    pub(crate) fn remember_render_width(&self, width: usize) {
        self.remembered_render_width.set(Some(width));
    }

    fn render_lines(&self, width: usize) -> Vec<String> {
        self.remember_render_width(width);
        let layout = Layout::new(&self.value, width);
        let rendered = rendering::render(
            &layout,
            self.cursor,
            self.cursor_affinity,
            self.maximum_height,
            self.scroll_row.get(),
            self.scroll_width.get(),
        );
        self.scroll_row.set(rendered.scroll_row);
        self.scroll_width.set(rendered.scroll_width);
        rendered.lines
    }

    fn reset_viewport(&self) {
        self.scroll_row.set(0);
        self.scroll_width.set(None);
    }

    fn reset_after_direct_cursor_change(&mut self) {
        self.cursor_affinity = WrapAffinity::NextRow;
        self.preferred_visual_column = None;
    }

    fn reset_after_replacement(&mut self) {
        self.reset_after_direct_cursor_change();
        self.reset_viewport();
    }
}

fn is_control_shortcut(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) && !modifiers.intersects(CONTROL_SHORTCUT_CONFLICTS)
}

fn is_alt_shortcut(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::ALT) && !modifiers.intersects(ALT_SHORTCUT_CONFLICTS)
}

impl Block for Textarea {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
        self.render_lines(context.width())
            .into_iter()
            .map(Cow::Owned)
            .collect()
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

    #[test]
    fn software_cursor_covers_semantic_content_positions() {
        let mut normal = Textarea::from("ab");
        assert_eq!(marked(normal.render_lines(4)), ["<a>b"]);

        normal.set_cursor(usize::MAX);
        assert_eq!(marked(normal.render_lines(4)), ["ab< >"]);

        let wide = Textarea::from("界x");
        assert_eq!(marked(wide.render_lines(4)), ["<界>x"]);

        let tab = Textarea::from("\t");
        assert_eq!(marked(tab.render_lines(6)), ["< >   "]);

        let mut tab_end = Textarea::from("\tb");
        tab_end.move_right_with_width(5);
        assert_cursor(&tab_end, 1, WrapAffinity::PreviousRow);
        assert_eq!(marked(tab_end.render_lines(5)), ["   < >", "b"]);

        let empty = Textarea::new();
        assert_eq!(marked(empty.render_lines(4)), ["< >"]);

        let mut hidden = Textarea::from("one   two");
        hidden.set_cursor(4);
        assert_eq!(marked(hidden.render_lines(7)), ["one< >", "two"]);

        let zero_width = Textarea::from("\u{200b}");
        assert_eq!(marked(zero_width.render_lines(2)), ["\u{200b}< >"]);
    }

    #[test]
    fn narrow_widths_and_split_tabs_never_overflow() {
        use ansi_str::AnsiStr;
        use unicode_width::UnicodeWidthStr;

        let textarea = Textarea::from("content");
        assert!(textarea.render_lines(0).is_empty());
        assert_eq!(textarea.remembered_render_width.get(), Some(0));
        assert_eq!(marked(textarea.render_lines(1)), ["< >"]);

        let wide = Textarea::from("界");
        assert_eq!(marked(wide.render_lines(2)), ["<界>", ""]);

        let tab = Textarea::from("\t").unlimited_height();
        let rows = tab.render_lines(2);
        assert_eq!(rows.len(), 4);
        for row in rows {
            assert!(UnicodeWidthStr::width(row.ansi_strip().as_ref()) <= 2);
        }
    }

    #[test]
    fn height_builders_and_sticky_viewport_select_expected_rows() {
        let value = "0\n1\n2\n3\n4\n5\n6\n7";
        let default = Textarea::from(value);
        assert_eq!(default.render_lines(10).len(), 6);
        assert_eq!(
            Textarea::from(value).max_height(2).render_lines(10).len(),
            2
        );
        assert_eq!(
            Textarea::from(value).max_height(0).render_lines(10).len(),
            1
        );
        assert_eq!(
            Textarea::from(value)
                .unlimited_height()
                .render_lines(10)
                .len(),
            8
        );

        let mut sticky = Textarea::from(value).max_height(3);
        sticky.set_cursor(usize::MAX);
        assert_eq!(plain(sticky.render_lines(10)), ["5", "6", "7 "]);
        assert_eq!(sticky.scroll_row.get(), 5);

        sticky.set_cursor(6);
        assert_eq!(plain(sticky.render_lines(10)), ["3", "4", "5"]);
        assert_eq!(sticky.scroll_row.get(), 3);

        sticky.set_cursor(8);
        assert_eq!(plain(sticky.render_lines(10)), ["3", "4", "5"]);
        assert_eq!(sticky.scroll_row.get(), 3);
    }

    #[test]
    fn viewport_clamps_on_width_changes_and_resets_on_value_replacement() {
        let mut textarea = Textarea::from("abcdef").max_height(2);
        textarea.set_cursor(usize::MAX);
        textarea.render_lines(2);
        assert!(textarea.scroll_row.get() > 0);
        assert_eq!(textarea.scroll_width.get(), Some(2));

        textarea.render_lines(10);
        assert_eq!(textarea.scroll_row.get(), 0);
        assert_eq!(textarea.scroll_width.get(), Some(10));

        textarea.render_lines(2);
        assert!(textarea.scroll_row.get() > 0);
        textarea.set_value("new");
        assert_eq!(textarea.scroll_row.get(), 0);
        assert_eq!(textarea.scroll_width.get(), None);
        assert_eq!(textarea.remembered_render_width.get(), Some(2));

        textarea.set_cursor(usize::MAX).render_lines(2);
        textarea.clear();
        assert_eq!(textarea.scroll_row.get(), 0);
        textarea
            .set_value("again")
            .set_cursor(usize::MAX)
            .render_lines(2);
        assert_eq!(textarea.take(), "again");
        assert_eq!(textarea.scroll_row.get(), 0);
    }

    #[test]
    fn equality_includes_height_and_viewport_state() {
        let baseline = Textarea::from("a\nb");
        assert_ne!(baseline, baseline.clone().max_height(2));
        assert_ne!(baseline, baseline.clone().unlimited_height());

        let rendered = baseline.clone();
        rendered.render_lines(4);
        assert_ne!(baseline, rendered);

        let mut scrolled = Textarea::from("0\n1\n2").max_height(1);
        scrolled.set_cursor(usize::MAX).render_lines(4);
        let unscrolled = Textarea::from("0\n1\n2").max_height(1);
        assert_ne!(scrolled, unscrolled);
    }

    #[test]
    fn key_dispatch_uses_remembered_width_and_reports_affinity_only_changes() {
        let mut vertical = Textarea::from("abcdef");
        vertical.remember_render_width(4);
        assert_eq!(
            vertical.handle_key_event(KeyEvent::new(KeyCode::Down)),
            KeyOutcome::Changed
        );
        assert_eq!(vertical.cursor(), 3);
        assert_eq!(
            vertical.handle_key_event(KeyEvent::new(KeyCode::Home)),
            KeyOutcome::Unchanged
        );
        assert_eq!(vertical.cursor(), 3);
        assert_eq!(
            vertical.handle_key_event(KeyEvent::new(KeyCode::End)),
            KeyOutcome::Changed
        );
        assert_eq!(vertical.cursor(), 6);

        let mut affinity = Textarea::from("abcd");
        affinity.remember_render_width(4);
        affinity.set_cursor(2).move_right();
        let previous = affinity.clone();
        let cursor = affinity.cursor();

        assert_eq!(
            affinity.handle_key_event(KeyEvent::new(KeyCode::Right)),
            KeyOutcome::Changed
        );
        assert_eq!(affinity.value(), previous.value());
        assert_eq!(affinity.cursor(), cursor);
        assert_ne!(affinity, previous);
    }

    #[test]
    fn plain_enter_submits_without_mutating_viewport_or_navigation_state() {
        let mut textarea = Textarea::from("0\n1\n2\n3\n4").max_height(2);
        textarea.set_cursor(usize::MAX);
        textarea.render_lines(4);
        let before = textarea.clone();

        assert_eq!(
            textarea.handle_key_event(KeyEvent::new(KeyCode::Enter)),
            KeyOutcome::Submit
        );
        assert_eq!(textarea, before);
    }

    #[test]
    fn textarea_uses_the_block_default_render_policy() {
        fn assert_block<T: Block>() {}

        assert_block::<Textarea>();
        assert!(!Textarea::new().render_every_frame());
    }

    fn marked(lines: Vec<String>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| line.replace("\x1b[7m", "<").replace("\x1b[27m", ">"))
            .collect()
    }

    fn plain(lines: Vec<String>) -> Vec<String> {
        use ansi_str::AnsiStr;

        lines
            .into_iter()
            .map(|line| line.ansi_strip().into_owned())
            .collect()
    }

    fn assert_cursor(textarea: &Textarea, cursor: usize, affinity: WrapAffinity) {
        assert_eq!(textarea.cursor(), cursor);
        assert_eq!(textarea.cursor_affinity, affinity);
    }
}
