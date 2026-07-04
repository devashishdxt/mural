use std::{cell::RefCell, convert::Infallible, rc::Rc};

use crate::Backend;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
    HideCursor,
    ShowCursor,
    CarriageReturn,
    Newline,
    MoveUp(usize),
    MoveDown(usize),
    MoveToTopLeft,
    ClearLine,
    ClearScreen,
    PurgeScrollback,
    InsertLines(usize),
    DeleteLines(usize),
    ScrollUp(usize),
    Write(String),
    Flush,
}

#[derive(Clone, Default)]
pub(crate) struct RecordingBackend {
    operations: Rc<RefCell<Vec<Operation>>>,
}

impl RecordingBackend {
    pub(crate) fn operations(&self) -> Vec<Operation> {
        self.operations.borrow().clone()
    }

    fn record(&mut self, operation: Operation) {
        self.operations.borrow_mut().push(operation);
    }
}

impl Backend for RecordingBackend {
    type Error = Infallible;

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ShowCursor);
        Ok(())
    }

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::CarriageReturn);
        Ok(())
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::Newline);
        Ok(())
    }

    fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
        if n != 0 {
            self.record(Operation::MoveUp(n));
        }
        Ok(())
    }

    fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
        if n != 0 {
            self.record(Operation::MoveDown(n));
        }
        Ok(())
    }

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::MoveToTopLeft);
        Ok(())
    }

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ClearLine);
        Ok(())
    }

    fn clear_screen(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ClearScreen);
        Ok(())
    }

    fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::PurgeScrollback);
        Ok(())
    }

    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        if n != 0 {
            self.record(Operation::InsertLines(n));
        }
        Ok(())
    }

    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        if n != 0 {
            self.record(Operation::DeleteLines(n));
        }
        Ok(())
    }

    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
        if n != 0 {
            self.record(Operation::ScrollUp(n));
        }
        Ok(())
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.record(Operation::Write(text.to_owned()));
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::Flush);
        Ok(())
    }
}
