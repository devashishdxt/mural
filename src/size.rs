//! Terminal size value used by backends and resize notifications.

/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    width: u16,
    height: u16,
}

impl Size {
    /// Creates a size from terminal `width` columns and `height` rows.
    ///
    /// Zero dimensions are supported for transient resize states. A zero-width
    /// terminal renders no text, and width `1` leaves a safe printable width of
    /// zero because Mural reserves one column to avoid unwanted wrapping.
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
