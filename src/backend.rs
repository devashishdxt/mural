mod termina;

pub use termina::TerminaBackend;

/// Semantic terminal I/O used by the renderer.
pub trait Backend {
    type Error: std::error::Error + Send + Sync + 'static;

    fn hide_cursor(&mut self) -> Result<(), Self::Error>;
    fn show_cursor(&mut self) -> Result<(), Self::Error>;
    fn carriage_return(&mut self) -> Result<(), Self::Error>;
    fn newline(&mut self) -> Result<(), Self::Error>;
    fn move_up(&mut self, n: usize) -> Result<(), Self::Error>;
    fn move_down(&mut self, n: usize) -> Result<(), Self::Error>;
    fn move_to_top_left(&mut self) -> Result<(), Self::Error>;
    fn clear_line(&mut self) -> Result<(), Self::Error>;
    fn clear_screen(&mut self) -> Result<(), Self::Error>;
    fn purge_scrollback(&mut self) -> Result<(), Self::Error>;
    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error>;
    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error>;
    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error>;
    fn write_str(&mut self, text: &str) -> Result<(), Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
}
