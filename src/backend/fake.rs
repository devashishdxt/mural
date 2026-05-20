//! In-memory backend for tests, examples, and deterministic demonstrations.

use crate::{Backend, Line, Size};
use std::io;

/// A backend operation recorded by [`FakeBackend`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// The renderer queried the terminal size.
    QuerySize,
    /// The renderer hid the cursor.
    HideCursor,
    /// The renderer showed the cursor.
    ShowCursor,
    /// The renderer moved to the managed buffer origin.
    MoveToOrigin,
    /// The renderer moved up by the stored number of rows.
    MoveUp(u16),
    /// The renderer moved to the stored terminal column.
    MoveToColumn(u16),
    /// The renderer printed the stored line.
    Print(Line),
    /// The renderer emitted a newline.
    Newline,
    /// The renderer cleared the visible buffer.
    Clear,
    /// The renderer purged scrollback.
    PurgeScrollback,
    /// The renderer cleared from the cursor down.
    ClearFromCursorDown,
    /// The renderer flushed pending output.
    Flush,
}

/// Deterministic backend that records operations instead of touching a terminal.
#[derive(Debug)]
pub struct FakeBackend {
    size: Size,
    operations: Vec<Operation>,
    fail_next_flush: bool,
}

impl FakeBackend {
    /// Creates a fake backend that reports `size` to the terminal.
    pub fn new(size: Size) -> Self {
        Self {
            size,
            operations: Vec::new(),
            fail_next_flush: false,
        }
    }

    /// Returns the operations recorded so far.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Makes the next [`Backend::flush`] call fail once.
    pub fn fail_next_flush(&mut self) {
        self.fail_next_flush = true;
    }
}

impl Backend for FakeBackend {
    fn size(&mut self) -> io::Result<Size> {
        self.operations.push(Operation::QuerySize);
        Ok(self.size)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.operations.push(Operation::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.operations.push(Operation::ShowCursor);
        Ok(())
    }

    fn move_to_origin(&mut self) -> io::Result<()> {
        self.operations.push(Operation::MoveToOrigin);
        Ok(())
    }

    fn move_up(&mut self, lines: u16) -> io::Result<()> {
        self.operations.push(Operation::MoveUp(lines));
        Ok(())
    }

    fn move_to_column(&mut self, column: u16) -> io::Result<()> {
        self.operations.push(Operation::MoveToColumn(column));
        Ok(())
    }

    fn print(&mut self, line: &Line) -> io::Result<()> {
        self.operations.push(Operation::Print(line.clone()));
        Ok(())
    }

    fn newline(&mut self) -> io::Result<()> {
        self.operations.push(Operation::Newline);
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.operations.push(Operation::Clear);
        Ok(())
    }

    fn purge_scrollback(&mut self) -> io::Result<()> {
        self.operations.push(Operation::PurgeScrollback);
        Ok(())
    }

    fn clear_from_cursor_down(&mut self) -> io::Result<()> {
        self.operations.push(Operation::ClearFromCursorDown);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.operations.push(Operation::Flush);
        if self.fail_next_flush {
            self.fail_next_flush = false;
            return Err(io::Error::other("injected flush failure"));
        }
        Ok(())
    }
}
