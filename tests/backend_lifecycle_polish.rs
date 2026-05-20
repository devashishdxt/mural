use brisk::{
    Backend, Line, Size, Terminal, TerminalError, Text,
    backend::fake::{FakeBackend, Operation},
};
use std::{cell::RefCell, io, rc::Rc};

#[derive(Clone, Debug)]
struct SharedLogBackend {
    size: Size,
    operations: Rc<RefCell<Vec<Operation>>>,
}

impl SharedLogBackend {
    fn new(size: Size, operations: Rc<RefCell<Vec<Operation>>>) -> Self {
        Self { size, operations }
    }

    fn record(&mut self, operation: Operation) {
        self.operations.borrow_mut().push(operation);
    }
}

impl Backend for SharedLogBackend {
    fn size(&mut self) -> io::Result<Size> {
        self.record(Operation::QuerySize);
        Ok(self.size)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.record(Operation::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.record(Operation::ShowCursor);
        Ok(())
    }

    fn move_to_origin(&mut self) -> io::Result<()> {
        self.record(Operation::MoveToOrigin);
        Ok(())
    }

    fn move_up(&mut self, lines: u16) -> io::Result<()> {
        self.record(Operation::MoveUp(lines));
        Ok(())
    }

    fn move_to_column(&mut self, column: u16) -> io::Result<()> {
        self.record(Operation::MoveToColumn(column));
        Ok(())
    }

    fn print(&mut self, line: &Line) -> io::Result<()> {
        self.record(Operation::Print(line.clone()));
        Ok(())
    }

    fn newline(&mut self) -> io::Result<()> {
        self.record(Operation::Newline);
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.record(Operation::Clear);
        Ok(())
    }

    fn purge_scrollback(&mut self) -> io::Result<()> {
        self.record(Operation::PurgeScrollback);
        Ok(())
    }

    fn clear_from_cursor_down(&mut self) -> io::Result<()> {
        self.record(Operation::ClearFromCursorDown);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.record(Operation::Flush);
        Ok(())
    }
}

#[test]
fn backend_mut_exposes_escape_hatch_and_forced_redraw_recovers_renderer_state() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(Text::from_plain("managed").unwrap())
        .unwrap();

    terminal
        .backend_mut()
        .print(&Line::from_plain("outside renderer").unwrap())
        .unwrap();
    terminal.force_full_redraw().unwrap();
    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("outside renderer").unwrap()),
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("managed").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn render_and_normal_mutations_report_lifecycle_errors_after_finish() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live("live", Text::from_plain("live").unwrap())
        .unwrap();
    terminal
        .insert_pinned("pinned", Text::from_plain("pinned").unwrap())
        .unwrap();

    terminal.finish().unwrap();

    assert_eq!(
        terminal.render().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal.force_full_redraw().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal.resize(Size::new(40, 10)).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal
            .push_live(Text::from_plain("late live").unwrap())
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal
            .push_pinned(Text::from_plain("late pinned").unwrap())
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal.insert_live("late", Text::from_plain("late").unwrap()),
        Err(TerminalError::Finished)
    );
    assert_eq!(
        terminal.insert_pinned("late", Text::from_plain("late").unwrap()),
        Err(TerminalError::Finished)
    );
    assert_eq!(
        terminal.live_block_mut::<Text>("live"),
        Err(TerminalError::Finished)
    );
    assert_eq!(
        terminal.pinned_block_mut::<Text>("pinned"),
        Err(TerminalError::Finished)
    );
    assert_eq!(terminal.remove_live("live"), Err(TerminalError::Finished));
    assert_eq!(
        terminal.remove_pinned("pinned"),
        Err(TerminalError::Finished)
    );
}

#[test]
fn drop_restores_cursor_and_flushes_without_rendering_blocks() {
    let operations = Rc::new(RefCell::new(Vec::new()));

    {
        let mut terminal = Terminal::new(SharedLogBackend::new(
            Size::new(80, 24),
            Rc::clone(&operations),
        ))
        .unwrap();
        terminal
            .push_live(Text::from_plain("unrendered").unwrap())
            .unwrap();
    }

    assert_eq!(
        operations.borrow().as_slice(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::ShowCursor,
            Operation::Flush
        ]
    );
}
