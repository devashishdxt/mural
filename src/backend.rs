use crate::{Line, Size};
use std::io;

pub mod fake;
pub mod stdout;

pub use fake::{FakeBackend, Operation};
pub use stdout::StdoutBackend;

pub trait Backend {
    fn size(&mut self) -> io::Result<Size>;
    fn hide_cursor(&mut self) -> io::Result<()>;
    fn print(&mut self, line: &Line) -> io::Result<()>;
    fn newline(&mut self) -> io::Result<()>;
    fn clear(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}
