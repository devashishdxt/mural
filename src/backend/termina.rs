use std::io::{self, Write};

use termina::{
    PlatformTerminal,
    escape::csi::{
        Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Edit, EraseInDisplay, EraseInLine, Mode,
    },
};

use crate::backend::Backend;

pub struct TerminaBackend {
    terminal: PlatformTerminal,
}

impl TerminaBackend {
    pub fn new() -> Result<Self, io::Error> {
        PlatformTerminal::new().map(|terminal| Self { terminal })
    }

    pub fn into_inner(self) -> PlatformTerminal {
        self.terminal
    }

    fn write_csi(&mut self, csi: Csi) -> Result<(), io::Error> {
        write!(self.terminal, "{csi}")
    }
}

impl Backend for TerminaBackend {
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

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\r")
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        self.terminal.write_all(b"\n")
    }

    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
        let Some(n) = nonzero_count(n) else {
            return Ok(());
        };
        self.write_csi(Csi::Edit(Edit::ScrollUp(n)))
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

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Edit(Edit::EraseInLine(EraseInLine::EraseLine)))
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.terminal.write_all(text.as_bytes())
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

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        self.write_csi(Csi::Cursor(Cursor::default_position()))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.terminal.flush()
    }
}

fn nonzero_count(n: usize) -> Option<u32> {
    (n != 0).then(|| n.try_into().unwrap_or(u32::MAX))
}
