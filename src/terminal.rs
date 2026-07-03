use crate::{Backend, CursorPosition, TerminalError, TerminalSize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Running,
    Finished,
}

/// Renderer entry point over a semantic backend.
pub struct Terminal<B: Backend> {
    backend: B,
    _size: TerminalSize,
    _cursor: CursorPosition,
    lifecycle: Lifecycle,
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
            _size: size,
            _cursor: cursor,
            lifecycle: Lifecycle::Running,
        })
    }
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
    use super::*;
    use crate::backend::recording::{Operation, RecordingBackend};

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
}
