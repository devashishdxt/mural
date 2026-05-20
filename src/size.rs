//! Terminal size value used by backends and resize notifications.

/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    width: u16,
    height: u16,
}

impl Size {
    /// Creates a size from terminal `width` columns and `height` rows.
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Returns the number of terminal columns.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Returns the number of terminal rows.
    pub fn height(&self) -> u16 {
        self.height
    }
}
