//! Crossterm-backed stdout backend for real terminal output.

use std::io::{self, Write};

use crossterm::{
    cursor, execute, queue,
    style::{Attribute, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal,
};

use crate::{Backend, Color, Line, Modifiers, Size, Style};

const MODIFIER_ATTRIBUTES: [(Modifiers, Attribute); 5] = [
    (Modifiers::BOLD, Attribute::Bold),
    (Modifiers::DIM, Attribute::Dim),
    (Modifiers::ITALIC, Attribute::Italic),
    (Modifiers::UNDERLINE, Attribute::Underlined),
    (Modifiers::REVERSED, Attribute::Reverse),
];

/// Backend that writes terminal operations to a [`Write`] sink.
///
/// [`StdoutBackend::stdout`] constructs the normal stdout-backed version. Tests
/// can use [`StdoutBackend::new`] with another writer to inspect emitted bytes.
pub struct StdoutBackend<W: Write> {
    writer: W,
}

impl<W: Write> StdoutBackend<W> {
    /// Creates a backend around `writer`.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Consumes the backend and returns its wrapped writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    fn write_style_prefix(&mut self, style: Style) -> io::Result<()> {
        if let Some(color) = style.foreground() {
            queue!(self.writer, SetForegroundColor(to_crossterm_color(color)))?;
        }
        if let Some(color) = style.background() {
            queue!(self.writer, SetBackgroundColor(to_crossterm_color(color)))?;
        }

        let modifiers = style.modifiers();
        for (modifier, attribute) in MODIFIER_ATTRIBUTES {
            if modifiers.contains(modifier) {
                queue!(self.writer, SetAttribute(attribute))?;
            }
        }
        Ok(())
    }
}

fn to_crossterm_color(color: Color) -> crossterm::style::Color {
    match color {
        Color::Black => crossterm::style::Color::Black,
        Color::Red => crossterm::style::Color::DarkRed,
        Color::Green => crossterm::style::Color::DarkGreen,
        Color::Yellow => crossterm::style::Color::DarkYellow,
        Color::Blue => crossterm::style::Color::DarkBlue,
        Color::Magenta => crossterm::style::Color::DarkMagenta,
        Color::Cyan => crossterm::style::Color::DarkCyan,
        Color::White => crossterm::style::Color::Grey,
        Color::BrightBlack => crossterm::style::Color::DarkGrey,
        Color::BrightRed => crossterm::style::Color::Red,
        Color::BrightGreen => crossterm::style::Color::Green,
        Color::BrightYellow => crossterm::style::Color::Yellow,
        Color::BrightBlue => crossterm::style::Color::Blue,
        Color::BrightMagenta => crossterm::style::Color::Magenta,
        Color::BrightCyan => crossterm::style::Color::Cyan,
        Color::BrightWhite => crossterm::style::Color::White,
        Color::Indexed(index) => crossterm::style::Color::AnsiValue(index),
        Color::Rgb(r, g, b) => crossterm::style::Color::Rgb { r, g, b },
    }
}

impl StdoutBackend<io::Stdout> {
    /// Creates a backend that writes to process stdout.
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

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, cursor::Show)
    }

    fn move_to_origin(&mut self) -> io::Result<()> {
        execute!(self.writer, cursor::MoveTo(0, 0))
    }

    fn move_up(&mut self, lines: u16) -> io::Result<()> {
        execute!(self.writer, cursor::MoveUp(lines))
    }

    fn move_to_column(&mut self, column: u16) -> io::Result<()> {
        execute!(self.writer, cursor::MoveToColumn(column))
    }

    fn print(&mut self, line: &Line) -> io::Result<()> {
        for span in line.spans() {
            let style = span.style();
            self.write_style_prefix(style)?;
            self.writer.write_all(span.content().as_bytes())?;
            if !style.is_plain() {
                queue!(self.writer, ResetColor)?;
            }
        }
        Ok(())
    }

    fn newline(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\r\n")
    }

    fn clear(&mut self) -> io::Result<()> {
        queue!(self.writer, terminal::Clear(terminal::ClearType::All))
    }

    fn purge_scrollback(&mut self) -> io::Result<()> {
        queue!(self.writer, terminal::Clear(terminal::ClearType::Purge))
    }

    fn clear_from_cursor_down(&mut self) -> io::Result<()> {
        queue!(
            self.writer,
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
