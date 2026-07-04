use std::{cell::RefCell, error::Error, fmt, rc::Rc};

use crate::Backend;

use super::recording_backend::Operation;

#[derive(Clone)]
pub(crate) struct FailOnSecondFlushBackend {
    operations: Rc<RefCell<Vec<Operation>>>,
    flushes: Rc<RefCell<usize>>,
    fail_on_flush: usize,
}

impl Default for FailOnSecondFlushBackend {
    fn default() -> Self {
        Self::fail_on_flush(2)
    }
}

impl FailOnSecondFlushBackend {
    pub(crate) fn fail_on_flush(fail_on_flush: usize) -> Self {
        Self {
            operations: Rc::default(),
            flushes: Rc::default(),
            fail_on_flush,
        }
    }

    pub(crate) fn operations(&self) -> Vec<Operation> {
        self.operations.borrow().clone()
    }

    fn record(&mut self, operation: Operation) {
        self.operations.borrow_mut().push(operation);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FlushFailed;

impl fmt::Display for FlushFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("flush failed")
    }
}

impl Error for FlushFailed {}

impl Backend for FailOnSecondFlushBackend {
    type Error = FlushFailed;

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
        let mut flushes = self.flushes.borrow_mut();
        *flushes += 1;
        if *flushes == self.fail_on_flush {
            Err(FlushFailed)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct FailOnArmedOperationBackend {
    operations: Rc<RefCell<Vec<Operation>>>,
    successful_operations_before_failure: Rc<RefCell<Option<usize>>>,
}

impl FailOnArmedOperationBackend {
    pub(crate) fn fail_next_operation(&self) {
        self.fail_after_successful_operations(0);
    }

    pub(crate) fn fail_after_successful_operations(&self, count: usize) {
        *self.successful_operations_before_failure.borrow_mut() = Some(count);
    }

    pub(crate) fn operations(&self) -> Vec<Operation> {
        self.operations.borrow().clone()
    }

    fn record(&mut self, operation: Operation) -> Result<(), OperationFailed> {
        self.operations.borrow_mut().push(operation);
        let mut remaining = self.successful_operations_before_failure.borrow_mut();
        let Some(count) = remaining.as_mut() else {
            return Ok(());
        };

        if *count == 0 {
            *remaining = None;
            return Err(OperationFailed);
        }

        *count -= 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationFailed;

impl fmt::Display for OperationFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation failed")
    }
}

impl Error for OperationFailed {}

impl Backend for FailOnArmedOperationBackend {
    type Error = OperationFailed;

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::HideCursor)
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ShowCursor)
    }

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::CarriageReturn)
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::Newline)
    }

    fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
        if n == 0 {
            Ok(())
        } else {
            self.record(Operation::MoveUp(n))
        }
    }

    fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
        if n == 0 {
            Ok(())
        } else {
            self.record(Operation::MoveDown(n))
        }
    }

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::MoveToTopLeft)
    }

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ClearLine)
    }

    fn clear_screen(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::ClearScreen)
    }

    fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::PurgeScrollback)
    }

    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        if n == 0 {
            Ok(())
        } else {
            self.record(Operation::InsertLines(n))
        }
    }

    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
        if n == 0 {
            Ok(())
        } else {
            self.record(Operation::DeleteLines(n))
        }
    }

    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
        if n == 0 {
            Ok(())
        } else {
            self.record(Operation::ScrollUp(n))
        }
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.record(Operation::Write(text.to_owned()))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.record(Operation::Flush)
    }
}
