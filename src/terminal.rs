use crate::{
    Backend, Block, CursorPosition, LifecycleError, TerminalError, TerminalSize, region::Region,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Running,
    Finishing { final_render_complete: bool },
    Finished,
}

/// Renderer entry point over a semantic backend.
pub struct Terminal<B: Backend> {
    backend: B,
    size: TerminalSize,
    _cursor: CursorPosition,
    lifecycle: Lifecycle,
    live_blocks: Region,
    pinned_blocks: Region,
    last_committed_frame: CommittedFrame,
    needs_full_redraw: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommittedFrame {
    lines: Vec<String>,
    sentinel_row: usize,
}

impl<B: Backend> Terminal<B> {
    pub fn new(
        mut backend: B,
        size: TerminalSize,
        position: CursorPosition,
    ) -> Result<Self, TerminalError<B::Error>> {
        validate_size(size)?;
        validate_position(size, position)?;

        backend.hide_cursor()?;
        let cursor = normalize_initial_position(&mut backend, size, position)?;
        backend.flush()?;

        Ok(Self {
            backend,
            size,
            _cursor: cursor,
            lifecycle: Lifecycle::Running,
            live_blocks: Region::default(),
            pinned_blocks: Region::default(),
            last_committed_frame: CommittedFrame {
                lines: Vec::new(),
                sentinel_row: 0,
            },
            needs_full_redraw: false,
        })
    }

    pub fn push_live<BlockType>(&mut self, block: BlockType)
    where
        BlockType: Block + 'static,
    {
        self.live_blocks.push(block);
    }

    pub fn push_pinned<BlockType>(&mut self, block: BlockType)
    where
        BlockType: Block + 'static,
    {
        self.pinned_blocks.push(block);
    }

    pub fn insert_live<Id, BlockType>(&mut self, id: Id, block: BlockType)
    where
        Id: Into<String>,
        BlockType: Block + 'static,
    {
        self.live_blocks.insert(id, block);
    }

    pub fn insert_pinned<Id, BlockType>(&mut self, id: Id, block: BlockType)
    where
        Id: Into<String>,
        BlockType: Block + 'static,
    {
        self.pinned_blocks.insert(id, block);
    }

    pub fn get_live<BlockType, Id>(&self, id: Id) -> Option<&BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.live_blocks.get(id)
    }

    pub fn get_pinned<BlockType, Id>(&self, id: Id) -> Option<&BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.pinned_blocks.get(id)
    }

    pub fn get_live_mut<BlockType, Id>(&mut self, id: Id) -> Option<&mut BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.live_blocks.get_mut(id)
    }

    pub fn get_pinned_mut<BlockType, Id>(&mut self, id: Id) -> Option<&mut BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.pinned_blocks.get_mut(id)
    }

    pub fn remove_live<Id>(&mut self, id: Id) -> bool
    where
        Id: AsRef<str>,
    {
        self.live_blocks.remove(id)
    }

    pub fn remove_pinned<Id>(&mut self, id: Id) -> bool
    where
        Id: AsRef<str>,
    {
        self.pinned_blocks.remove(id)
    }

    pub fn clear_live(&mut self) {
        self.live_blocks.clear();
    }

    pub fn clear_pinned(&mut self) {
        self.pinned_blocks.clear();
    }

    pub fn resize(&mut self, size: TerminalSize) -> Result<(), TerminalError<B::Error>> {
        validate_size(size)?;
        if size.width != self.size.width {
            self.live_blocks.mark_all_dirty();
            self.pinned_blocks.mark_all_dirty();
        }
        self.size = size;
        self.needs_full_redraw = true;
        Ok(())
    }

    pub fn force_full_redraw(&mut self) {
        self.needs_full_redraw = true;
    }

    pub fn render(&mut self) -> Result<(), TerminalError<B::Error>> {
        if self.lifecycle != Lifecycle::Running {
            return Err(TerminalError::Lifecycle(
                LifecycleError::RenderAfterFinishStarted,
            ));
        }

        let frame = current_frame(
            &mut self.live_blocks,
            &mut self.pinned_blocks,
            self.size.width.saturating_sub(1),
        );

        if let Err(err) = render_frame_transaction(
            &mut self.backend,
            &self.last_committed_frame.lines,
            &frame,
            self.needs_full_redraw,
        ) {
            self.needs_full_redraw = true;
            return Err(err);
        }

        let sentinel_row = frame.len();
        self.live_blocks.mark_all_clean();
        self.pinned_blocks.mark_all_clean();
        self.last_committed_frame = CommittedFrame {
            lines: frame,
            sentinel_row,
        };
        self.needs_full_redraw = false;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), TerminalError<B::Error>> {
        match self.lifecycle {
            Lifecycle::Running => {
                self.lifecycle = Lifecycle::Finishing {
                    final_render_complete: false,
                };
            }
            Lifecycle::Finishing { .. } => {}
            Lifecycle::Finished => {
                return Err(TerminalError::Lifecycle(LifecycleError::AlreadyFinished));
            }
        }

        if matches!(
            self.lifecycle,
            Lifecycle::Finishing {
                final_render_complete: false,
            }
        ) {
            let frame = self
                .live_blocks
                .render_lines(self.size.width.saturating_sub(1));

            if let Err(err) = render_frame_transaction(
                &mut self.backend,
                &self.last_committed_frame.lines,
                &frame,
                self.needs_full_redraw,
            ) {
                self.needs_full_redraw = true;
                return Err(err);
            }

            let sentinel_row = frame.len();
            self.live_blocks.mark_all_clean();
            self.last_committed_frame = CommittedFrame {
                lines: frame,
                sentinel_row,
            };
            self.needs_full_redraw = false;
            self.lifecycle = Lifecycle::Finishing {
                final_render_complete: true,
            };
        }

        self.backend.show_cursor()?;
        self.backend.flush()?;
        self.lifecycle = Lifecycle::Finished;
        Ok(())
    }
}

fn current_frame(
    live_blocks: &mut Region,
    pinned_blocks: &mut Region,
    width: usize,
) -> Vec<String> {
    let mut lines = live_blocks.render_lines(width);
    lines.extend(pinned_blocks.render_lines(width));
    lines
}

fn frame_changed(last_frame: &[String], current_frame: &[String]) -> bool {
    last_frame.len() != current_frame.len()
        || last_frame
            .iter()
            .zip(current_frame)
            .any(|(last, current)| last != current)
}

fn is_append_only(last_frame: &[String], current_frame: &[String]) -> bool {
    current_frame.len() > last_frame.len()
        && current_frame
            .iter()
            .zip(last_frame)
            .all(|(current, last)| current == last)
}

fn render_frame_transaction<B: Backend>(
    backend: &mut B,
    last_frame: &[String],
    current_frame: &[String],
    needs_full_redraw: bool,
) -> Result<(), TerminalError<B::Error>> {
    if needs_full_redraw {
        render_full_frame(backend, current_frame)?;
    } else if is_append_only(last_frame, current_frame) {
        render_appended_lines(backend, &current_frame[last_frame.len()..])?;
    } else if frame_changed(last_frame, current_frame) {
        render_full_frame(backend, current_frame)?;
    }

    backend.flush()?;
    Ok(())
}

fn render_appended_lines<B: Backend>(
    backend: &mut B,
    lines: &[String],
) -> Result<(), TerminalError<B::Error>> {
    for line in lines {
        backend.write_str(line)?;
        backend.newline()?;
    }

    Ok(())
}

fn render_full_frame<B: Backend>(
    backend: &mut B,
    frame: &[String],
) -> Result<(), TerminalError<B::Error>> {
    backend.clear_screen()?;
    backend.purge_scrollback()?;
    backend.move_to_top_left()?;

    for line in frame {
        backend.write_str(line)?;
        backend.newline()?;
    }

    Ok(())
}

impl<B: Backend> Drop for Terminal<B> {
    fn drop(&mut self) {
        if self.lifecycle != Lifecycle::Finished {
            let _ = self.backend.show_cursor();
            let _ = self.backend.flush();
        }
    }
}

fn validate_size<E>(size: TerminalSize) -> Result<(), TerminalError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if size.width == 0 || size.height == 0 {
        Err(TerminalError::InvalidTerminalSize)
    } else {
        Ok(())
    }
}

fn validate_position<E>(
    size: TerminalSize,
    position: CursorPosition,
) -> Result<(), TerminalError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if position.row >= size.height || position.column >= size.width {
        Err(TerminalError::InvalidCursorPosition)
    } else {
        Ok(())
    }
}

fn normalize_initial_position<B: Backend>(
    backend: &mut B,
    size: TerminalSize,
    position: CursorPosition,
) -> Result<CursorPosition, TerminalError<B::Error>> {
    if position.column == 0 {
        return Ok(position);
    }

    backend.newline()?;
    Ok(CursorPosition {
        row: position.row.saturating_add(1).min(size.height - 1),
        column: 0,
    })
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, cell::RefCell, error::Error, fmt, rc::Rc};

    use super::*;
    use crate::backend::recording::{Operation, RecordingBackend};

    #[derive(Clone, Default)]
    struct WidthRecordingBlock {
        widths: Rc<RefCell<Vec<usize>>>,
    }

    impl WidthRecordingBlock {
        fn widths(&self) -> Vec<usize> {
            self.widths.borrow().clone()
        }
    }

    impl Block for WidthRecordingBlock {
        fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
            self.widths.borrow_mut().push(width);
            vec![Cow::Borrowed("width")]
        }
    }

    #[derive(Clone)]
    struct FailOnSecondFlushBackend {
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
        fn fail_on_flush(fail_on_flush: usize) -> Self {
            Self {
                operations: Rc::default(),
                flushes: Rc::default(),
                fail_on_flush,
            }
        }

        fn operations(&self) -> Vec<Operation> {
            self.operations.borrow().clone()
        }

        fn record(&mut self, operation: Operation) {
            self.operations.borrow_mut().push(operation);
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FlushFailed;

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
    struct FailOnArmedOperationBackend {
        operations: Rc<RefCell<Vec<Operation>>>,
        successful_operations_before_failure: Rc<RefCell<Option<usize>>>,
    }

    impl FailOnArmedOperationBackend {
        fn fail_next_operation(&self) {
            self.fail_after_successful_operations(0);
        }

        fn fail_after_successful_operations(&self, count: usize) {
            *self.successful_operations_before_failure.borrow_mut() = Some(count);
        }

        fn operations(&self) -> Vec<Operation> {
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
    struct OperationFailed;

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

    #[test]
    fn terminal_rejects_zero_sized_dimensions() {
        let err = Terminal::new(
            RecordingBackend::default(),
            TerminalSize {
                width: 0,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .err()
        .expect("invalid size should fail");

        assert!(matches!(err, TerminalError::InvalidTerminalSize));
    }

    #[test]
    fn terminal_rejects_cursor_positions_outside_size() {
        let err = Terminal::new(
            RecordingBackend::default(),
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 24, column: 0 },
        )
        .err()
        .expect("invalid cursor position should fail");

        assert!(matches!(err, TerminalError::InvalidCursorPosition));
    }

    #[test]
    fn construction_hides_cursor_and_flushes() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();

        let _terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 3, column: 0 },
        )
        .unwrap();

        assert_eq!(
            operations.operations(),
            vec![Operation::HideCursor, Operation::Flush]
        );
    }

    #[test]
    fn construction_normalizes_nonzero_initial_column_with_newline() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();

        let _terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 3, column: 5 },
        )
        .unwrap();

        assert_eq!(
            operations.operations(),
            vec![Operation::HideCursor, Operation::Newline, Operation::Flush]
        );
    }

    #[test]
    fn bottom_row_nonzero_column_normalizes_after_scroll() {
        let mut backend = RecordingBackend::default();

        let cursor = normalize_initial_position(
            &mut backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 23, column: 5 },
        )
        .unwrap();

        assert_eq!(backend.operations(), vec![Operation::Newline]);
        assert_eq!(cursor, CursorPosition { row: 23, column: 0 });
    }

    #[test]
    fn drop_restores_cursor_and_flushes_best_effort() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();

        {
            let _terminal = Terminal::new(
                backend,
                TerminalSize {
                    width: 80,
                    height: 24,
                },
                CursorPosition { row: 3, column: 0 },
            )
            .unwrap();
        }

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn render_writes_live_blocks_before_pinned_blocks() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("live");
        terminal.push_pinned("pinned");
        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("live".to_owned()),
                Operation::Newline,
                Operation::Write("pinned".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn built_in_string_blocks_wrap_at_terminal_width_minus_one() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 6,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("hello world");
        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("hello".to_owned()),
                Operation::Newline,
                Operation::Write("world".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn unchanged_render_flushes_without_redrawing_visible_content() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("same");
        terminal.render().unwrap();
        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("same".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn mutation_apis_are_memory_only_until_render() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("live");
        terminal.push_pinned("pinned");

        assert_eq!(
            operations.operations(),
            vec![Operation::HideCursor, Operation::Flush]
        );
    }

    #[test]
    fn empty_document_render_commits_empty_frame_with_sentinel() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![Operation::HideCursor, Operation::Flush, Operation::Flush]
        );
        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: Vec::new(),
                sentinel_row: 0,
            }
        );
    }

    #[test]
    fn blocks_receive_safe_width_saturated_at_zero() {
        let backend = RecordingBackend::default();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 1,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();
        let block = WidthRecordingBlock::default();
        let widths = block.clone();

        terminal.push_live(block);
        terminal.render().unwrap();

        assert_eq!(widths.widths(), vec![0]);
    }

    #[test]
    fn failed_flush_does_not_commit_frame() {
        let backend = FailOnSecondFlushBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("retry");
        assert!(terminal.render().is_err());
        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: Vec::new(),
                sentinel_row: 0,
            }
        );
        assert!(terminal.needs_full_redraw);

        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("retry".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("retry".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn backend_operation_failure_during_render_preserves_frame_and_repairs_with_full_redraw() {
        let backend = FailOnArmedOperationBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("committed");
        terminal.render().unwrap();
        terminal.push_live("uncommitted");
        operations.fail_next_operation();

        let err = terminal
            .render()
            .err()
            .expect("backend operation failure should be reported");

        assert!(matches!(err, TerminalError::Backend(OperationFailed)));
        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: vec!["committed".to_owned()],
                sentinel_row: 1,
            }
        );
        assert!(terminal.needs_full_redraw);

        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("committed".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::Write("uncommitted".to_owned()),
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("committed".to_owned()),
                Operation::Newline,
                Operation::Write("uncommitted".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[derive(Debug, Eq, PartialEq)]
    struct NamedBlock(&'static str);

    impl Block for NamedBlock {
        fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
            vec![Cow::Borrowed(self.0)]
        }
    }

    struct OtherBlock;

    impl Block for OtherBlock {
        fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
            vec![Cow::Borrowed("other")]
        }
    }

    #[derive(Clone)]
    struct CountingBlock {
        text: Rc<RefCell<String>>,
        renders: Rc<RefCell<usize>>,
        every_frame: bool,
    }

    impl CountingBlock {
        fn new(text: &str) -> Self {
            Self {
                text: Rc::new(RefCell::new(text.to_owned())),
                renders: Rc::new(RefCell::new(0)),
                every_frame: false,
            }
        }

        fn every_frame(text: &str) -> Self {
            Self {
                every_frame: true,
                ..Self::new(text)
            }
        }

        fn render_count(&self) -> usize {
            *self.renders.borrow()
        }

        fn set_text(&self, text: &str) {
            *self.text.borrow_mut() = text.to_owned();
        }
    }

    impl Block for CountingBlock {
        fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
            *self.renders.borrow_mut() += 1;
            vec![Cow::Owned(self.text.borrow().clone())]
        }

        fn render_every_frame(&self) -> bool {
            self.every_frame
        }
    }

    #[test]
    fn selected_full_redraw_operation_failures_are_transactional() {
        let selected_failures = [
            (0, Operation::ClearScreen),
            (1, Operation::PurgeScrollback),
            (2, Operation::MoveToTopLeft),
            (3, Operation::Write("new".to_owned())),
            (4, Operation::Newline),
        ];

        for (successful_operations_before_failure, failed_operation) in selected_failures {
            let backend = FailOnArmedOperationBackend::default();
            let operations = backend.clone();
            let block = CountingBlock::new("old");
            let mut terminal = Terminal::new(
                backend,
                TerminalSize {
                    width: 80,
                    height: 24,
                },
                CursorPosition { row: 0, column: 0 },
            )
            .unwrap();

            terminal.insert_live("status", block.clone());
            terminal.render().unwrap();
            terminal
                .get_live_mut::<CountingBlock, _>("status")
                .expect("status block should exist")
                .set_text("new");
            let operations_before_failure = operations.operations().len();
            operations.fail_after_successful_operations(successful_operations_before_failure);

            let err = terminal
                .render()
                .err()
                .expect("selected backend operation should fail");

            assert!(matches!(err, TerminalError::Backend(OperationFailed)));
            assert_eq!(
                operations.operations()
                    [operations_before_failure + successful_operations_before_failure],
                failed_operation
            );
            assert_eq!(
                terminal.last_committed_frame,
                CommittedFrame {
                    lines: vec!["old".to_owned()],
                    sentinel_row: 1,
                }
            );
            assert!(terminal.needs_full_redraw);

            terminal.render().unwrap();

            assert_eq!(
                terminal.last_committed_frame,
                CommittedFrame {
                    lines: vec!["new".to_owned()],
                    sentinel_row: 1,
                }
            );
            assert!(!terminal.needs_full_redraw);
        }
    }

    #[test]
    fn failed_render_preserves_dirty_state_until_successful_repair() {
        let backend = FailOnArmedOperationBackend::default();
        let operations = backend.clone();
        let block = CountingBlock::new("clean");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.insert_live("status", block.clone());
        terminal.render().unwrap();
        terminal
            .get_live_mut::<CountingBlock, _>("status")
            .expect("status block should exist")
            .set_text("dirty");
        operations.fail_next_operation();

        assert!(terminal.render().is_err());
        assert_eq!(block.render_count(), 2);
        assert!(terminal.needs_full_redraw);

        terminal.render().unwrap();

        assert_eq!(block.render_count(), 3);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("clean".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("dirty".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn identified_blocks_are_scoped_by_region_and_empty_ids_can_be_replaced_in_place() {
        let mut terminal = Terminal::new(
            RecordingBackend::default(),
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.insert_live("", NamedBlock("live first"));
        terminal.insert_pinned("", NamedBlock("pinned"));
        terminal.insert_live(String::from(""), NamedBlock("live second"));

        assert_eq!(
            terminal.get_live::<NamedBlock, _>("").map(|block| block.0),
            Some("live second")
        );
        assert_eq!(
            terminal
                .get_pinned::<NamedBlock, _>("")
                .map(|block| block.0),
            Some("pinned")
        );
    }

    #[test]
    fn identified_replacement_preserves_order_and_remove_reports_whether_anything_was_removed() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("before");
        terminal.insert_live("stream", "first");
        terminal.push_live("after");
        terminal.insert_live("stream", "second");
        terminal.render().unwrap();

        assert!(!terminal.remove_live("missing"));
        let id = String::from("stream");
        assert!(terminal.remove_live(&id));
        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("before".to_owned()),
                Operation::Newline,
                Operation::Write("second".to_owned()),
                Operation::Newline,
                Operation::Write("after".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("before".to_owned()),
                Operation::Newline,
                Operation::Write("after".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn clear_live_and_clear_pinned_remove_region_contents_independently() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("anonymous live");
        terminal.insert_live("live", NamedBlock("live"));
        terminal.push_pinned("anonymous pinned");
        terminal.insert_pinned("pinned", NamedBlock("pinned"));

        terminal.clear_pinned();
        assert!(terminal.get_pinned::<NamedBlock, _>("pinned").is_none());
        terminal.render().unwrap();

        terminal.clear_live();
        assert!(terminal.get_live::<NamedBlock, _>("live").is_none());
        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("anonymous live".to_owned()),
                Operation::Newline,
                Operation::Write("live".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn mutable_typed_lookup_marks_only_matching_block_dirty() {
        let first = CountingBlock::new("first");
        let second = CountingBlock::new("second");
        let mut terminal = Terminal::new(
            RecordingBackend::default(),
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.insert_live("first", first.clone());
        terminal.insert_live("second", second.clone());
        terminal.render().unwrap();
        terminal.render().unwrap();
        assert_eq!((first.render_count(), second.render_count()), (1, 1));

        assert!(terminal.get_live_mut::<OtherBlock, _>("first").is_none());
        terminal.render().unwrap();
        assert_eq!((first.render_count(), second.render_count()), (1, 1));

        terminal
            .get_live_mut::<CountingBlock, _>("first")
            .expect("matching mutable lookup should succeed")
            .set_text("changed");
        terminal.render().unwrap();

        assert_eq!((first.render_count(), second.render_count()), (2, 1));
    }

    #[test]
    fn clean_caches_are_reused_while_every_frame_blocks_render_each_attempt() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let regular = CountingBlock::new("regular");
        let every_frame = CountingBlock::every_frame("dynamic");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live(regular.clone());
        terminal.push_live(every_frame.clone());
        terminal.render().unwrap();
        terminal.render().unwrap();

        assert_eq!(regular.render_count(), 1);
        assert_eq!(every_frame.render_count(), 2);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("regular".to_owned()),
                Operation::Newline,
                Operation::Write("dynamic".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn invalid_resize_is_memory_only_and_leaves_committed_state_unchanged() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let block = CountingBlock::new("stable");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live(block.clone());
        terminal.render().unwrap();
        let err = terminal
            .resize(TerminalSize {
                width: 0,
                height: 40,
            })
            .err()
            .expect("zero width resize should fail");
        terminal.render().unwrap();

        assert!(matches!(err, TerminalError::InvalidTerminalSize));
        assert_eq!(block.render_count(), 1);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("stable".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn unchanged_size_resize_forces_full_redraw_without_dirtying_clean_caches() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let block = CountingBlock::new("same size");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live(block.clone());
        terminal.render().unwrap();
        terminal
            .resize(TerminalSize {
                width: 80,
                height: 24,
            })
            .unwrap();
        terminal.render().unwrap();

        assert_eq!(block.render_count(), 1);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("same size".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("same size".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn width_changes_dirty_all_blocks_but_height_only_resize_reuses_clean_caches() {
        let live = CountingBlock::new("live");
        let pinned = CountingBlock::new("pinned");
        let mut terminal = Terminal::new(
            RecordingBackend::default(),
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live(live.clone());
        terminal.push_pinned(pinned.clone());
        terminal.render().unwrap();
        assert_eq!((live.render_count(), pinned.render_count()), (1, 1));

        terminal
            .resize(TerminalSize {
                width: 80,
                height: 40,
            })
            .unwrap();
        terminal.render().unwrap();
        assert_eq!((live.render_count(), pinned.render_count()), (1, 1));

        terminal
            .resize(TerminalSize {
                width: 40,
                height: 40,
            })
            .unwrap();
        terminal.render().unwrap();
        assert_eq!((live.render_count(), pinned.render_count()), (2, 2));
    }

    #[test]
    fn resize_after_finish_start_or_completion_does_not_write_or_reopen_rendering() {
        let backend = FailOnSecondFlushBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("partial finish");
        assert!(terminal.finish().is_err());
        let before_resize = operations.operations();
        terminal
            .resize(TerminalSize {
                width: 40,
                height: 12,
            })
            .unwrap();
        let err = terminal
            .render()
            .err()
            .expect("render should stay rejected");

        assert_eq!(operations.operations(), before_resize);
        assert!(matches!(
            err,
            TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
        ));

        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.finish().unwrap();
        let before_resize = operations.operations();
        terminal
            .resize(TerminalSize {
                width: 100,
                height: 30,
            })
            .unwrap();
        let render_err = terminal
            .render()
            .err()
            .expect("render should stay rejected");
        let finish_err = terminal
            .finish()
            .err()
            .expect("finish should stay completed");

        assert_eq!(operations.operations(), before_resize);
        assert!(matches!(
            render_err,
            TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
        ));
        assert!(matches!(
            finish_err,
            TerminalError::Lifecycle(LifecycleError::AlreadyFinished)
        ));
    }

    #[test]
    fn force_full_redraw_is_memory_only_and_preserves_cache_dirty_state() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let block = CountingBlock::new("clean");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.insert_live("status", block.clone());
        terminal.render().unwrap();
        let before_force = operations.operations();
        terminal.force_full_redraw();
        assert_eq!(operations.operations(), before_force);
        terminal.render().unwrap();
        assert_eq!(block.render_count(), 1);

        terminal
            .get_live_mut::<CountingBlock, _>("status")
            .expect("status block should be present")
            .set_text("dirty");
        terminal.force_full_redraw();
        terminal.render().unwrap();

        assert_eq!(block.render_count(), 2);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("clean".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("clean".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("dirty".to_owned()),
                Operation::Newline,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn force_full_redraw_before_finish_renders_live_frame_before_cursor_restore() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("durable");
        terminal.render().unwrap();
        terminal.force_full_redraw();
        terminal.finish().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("durable".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("durable".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn backend_operation_failure_during_finish_preserves_retry_state_and_repairs() {
        let backend = FailOnArmedOperationBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("committed");
        terminal.render().unwrap();
        terminal.push_live("failed final");
        operations.fail_next_operation();

        let err = terminal
            .finish()
            .err()
            .expect("backend operation failure should be reported");

        assert!(matches!(err, TerminalError::Backend(OperationFailed)));
        assert_eq!(
            terminal.lifecycle,
            Lifecycle::Finishing {
                final_render_complete: false,
            }
        );
        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: vec!["committed".to_owned()],
                sentinel_row: 1,
            }
        );
        assert!(terminal.needs_full_redraw);

        terminal.clear_live();
        terminal.push_live("repaired final");
        terminal.finish().unwrap();

        assert_eq!(terminal.lifecycle, Lifecycle::Finished);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("committed".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::Write("failed final".to_owned()),
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("repaired final".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn finish_renders_live_only_restores_cursor_flushes_and_completes() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("final live");
        terminal.push_pinned("transient pinned");
        terminal.finish().unwrap();

        assert_eq!(terminal.lifecycle, Lifecycle::Finished);
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("final live".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn render_errors_after_finish_starts_even_when_finish_fails() {
        let backend = FailOnSecondFlushBackend::default();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("final live");
        assert!(terminal.finish().is_err());

        let err = terminal.render().err().expect("render should be rejected");
        assert!(matches!(
            err,
            TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
        ));
    }

    #[test]
    fn retry_finish_after_failed_final_render_rerenders_current_live_region() {
        let backend = FailOnSecondFlushBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("first");
        assert!(terminal.finish().is_err());
        terminal.clear_live();
        terminal.push_live("second");

        terminal.finish().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("first".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("second".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn retry_finish_after_final_render_succeeds_retries_only_cursor_restore_and_flush() {
        let backend = FailOnSecondFlushBackend::fail_on_flush(3);
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("already rendered");
        assert!(terminal.finish().is_err());
        terminal.clear_live();
        terminal.push_live("should stay memory-only");

        terminal.finish().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("already rendered".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn mutations_after_finished_are_memory_only_and_do_not_enable_render() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.finish().unwrap();
        terminal.insert_live("live", NamedBlock("stored live"));
        terminal.insert_pinned("pinned", NamedBlock("stored pinned"));

        assert_eq!(
            terminal
                .get_live::<NamedBlock, _>("live")
                .map(|block| block.0),
            Some("stored live")
        );
        assert_eq!(
            terminal
                .get_pinned::<NamedBlock, _>("pinned")
                .map(|block| block.0),
            Some("stored pinned")
        );
        let err = terminal
            .render()
            .err()
            .expect("render should stay rejected");
        assert!(matches!(
            err,
            TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
        ));
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn finish_after_finished_errors_and_drop_writes_no_cleanup() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();

        {
            let mut terminal = Terminal::new(
                backend,
                TerminalSize {
                    width: 80,
                    height: 24,
                },
                CursorPosition { row: 0, column: 0 },
            )
            .unwrap();

            terminal.push_live("done");
            terminal.finish().unwrap();
            let err = terminal.finish().err().expect("second finish should fail");
            assert!(matches!(
                err,
                TerminalError::Lifecycle(LifecycleError::AlreadyFinished)
            ));
        }

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("done".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn finish_preserves_pinned_cache_without_rendering_it() {
        let backend = RecordingBackend::default();
        let operations = backend.clone();
        let pinned = CountingBlock::new("pinned");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("live");
        terminal.insert_pinned("status", pinned.clone());
        terminal.render().unwrap();
        assert_eq!(pinned.render_count(), 1);

        terminal.finish().unwrap();

        assert_eq!(pinned.render_count(), 1);
        assert!(terminal.get_pinned::<CountingBlock, _>("status").is_some());
        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::Write("live".to_owned()),
                Operation::Newline,
                Operation::Write("pinned".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Write("live".to_owned()),
                Operation::Newline,
                Operation::Flush,
                Operation::ShowCursor,
                Operation::Flush,
            ]
        );
    }

    #[test]
    fn block_every_frame_hook_defaults_to_false() {
        struct MinimalBlock;

        impl Block for MinimalBlock {
            fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
                Vec::new()
            }
        }

        assert!(!MinimalBlock.render_every_frame());
    }
}
