use crate::{Backend, Line, Size};
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    QuerySize,
    HideCursor,
    ShowCursor,
    MoveToOrigin,
    MoveUp(u16),
    MoveToColumn(u16),
    Print(Line),
    Newline,
    Clear,
    PurgeScrollback,
    ClearFromCursorDown,
    Flush,
}

#[derive(Debug)]
pub struct FakeBackend {
    size: Size,
    operations: Vec<Operation>,
    fail_next_flush: bool,
}

impl FakeBackend {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            operations: Vec::new(),
            fail_next_flush: false,
        }
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

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
