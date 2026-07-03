use std::borrow::Cow;

use crate::{Backend, Block, CursorPosition, TerminalError, TerminalSize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Running,
    Finished,
}

/// Renderer entry point over a semantic backend.
pub struct Terminal<B: Backend> {
    backend: B,
    size: TerminalSize,
    _cursor: CursorPosition,
    lifecycle: Lifecycle,
    live_blocks: Vec<Box<dyn Block>>,
    pinned_blocks: Vec<Box<dyn Block>>,
    last_committed_frame: Option<CommittedFrame>,
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
            live_blocks: Vec::new(),
            pinned_blocks: Vec::new(),
            last_committed_frame: None,
        })
    }

    pub fn push_live<BlockType>(&mut self, block: BlockType)
    where
        BlockType: Block + 'static,
    {
        self.live_blocks.push(Box::new(block));
    }

    pub fn push_pinned<BlockType>(&mut self, block: BlockType)
    where
        BlockType: Block + 'static,
    {
        self.pinned_blocks.push(Box::new(block));
    }

    pub fn render(&mut self) -> Result<(), TerminalError<B::Error>> {
        let frame = current_frame(
            &self.live_blocks,
            &self.pinned_blocks,
            self.size.width.saturating_sub(1),
        );
        if frame_changed(self.last_committed_frame.as_ref(), &frame) {
            render_full_frame(&mut self.backend, &frame)?;
        }
        self.backend.flush()?;
        self.last_committed_frame = Some(CommittedFrame {
            lines: frame.iter().map(|line| line.to_string()).collect(),
            sentinel_row: frame.len(),
        });
        Ok(())
    }
}

fn current_frame<'a>(
    live_blocks: &'a [Box<dyn Block>],
    pinned_blocks: &'a [Box<dyn Block>],
    width: usize,
) -> Vec<Cow<'a, str>> {
    let mut lines = Vec::new();

    for block in live_blocks.iter().chain(pinned_blocks.iter()) {
        let rendered_lines = block.render(width);
        debug_assert!(
            rendered_lines
                .iter()
                .all(|line| !line.contains('\n') && !line.contains('\r'))
        );
        lines.extend(rendered_lines);
    }

    lines
}

fn frame_changed(last_frame: Option<&CommittedFrame>, current_frame: &[Cow<'_, str>]) -> bool {
    let Some(last_frame) = last_frame else {
        return true;
    };

    last_frame.lines.len() != current_frame.len()
        || last_frame
            .lines
            .iter()
            .zip(current_frame)
            .any(|(last, current)| last.as_str() != current.as_ref())
}

fn render_full_frame<B: Backend>(
    backend: &mut B,
    frame: &[Cow<'_, str>],
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
    use std::{cell::RefCell, error::Error, fmt, rc::Rc};

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

    #[derive(Clone, Default)]
    struct FailOnSecondFlushBackend {
        operations: Rc<RefCell<Vec<Operation>>>,
        flushes: Rc<RefCell<usize>>,
    }

    impl FailOnSecondFlushBackend {
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
            if *flushes == 2 {
                Err(FlushFailed)
            } else {
                Ok(())
            }
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
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
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
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
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
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
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
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
                Operation::Flush,
            ]
        );
        assert_eq!(
            terminal.last_committed_frame,
            Some(CommittedFrame {
                lines: Vec::new(),
                sentinel_row: 0,
            })
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
        assert_eq!(terminal.last_committed_frame, None);

        terminal.render().unwrap();

        assert_eq!(
            operations.operations(),
            vec![
                Operation::HideCursor,
                Operation::Flush,
                Operation::ClearScreen,
                Operation::PurgeScrollback,
                Operation::MoveToTopLeft,
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
