use std::io::{self, Write};

use termina::{
    Event, PlatformTerminal, Terminal as _,
    escape::csi::{
        Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Edit, EraseInDisplay, EraseInLine, Mode,
    },
};

use crate::{
    backend::{Backend, BackendProbe},
    terminal::{CursorPosition, TerminalSize},
};

/// A terminal backend using Termina's platform terminal.
pub struct TerminaBackend {
    terminal: PlatformTerminal,
}

impl TerminaBackend {
    pub fn new() -> Result<Self, io::Error> {
        PlatformTerminal::new().map(Into::into)
    }

    pub fn into_inner(self) -> PlatformTerminal {
        self.terminal
    }

    fn write_csi(&mut self, csi: Csi) -> Result<(), io::Error> {
        write!(self.terminal, "{csi}")
    }
}

impl From<PlatformTerminal> for TerminaBackend {
    fn from(terminal: PlatformTerminal) -> Self {
        Self { terminal }
    }
}

impl AsRef<PlatformTerminal> for TerminaBackend {
    fn as_ref(&self) -> &PlatformTerminal {
        &self.terminal
    }
}

impl AsMut<PlatformTerminal> for TerminaBackend {
    fn as_mut(&mut self) -> &mut PlatformTerminal {
        &mut self.terminal
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

/// The underlying terminal must be configured so terminal responses can be read before calling
/// [`BackendProbe::cursor_position`], which typically means entering raw mode. The probe flushes pending output and
/// blocks until the next cursor-position report. Unrelated events remain available to Termina's event reader. Callers
/// must not issue overlapping cursor-position queries.
impl BackendProbe for TerminaBackend {
    fn terminal_size(&mut self) -> Result<TerminalSize, Self::Error> {
        let size = self.terminal.get_dimensions()?;

        Ok(TerminalSize {
            height: usize::from(size.rows),
            width: usize::from(size.cols),
        })
    }

    fn cursor_position(&mut self) -> Result<CursorPosition, Self::Error> {
        self.write_csi(Csi::Cursor(Cursor::RequestActivePositionReport))?;
        self.terminal.flush()?;

        let event = self.terminal.read(|event| {
            matches!(
                event,
                Event::Csi(Csi::Cursor(Cursor::ActivePositionReport { .. }))
            )
        })?;
        let Event::Csi(Csi::Cursor(Cursor::ActivePositionReport { line, col })) = event else {
            unreachable!("filtered terminal read returned an unrelated event")
        };

        Ok(CursorPosition {
            row: usize::from(line.get_zero_based()),
            column: usize::from(col.get_zero_based()),
        })
    }
}

fn nonzero_count(n: usize) -> Option<u32> {
    (n != 0).then(|| n.try_into().unwrap_or(u32::MAX))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::nonzero_count;

    #[test]
    fn count_omits_zero_and_clamps_large_values() {
        assert_eq!(nonzero_count(0), None);
        assert_eq!(nonzero_count(42), Some(42));
        assert_eq!(nonzero_count(usize::MAX), Some(u32::MAX));
    }
}
