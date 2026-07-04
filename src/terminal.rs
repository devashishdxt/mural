mod diff;
mod frame;
mod rendering;

#[cfg(test)]
mod tests;

use crate::{
    Backend, Block, CursorPosition, LifecycleError, TerminalError, TerminalSize, region::Region,
};

use frame::{CommittedFrame, current_frame};
use rendering::render_frame_transaction;

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
            &self.last_committed_frame,
            &frame,
            self.size.height,
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
                &self.last_committed_frame,
                &frame,
                self.size.height,
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
