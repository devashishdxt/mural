use std::io::{self, Write};

use termina::{
    PlatformTerminal,
    escape::csi::{
        Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Edit, EraseInDisplay, EraseInLine, Mode,
    },
};

use super::Backend;

/// Backend backed by the `termina` crate.
pub struct TerminaBackend<W = PlatformTerminal> {
    terminal: W,
}

impl TerminaBackend<PlatformTerminal> {
    /// Open the current terminal for rendering.
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {
            terminal: PlatformTerminal::new()?,
        })
    }
}

impl<W: Write> TerminaBackend<W> {
    #[cfg(test)]
    fn from_writer(writer: W) -> Self {
        Self { terminal: writer }
    }

    #[cfg(test)]
    fn into_writer(self) -> W {
        self.terminal
    }

    fn write_csi(&mut self, csi: Csi) -> Result<(), io::Error> {
        write!(self.terminal, "{csi}")
    }
}

impl<W: Write> Backend for TerminaBackend<W> {
    type Error = io::Error;

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
            DecPrivateModeCode::ShowCursor,
        ))))
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
            DecPrivateModeCode::ShowCursor,
        ))))
    }

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\r")
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\r\n")
    }

    fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Cursor(Cursor::Up(n)))
    }

    fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Cursor(Cursor::Down(n)))
    }

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Cursor(Cursor::default_position()))
    }

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInLine(EraseInLine::EraseLine)))
    }

    fn clear_screen(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInDisplay(
            EraseInDisplay::EraseDisplay,
        )))
    }

    fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInDisplay(
            EraseInDisplay::EraseScrollback,
        )))
    }

    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::InsertLine(n)))
    }

    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::DeleteLine(n)))
    }

    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::ScrollUp(n)))
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.terminal.write_all(text.as_bytes())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.terminal.flush()
    }
}

fn nonzero_count(n: usize) -> Option<u32> {
    (n != 0).then(|| n.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn nonzero_count_filters_zero_and_saturates_large_values() {
        assert_eq!(nonzero_count(0), None);
        assert_eq!(nonzero_count(7), Some(7));
        assert_eq!(nonzero_count(usize::MAX), Some(u32::MAX));
    }

    #[test]
    fn writes_backend_escape_sequences_and_text_to_inner_writer() {
        let mut backend = TerminaBackend::from_writer(Vec::new());

        backend.hide_cursor().unwrap();
        backend.show_cursor().unwrap();
        backend.carriage_return().unwrap();
        backend.newline().unwrap();
        backend.move_up(0).unwrap();
        backend.move_up(2).unwrap();
        backend.move_down(0).unwrap();
        backend.move_down(3).unwrap();
        backend.move_to_top_left().unwrap();
        backend.clear_line().unwrap();
        backend.clear_screen().unwrap();
        backend.purge_scrollback().unwrap();
        backend.insert_lines(0).unwrap();
        backend.insert_lines(4).unwrap();
        backend.delete_lines(0).unwrap();
        backend.delete_lines(5).unwrap();
        backend.scroll_up(0).unwrap();
        backend.scroll_up(6).unwrap();
        backend.write_str("hello").unwrap();
        backend.flush().unwrap();

        let output = String::from_utf8(backend.into_writer()).unwrap();
        let expected = format!(
            "{}{}\r\r\n{}{}{}{}{}{}{}{}{}hello",
            Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ShowCursor,
            ))),
            Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                DecPrivateModeCode::ShowCursor,
            ))),
            Csi::Cursor(Cursor::Up(2)),
            Csi::Cursor(Cursor::Down(3)),
            Csi::Cursor(Cursor::default_position()),
            Csi::Edit(Edit::EraseInLine(EraseInLine::EraseLine)),
            Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseDisplay)),
            Csi::Edit(Edit::EraseInDisplay(EraseInDisplay::EraseScrollback)),
            Csi::Edit(Edit::InsertLine(4)),
            Csi::Edit(Edit::DeleteLine(5)),
            Csi::Edit(Edit::ScrollUp(6)),
        );
        assert_eq!(output, expected);
    }
}
