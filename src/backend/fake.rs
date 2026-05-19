use crate::{Backend, Line, Size};
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    QuerySize,
    HideCursor,
    ShowCursor,
    MoveToColumn(u16),
    Print(Line),
    Newline,
    Clear,
    Flush,
}

#[derive(Debug)]
pub struct FakeBackend {
    size: Size,
    operations: Vec<Operation>,
}

impl FakeBackend {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            operations: Vec::new(),
        }
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
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

    fn flush(&mut self) -> io::Result<()> {
        self.operations.push(Operation::Flush);
        Ok(())
    }
}
