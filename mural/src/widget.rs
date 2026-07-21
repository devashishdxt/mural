//! High-level terminal widgets.

use crate::editing;

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

    // These focused reset points let navigation and viewport state be added without
    // scattering their invariants across every public buffer operation.
    fn reset_after_direct_cursor_change(&mut self) {}

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
