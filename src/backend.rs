mod termina;

pub use self::termina::TerminaBackend;

use crate::{
    planner::RenderOp,
    terminal::{CursorPosition, TerminalSize},
};

pub trait Backend {
    type Error: std::error::Error + Send + Sync + 'static;

    // Cursor visibility
    fn hide_cursor(&mut self) -> Result<(), Self::Error>;
    fn show_cursor(&mut self) -> Result<(), Self::Error>;

    // Cursor movement
    fn move_up(&mut self, n: usize) -> Result<(), Self::Error>;
    fn move_down(&mut self, n: usize) -> Result<(), Self::Error>;
    fn carriage_return(&mut self) -> Result<(), Self::Error>;
    fn newline(&mut self) -> Result<(), Self::Error>;

    // Viewport movement
    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error>;

    // Line editing
    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error>;
    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error>;
    fn clear_line(&mut self) -> Result<(), Self::Error>;
    fn write_str(&mut self, text: &str) -> Result<(), Self::Error>;

    // Terminal management
    fn clear_screen(&mut self) -> Result<(), Self::Error>;
    fn purge_scrollback(&mut self) -> Result<(), Self::Error>;
    fn move_to_top_left(&mut self) -> Result<(), Self::Error>;

    // Commit
    fn flush(&mut self) -> Result<(), Self::Error>;
}

/// Terminal state inspection operations supported by a [`Backend`].
pub trait BackendProbe: Backend {
    /// Returns the current terminal dimensions in cells.
    fn terminal_size(&mut self) -> Result<TerminalSize, Self::Error>;

    /// Returns the current zero-based cursor position.
    fn cursor_position(&mut self) -> Result<CursorPosition, Self::Error>;
}

pub(crate) trait ExecuteOp: Backend {
    fn execute(&mut self, render_op: RenderOp) -> Result<(), Self::Error>;
}

impl<T> ExecuteOp for T
where
    T: Backend,
{
    fn execute(&mut self, render_op: RenderOp) -> Result<(), Self::Error> {
        match render_op {
            RenderOp::MoveUp(n) => self.move_up(n),
            RenderOp::MoveDown(n) => self.move_down(n),
            RenderOp::CarriageReturn => self.carriage_return(),
            RenderOp::Newline => self.newline(),
            RenderOp::ScrollUp(n) => self.scroll_up(n),
            RenderOp::InsertLines(n) => self.insert_lines(n),
            RenderOp::DeleteLines(n) => self.delete_lines(n),
            RenderOp::ClearLine => self.clear_line(),
            RenderOp::Write(text) => self.write_str(text),
            RenderOp::ClearScreen => self.clear_screen(),
            RenderOp::PurgeScrollback => self.purge_scrollback(),
            RenderOp::MoveToTopLeft => self.move_to_top_left(),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::{convert::Infallible, error::Error};

    use super::{Backend, ExecuteOp};
    use crate::planner::RenderOp;

    #[derive(Default)]
    struct RecordingBackend {
        calls: Vec<String>,
    }

    impl RecordingBackend {
        fn record(&mut self, call: impl Into<String>) -> Result<(), Infallible> {
            self.calls.push(call.into());
            Ok(())
        }
    }

    impl Backend for RecordingBackend {
        type Error = Infallible;

        fn hide_cursor(&mut self) -> Result<(), Self::Error> {
            self.record("hide_cursor")
        }

        fn show_cursor(&mut self) -> Result<(), Self::Error> {
            self.record("show_cursor")
        }

        fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
            self.record(format!("move_up:{n}"))
        }

        fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
            self.record(format!("move_down:{n}"))
        }

        fn carriage_return(&mut self) -> Result<(), Self::Error> {
            self.record("carriage_return")
        }

        fn newline(&mut self) -> Result<(), Self::Error> {
            self.record("newline")
        }

        fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
            self.record(format!("scroll_up:{n}"))
        }

        fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
            self.record(format!("insert_lines:{n}"))
        }

        fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
            self.record(format!("delete_lines:{n}"))
        }

        fn clear_line(&mut self) -> Result<(), Self::Error> {
            self.record("clear_line")
        }

        fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
            self.record(format!("write:{text}"))
        }

        fn clear_screen(&mut self) -> Result<(), Self::Error> {
            self.record("clear_screen")
        }

        fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
            self.record("purge_scrollback")
        }

        fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
            self.record("move_to_top_left")
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            self.record("flush")
        }
    }

    #[test]
    fn execute_dispatches_every_render_operation() -> Result<(), Box<dyn Error>> {
        let mut backend = RecordingBackend::default();
        let operations = [
            RenderOp::MoveUp(1),
            RenderOp::MoveDown(2),
            RenderOp::CarriageReturn,
            RenderOp::Newline,
            RenderOp::ScrollUp(3),
            RenderOp::InsertLines(4),
            RenderOp::DeleteLines(5),
            RenderOp::ClearLine,
            RenderOp::Write("text"),
            RenderOp::ClearScreen,
            RenderOp::PurgeScrollback,
            RenderOp::MoveToTopLeft,
        ];

        for operation in operations {
            backend.execute(operation)?;
        }

        assert_eq!(
            backend.calls,
            [
                "move_up:1",
                "move_down:2",
                "carriage_return",
                "newline",
                "scroll_up:3",
                "insert_lines:4",
                "delete_lines:5",
                "clear_line",
                "write:text",
                "clear_screen",
                "purge_scrollback",
                "move_to_top_left",
            ]
        );
        Ok(())
    }
}
