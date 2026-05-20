//! High-level renderer lifecycle and block management.

use crate::{Backend, Line, Render, Size, StdoutBackend, TerminalError};
use std::{any::type_name, io};

use crate::render::RenderBlock;

struct BlockEntry {
    id: Option<String>,
    block: Box<dyn RenderBlock>,
    dirty: bool,
    cached_width: Option<u16>,
    cached_lines: Vec<Line>,
}

impl BlockEntry {
    fn unidentified(block: impl Render + 'static) -> Self {
        Self::new(None, block)
    }

    fn identified(id: String, block: impl Render + 'static) -> Self {
        Self::new(Some(id), block)
    }

    fn new(id: Option<String>, block: impl Render + 'static) -> Self {
        Self {
            id,
            block: Box::new(block),
            dirty: true,
            cached_width: None,
            cached_lines: Vec::new(),
        }
    }

    fn rendered_lines(&mut self, width: u16) -> Vec<Line> {
        if self.dirty || self.cached_width != Some(width) {
            self.cached_lines = self
                .block
                .render(width)
                .into_wrapped(usize::from(width))
                .lines()
                .to_vec();
            self.cached_width = Some(width);
        }

        self.cached_lines.clone()
    }

    fn has_id(&self, id: &str) -> bool {
        self.id.as_deref() == Some(id)
    }
}

/// Renderer that manages live and pinned blocks over a terminal backend.
///
/// A terminal starts by hiding the cursor. Callers mutate blocks, call
/// [`Terminal::render`] when they want output, and call [`Terminal::finish`] to
/// leave the live transcript behind and restore terminal state.
pub struct Terminal<B: Backend> {
    backend: B,
    size: Size,
    live_blocks: Vec<BlockEntry>,
    pinned_blocks: Vec<BlockEntry>,
    finished: bool,
    recovery_required: bool,
    screen_snapshot: Vec<Line>,
}

impl Terminal<StdoutBackend<io::Stdout>> {
    /// Creates a terminal that renders to process stdout.
    pub fn stdout() -> io::Result<Self> {
        Self::new(StdoutBackend::stdout())
    }
}

impl<B: Backend> Terminal<B> {
    /// Creates a terminal over `backend` and hides the cursor.
    pub fn new(mut backend: B) -> io::Result<Self> {
        let size = backend.size()?;
        backend.hide_cursor()?;
        Ok(Self {
            backend,
            size,
            live_blocks: Vec::new(),
            pinned_blocks: Vec::new(),
            finished: false,
            recovery_required: false,
            screen_snapshot: Vec::new(),
        })
    }

    /// Returns the backend used by this terminal.
    ///
    /// This is an escape hatch for advanced callers that need to inspect backend-specific
    /// state. Prefer the renderer APIs for normal terminal output.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns mutable access to the backend used by this terminal.
    ///
    /// Direct backend writes can invalidate the renderer's cached screen assumptions. After
    /// writing through this escape hatch, call [`Terminal::force_full_redraw`] before the next
    /// [`Terminal::render`] so the renderer recovers with a full rewrite.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Appends an unidentified live block before pinned blocks.
    ///
    /// Live blocks form the durable conversation transcript and are left behind
    /// by [`Terminal::finish`].
    pub fn push_live(&mut self, block: impl Render + 'static) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.live_blocks.push(BlockEntry::unidentified(block));
        Ok(())
    }

    /// Appends an unidentified pinned block after all live blocks.
    ///
    /// Pinned blocks are useful for status UI and are removed during
    /// [`Terminal::finish`].
    pub fn push_pinned(&mut self, block: impl Render + 'static) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.pinned_blocks.push(BlockEntry::unidentified(block));
        Ok(())
    }

    /// Appends an identified live block.
    ///
    /// Identified blocks can later be mutated with [`Terminal::live_block_mut`]
    /// or removed with [`Terminal::remove_live`]. Ids are unique across live and
    /// pinned regions.
    pub fn insert_live(
        &mut self,
        id: impl Into<String>,
        block: impl Render + 'static,
    ) -> Result<(), TerminalError> {
        self.insert_identified(id, block, BlockRegion::Live)
    }

    /// Appends an identified pinned block.
    ///
    /// Identified pinned blocks can later be mutated with
    /// [`Terminal::pinned_block_mut`] or removed with [`Terminal::remove_pinned`].
    pub fn insert_pinned(
        &mut self,
        id: impl Into<String>,
        block: impl Render + 'static,
    ) -> Result<(), TerminalError> {
        self.insert_identified(id, block, BlockRegion::Pinned)
    }

    /// Returns typed mutable access to an identified live block.
    ///
    /// Successful mutable access marks the block dirty so the next render will
    /// rerender it.
    pub fn live_block_mut<T: 'static>(&mut self, id: &str) -> Result<&mut T, TerminalError> {
        self.block_mut(id, BlockRegion::Live)
    }

    /// Returns typed mutable access to an identified pinned block.
    ///
    /// Successful mutable access marks the block dirty so the next render will
    /// rerender it.
    pub fn pinned_block_mut<T: 'static>(&mut self, id: &str) -> Result<&mut T, TerminalError> {
        self.block_mut(id, BlockRegion::Pinned)
    }

    /// Removes an identified live block.
    pub fn remove_live(&mut self, id: &str) -> Result<(), TerminalError> {
        self.remove_identified(id, BlockRegion::Live)
    }

    /// Removes an identified pinned block.
    pub fn remove_pinned(&mut self, id: &str) -> Result<(), TerminalError> {
        self.remove_identified(id, BlockRegion::Pinned)
    }

    /// Reports whether an identified block is dirty and will be rerendered.
    pub fn is_block_dirty(&self, id: &str) -> Result<bool, TerminalError> {
        self.blocks()
            .find(|block| block.has_id(id))
            .map(|block| block.dirty)
            .ok_or_else(|| TerminalError::MissingBlockId { id: id.to_owned() })
    }

    /// Forces the next render to recover by rewriting the full managed screen buffer.
    ///
    /// Use this after direct writes through [`Terminal::backend_mut`] or after any external
    /// terminal interaction that may have invalidated the renderer's cached screen snapshot.
    pub fn force_full_redraw(&mut self) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.recovery_required = true;
        Ok(())
    }

    /// Notifies the terminal about a caller-observed resize.
    ///
    /// The notification performs no I/O immediately. The next render uses the
    /// new safe printable width and performs a full redraw.
    pub fn resize(&mut self, size: Size) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.size = size;
        self.recovery_required = true;
        Ok(())
    }

    /// Renders dirty or changed live and pinned blocks to the backend.
    ///
    /// Brisk writes the smallest safe suffix when possible and falls back to a
    /// full rewrite for recovery, resize, or changes above the visible viewport.
    pub fn render(&mut self) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.render_frame(RenderMode::Normal)
    }

    /// Finishes live rendering and restores terminal state.
    ///
    /// Finish renders a final live-only frame, removes pinned UI with the same
    /// diff/recovery algorithm used by [`Terminal::render`], moves to a fresh
    /// prompt line, shows the cursor, and flushes. After finish, rendering and
    /// mutation APIs return lifecycle errors.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            self.backend.show_cursor()?;
            return self.backend.flush();
        }

        self.render_frame(RenderMode::Finish)?;
        self.finished = true;
        Ok(())
    }

    fn insert_identified(
        &mut self,
        id: impl Into<String>,
        block: impl Render + 'static,
        region: BlockRegion,
    ) -> Result<(), TerminalError> {
        self.ensure_unfinished_for_mutation()?;
        let id = id.into();
        if self.blocks().any(|block| block.has_id(&id)) {
            return Err(TerminalError::DuplicateBlockId { id });
        }

        self.blocks_in_region_mut(region)
            .push(BlockEntry::identified(id, block));
        Ok(())
    }

    fn remove_identified(
        &mut self,
        id: &str,
        expected_region: BlockRegion,
    ) -> Result<(), TerminalError> {
        self.ensure_unfinished_for_mutation()?;
        self.ensure_block_region(id, expected_region)?;

        let blocks = self.blocks_in_region_mut(expected_region);
        let index = blocks
            .iter()
            .position(|block| block.has_id(id))
            .ok_or_else(|| TerminalError::MissingBlockId { id: id.to_owned() })?;
        blocks.remove(index);
        Ok(())
    }

    fn block_mut<T: 'static>(
        &mut self,
        id: &str,
        expected_region: BlockRegion,
    ) -> Result<&mut T, TerminalError> {
        self.ensure_unfinished_for_mutation()?;
        self.ensure_block_region(id, expected_region)?;

        let block = self
            .blocks_in_region_mut(expected_region)
            .iter_mut()
            .find(|block| block.has_id(id))
            .ok_or_else(|| TerminalError::MissingBlockId { id: id.to_owned() })?;
        if !block.block.as_any().is::<T>() {
            return Err(TerminalError::WrongBlockType {
                id: id.to_owned(),
                expected: type_name::<T>(),
                actual: block.block.type_name(),
            });
        }

        block.dirty = true;
        Ok(block
            .block
            .as_any_mut()
            .downcast_mut::<T>()
            .expect("type was checked before downcast"))
    }

    fn blocks(&self) -> impl Iterator<Item = &BlockEntry> {
        self.live_blocks.iter().chain(self.pinned_blocks.iter())
    }

    fn blocks_mut(&mut self) -> impl Iterator<Item = &mut BlockEntry> {
        self.live_blocks
            .iter_mut()
            .chain(self.pinned_blocks.iter_mut())
    }

    fn blocks_in_region_mut(&mut self, region: BlockRegion) -> &mut Vec<BlockEntry> {
        match region {
            BlockRegion::Live => &mut self.live_blocks,
            BlockRegion::Pinned => &mut self.pinned_blocks,
        }
    }

    fn region_containing_id(&self, id: &str) -> Option<BlockRegion> {
        if self.live_blocks.iter().any(|block| block.has_id(id)) {
            Some(BlockRegion::Live)
        } else if self.pinned_blocks.iter().any(|block| block.has_id(id)) {
            Some(BlockRegion::Pinned)
        } else {
            None
        }
    }

    fn ensure_block_region(
        &self,
        id: &str,
        expected_region: BlockRegion,
    ) -> Result<(), TerminalError> {
        let actual_region = self
            .region_containing_id(id)
            .ok_or_else(|| TerminalError::MissingBlockId { id: id.to_owned() })?;
        if actual_region != expected_region {
            return Err(expected_region.wrong_region_error(id));
        }

        Ok(())
    }

    fn ensure_unfinished(&self) -> io::Result<()> {
        if self.finished {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "terminal has already finished",
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_unfinished_for_mutation(&self) -> Result<(), TerminalError> {
        if self.finished {
            Err(TerminalError::Finished)
        } else {
            Ok(())
        }
    }

    fn render_frame(&mut self, mode: RenderMode) -> io::Result<()> {
        let target_lines = self.rendered_lines(mode);

        if let Err(error) = self.write_frame(&target_lines, mode) {
            self.recovery_required = true;
            return Err(error);
        }

        self.screen_snapshot = target_lines;
        self.recovery_required = false;
        self.mark_rendered_blocks_clean();
        Ok(())
    }

    fn write_frame(&mut self, target_lines: &[Line], mode: RenderMode) -> io::Result<()> {
        self.write_target_lines(target_lines)?;
        self.write_frame_ending(mode)?;
        self.backend.flush()
    }

    fn write_frame_ending(&mut self, mode: RenderMode) -> io::Result<()> {
        if mode.end_with_newline() {
            self.backend.newline()?;
            self.backend.move_to_column(0)?;
        }
        if mode.show_cursor() {
            self.backend.show_cursor()?;
        }
        Ok(())
    }

    fn print_lines_with_separator(
        &mut self,
        lines: &[Line],
        leading_separator: bool,
    ) -> io::Result<()> {
        for (index, line) in lines.iter().enumerate() {
            if (leading_separator && index == 0) || index > 0 {
                self.backend.newline()?;
            }
            self.backend.print(line)?;
        }
        Ok(())
    }

    fn write_target_lines(&mut self, target_lines: &[Line]) -> io::Result<()> {
        if self.recovery_required {
            self.recover_with_full_rewrite(target_lines)
        } else if target_lines != self.screen_snapshot {
            self.write_changed_lines(target_lines)
        } else {
            Ok(())
        }
    }

    fn recover_with_full_rewrite(&mut self, target_lines: &[Line]) -> io::Result<()> {
        self.backend.move_to_origin()?;
        self.backend.clear()?;
        self.backend.purge_scrollback()?;
        self.print_lines_with_separator(target_lines, false)
    }

    fn write_changed_lines(&mut self, target_lines: &[Line]) -> io::Result<()> {
        let Some(first_changed) = first_changed_line(&self.screen_snapshot, target_lines) else {
            return Ok(());
        };
        let previous_len = self.screen_snapshot.len();

        if first_changed < previous_len {
            let lines_up = previous_len - 1 - first_changed;
            if self.changed_line_is_above_viewport(lines_up) {
                return self.recover_with_full_rewrite(target_lines);
            }
            if lines_up > 0 {
                self.backend.move_up(lines_up as u16)?;
            }
            self.backend.move_to_column(0)?;
            self.backend.clear_from_cursor_down()?;
            self.print_lines_with_separator(&target_lines[first_changed..], false)
        } else {
            self.print_lines_with_separator(&target_lines[first_changed..], previous_len > 0)
        }
    }

    fn changed_line_is_above_viewport(&self, lines_up: usize) -> bool {
        lines_up >= usize::from(self.size.height())
    }

    fn rendered_lines(&mut self, mode: RenderMode) -> Vec<Line> {
        let safe_width = self.safe_width();
        if mode.render_pinned() {
            self.blocks_mut()
                .flat_map(|block| block.rendered_lines(safe_width))
                .collect()
        } else {
            self.live_blocks
                .iter_mut()
                .flat_map(|block| block.rendered_lines(safe_width))
                .collect()
        }
    }

    fn safe_width(&self) -> u16 {
        self.size.width().saturating_sub(1)
    }

    fn mark_rendered_blocks_clean(&mut self) {
        for block in self.blocks_mut() {
            block.dirty = false;
        }
    }
}

impl<B: Backend> Drop for Terminal<B> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.backend.show_cursor();
            let _ = self.backend.flush();
        }
    }
}

fn first_changed_line(previous: &[Line], next: &[Line]) -> Option<usize> {
    let shared_len = previous.len().min(next.len());

    previous
        .iter()
        .zip(next)
        .position(|(previous, next)| previous != next)
        .or_else(|| (previous.len() != next.len()).then_some(shared_len))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderMode {
    Normal,
    Finish,
}

impl RenderMode {
    fn render_pinned(self) -> bool {
        matches!(self, RenderMode::Normal)
    }

    fn end_with_newline(self) -> bool {
        matches!(self, RenderMode::Finish)
    }

    fn show_cursor(self) -> bool {
        matches!(self, RenderMode::Finish)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockRegion {
    Live,
    Pinned,
}

impl BlockRegion {
    fn wrong_region_error(self, id: &str) -> TerminalError {
        match self {
            BlockRegion::Live => TerminalError::ExpectedLiveBlock { id: id.to_owned() },
            BlockRegion::Pinned => TerminalError::ExpectedPinnedBlock { id: id.to_owned() },
        }
    }
}
