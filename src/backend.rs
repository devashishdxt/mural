mod termina;

pub use self::termina::TerminaBackend;

use crate::planner::RenderOp;

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
