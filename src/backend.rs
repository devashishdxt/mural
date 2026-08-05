mod termina;

pub use self::termina::TerminaBackend;

use crate::{
    color_scheme::{ColorScheme, detect_from_env},
    planner::RenderOp,
    terminal::{CursorPosition, TerminalSize},
};

/// A terminal output target capable of executing Mural's rendering operations.
pub trait Backend {
    /// The error returned by terminal operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Hides the terminal cursor.
    fn hide_cursor(&mut self) -> Result<(), Self::Error>;

    /// Shows the terminal cursor.
    fn show_cursor(&mut self) -> Result<(), Self::Error>;

    /// Moves the cursor up by `n` rows.
    fn move_up(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Moves the cursor down by `n` rows.
    fn move_down(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Moves the cursor to the beginning of its current row.
    fn carriage_return(&mut self) -> Result<(), Self::Error>;

    /// Writes a newline, scrolling the viewport when necessary.
    fn newline(&mut self) -> Result<(), Self::Error>;

    /// Scrolls the viewport up by `n` rows.
    fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Inserts `n` blank lines at the cursor.
    fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Deletes `n` lines at the cursor.
    fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error>;

    /// Clears the entire row containing the cursor.
    fn clear_line(&mut self) -> Result<(), Self::Error>;

    /// Writes text at the current cursor position.
    fn write_str(&mut self, text: &str) -> Result<(), Self::Error>;

    /// Clears the visible terminal screen.
    fn clear_screen(&mut self) -> Result<(), Self::Error>;

    /// Clears the terminal's scrollback history.
    fn purge_scrollback(&mut self) -> Result<(), Self::Error>;

    /// Moves the cursor to the terminal's top-left cell.
    fn move_to_top_left(&mut self) -> Result<(), Self::Error>;

    /// Flushes buffered output to the terminal.
    fn flush(&mut self) -> Result<(), Self::Error>;
}

/// Terminal state inspection operations supported by a [`Backend`].
pub trait BackendProbe: Backend {
    /// Returns the current terminal dimensions in cells.
    fn terminal_size(&mut self) -> Result<TerminalSize, Self::Error>;

    /// Returns the current zero-based cursor position.
    fn cursor_position(&mut self) -> Result<CursorPosition, Self::Error>;

    /// Detects the terminal's preferred color scheme.
    ///
    /// Returns `None` when no supported detection mechanism provides a usable hint, allowing the
    /// caller to select its own fallback. Backends that do not override this method inspect only
    /// `COLORFGBG`.
    fn color_scheme(&mut self) -> Result<Option<ColorScheme>, Self::Error> {
        Ok(detect_from_env())
    }
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
