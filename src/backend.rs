use crate::{Line, Size};
use std::io;

pub mod fake;
pub mod stdout;

pub use fake::{FakeBackend, Operation};
pub use stdout::StdoutBackend;

pub trait Backend {
    fn size(&mut self) -> io::Result<Size>;
    fn hide_cursor(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
    fn move_to_origin(&mut self) -> io::Result<()>;
    fn move_up(&mut self, lines: u16) -> io::Result<()>;
    fn move_to_column(&mut self, column: u16) -> io::Result<()>;
    fn print(&mut self, line: &Line) -> io::Result<()>;
    fn newline(&mut self) -> io::Result<()>;
    fn clear(&mut self) -> io::Result<()>;
    fn purge_scrollback(&mut self) -> io::Result<()>;
    fn clear_from_cursor_down(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}
