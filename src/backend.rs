mod termina;

pub use termina::TerminaBackend;

/// Terminal output used by [`Terminal`](crate::Terminal).
pub trait Backend {
    /// Error type produced by backend operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Hide the terminal cursor while managed content is being updated.
    fn hide_cursor(&mut self) -> Result<(), Self::Error>;

    /// Show the terminal cursor again during finish or drop cleanup.
    fn show_cursor(&mut self) -> Result<(), Self::Error>;

    /// Move to column zero on the current row.
    fn carriage_return(&mut self) -> Result<(), Self::Error>;

    /// Move to column zero on the next row.
    fn newline(&mut self) -> Result<(), Self::Error>;

    /// Move the cursor up by `n` rows.
    fn move_up(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Move the cursor down by `n` rows.
    fn move_down(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Move the cursor to the top-left of the visible screen.
    fn move_to_top_left(&mut self) -> Result<(), Self::Error>;

    /// Clear the current terminal line.
    fn clear_line(&mut self) -> Result<(), Self::Error>;

    /// Clear the visible terminal screen.
    fn clear_screen(&mut self) -> Result<(), Self::Error>;

    /// Purge terminal scrollback.
    fn purge_scrollback(&mut self) -> Result<(), Self::Error>;

    /// Insert `n` blank lines at the current row.
    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Delete `n` lines starting at the current row.
    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Scroll the normal screen up by `n` rows.
    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Write text at the current cursor position.
    fn write_str(&mut self, text: &str) -> Result<(), Self::Error>;

    /// Flush buffered terminal output.
    fn flush(&mut self) -> Result<(), Self::Error>;
}
