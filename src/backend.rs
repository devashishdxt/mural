//! Terminal I/O backends used by [`Terminal`](crate::Terminal).
//!
//! Use [`StdoutBackend`] for real terminal output, [`FakeBackend`] for examples
//! and tests, or implement [`Backend`] to adapt another terminal abstraction.

use crate::{Line, Size};
use std::io;

pub mod fake;
pub mod stdout;

pub use fake::{FakeBackend, Operation};
pub use stdout::StdoutBackend;

/// Low-level terminal operations required by the renderer.
///
/// Backends are responsible only for terminal I/O. Mural decides when to move,
/// clear, print, and flush while keeping its own cached view of the managed
/// screen area.
pub trait Backend {
    /// Returns the current terminal size.
    fn size(&mut self) -> io::Result<Size>;

    /// Hides the terminal cursor while live rendering is active.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Shows the terminal cursor during finish and drop cleanup.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Moves the cursor to the origin of the managed screen buffer.
    fn move_to_origin(&mut self) -> io::Result<()>;

    /// Moves the cursor up by `lines` terminal rows.
    fn move_up(&mut self, lines: u16) -> io::Result<()>;

    /// Moves the cursor to `column` on the current row.
    fn move_to_column(&mut self, column: u16) -> io::Result<()>;

    /// Prints one already-wrapped line at the current cursor position.
    fn print(&mut self, line: &Line) -> io::Result<()>;

    /// Advances to the next terminal row.
    fn newline(&mut self) -> io::Result<()>;

    /// Clears the visible terminal buffer.
    fn clear(&mut self) -> io::Result<()>;

    /// Clears terminal scrollback when the backend supports it.
    fn purge_scrollback(&mut self) -> io::Result<()>;

    /// Clears from the cursor position down to the end of the screen.
    fn clear_from_cursor_down(&mut self) -> io::Result<()>;

    /// Flushes pending backend output.
    fn flush(&mut self) -> io::Result<()>;
}
