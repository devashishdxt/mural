use crate::{Backend, Size};
use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, Write};

pub struct StdoutBackend<W: Write> {
    writer: W,
}

impl<W: Write> StdoutBackend<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl StdoutBackend<io::Stdout> {
    pub fn stdout() -> Self {
        Self::new(io::stdout())
    }
}

impl<W: Write> Backend for StdoutBackend<W> {
    fn size(&mut self) -> io::Result<Size> {
        let (width, height) = terminal::size()?;
        Ok(Size::new(width, height))
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, cursor::Hide)
    }

    fn print(&mut self, content: &str) -> io::Result<()> {
        self.writer.write_all(content.as_bytes())
    }

    fn newline(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\n")
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(self.writer, terminal::Clear(terminal::ClearType::All))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
