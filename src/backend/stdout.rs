use crate::{Backend, Color, Line, Modifiers, Size, Style};
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

    fn write_style_prefix(&mut self, style: Style) -> io::Result<()> {
        if let Some(fg) = style.foreground() {
            queue!(
                self.writer,
                crossterm::style::SetForegroundColor(to_crossterm_color(fg))
            )?;
        }
        if let Some(bg) = style.background() {
            queue!(
                self.writer,
                crossterm::style::SetBackgroundColor(to_crossterm_color(bg))
            )?;
        }
        let modifiers = style.modifiers();
        if modifiers.contains(Modifiers::BOLD) {
            queue!(
                self.writer,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)
            )?;
        }
        if modifiers.contains(Modifiers::DIM) {
            queue!(
                self.writer,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Dim)
            )?;
        }
        if modifiers.contains(Modifiers::ITALIC) {
            queue!(
                self.writer,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Italic)
            )?;
        }
        if modifiers.contains(Modifiers::UNDERLINE) {
            queue!(
                self.writer,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Underlined)
            )?;
        }
        if modifiers.contains(Modifiers::REVERSED) {
            queue!(
                self.writer,
                crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse)
            )?;
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

    fn print(&mut self, line: &Line) -> io::Result<()> {
        for span in line.spans() {
            let style = span.style();
            self.write_style_prefix(style)?;
            self.writer.write_all(span.content().as_bytes())?;
            if style != Style::new() {
                queue!(self.writer, crossterm::style::ResetColor)?;
            }
        }
        Ok(())
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
