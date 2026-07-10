use thiserror::Error;

use crate::{backend::Backend, block::Block, region::Region, renderer::Renderer};

#[derive(Debug, Error)]
pub enum Error<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("invalid terminal size: width and height must be greater than zero")]
    InvalidTerminalSize,

    #[error("invalid cursor position: row/column must be within terminal size")]
    InvalidCursorPosition,

    #[error(transparent)]
    Backend(#[from] E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub height: usize,
    pub width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub column: usize,
}

pub struct Terminal<B> {
    backend: B,
    live_region: Region,
    pinned_region: Region,
    size: TerminalSize,
    needs_full_redraw: bool,
    committed_frame: Vec<String>,
    sentinel_row: usize,
}

impl<B> Terminal<B>
where
    B: Backend,
{
    pub fn new(
        mut backend: B,
        size: TerminalSize,
        mut position: CursorPosition,
    ) -> Result<Self, Error<B::Error>> {
        validate_size(size)?;
        validate_position(size, position)?;

        backend.hide_cursor()?;
        position = normalize_initial_position(&mut backend, size, position)?;
        backend.flush()?;

        Ok(Self {
            backend,
            live_region: Default::default(),
            pinned_region: Default::default(),
            size,
            needs_full_redraw: false,
            committed_frame: Vec::new(),
            sentinel_row: position.row,
        })
    }

    pub fn push_live(&mut self, block: impl Block + 'static) {
        self.live_region.push(block);
    }

    pub fn push_pinned(&mut self, block: impl Block + 'static) {
        self.pinned_region.push(block);
    }

    pub fn insert_live(&mut self, id: impl Into<String>, block: impl Block + 'static) {
        self.live_region.insert(id, block);
    }

    pub fn insert_pinned(&mut self, id: impl Into<String>, block: impl Block + 'static) {
        self.pinned_region.insert(id, block);
    }

    pub fn get_live<T>(&self, id: impl AsRef<str>) -> Option<&T>
    where
        T: Block + 'static,
    {
        self.live_region.get(id)
    }

    pub fn get_pinned<T>(&self, id: impl AsRef<str>) -> Option<&T>
    where
        T: Block + 'static,
    {
        self.pinned_region.get(id)
    }

    pub fn get_live_mut<T>(&mut self, id: impl AsRef<str>) -> Option<&mut T>
    where
        T: Block + 'static,
    {
        self.live_region.get_mut(id)
    }

    pub fn get_pinned_mut<T>(&mut self, id: impl AsRef<str>) -> Option<&mut T>
    where
        T: Block + 'static,
    {
        self.pinned_region.get_mut(id)
    }

    pub fn remove_live(&mut self, id: impl AsRef<str>) {
        self.live_region.remove(id);
    }

    pub fn remove_pinned(&mut self, id: impl AsRef<str>) {
        self.pinned_region.remove(id);
    }

    pub fn clear_live(&mut self) {
        self.live_region.clear();
    }

    pub fn clear_pinned(&mut self) {
        self.pinned_region.clear();
    }

    pub fn clear_all(&mut self) {
        self.clear_live();
        self.clear_pinned();
    }

    pub fn resize(&mut self, size: TerminalSize) -> Result<(), Error<B::Error>> {
        validate_size(size)?;

        if size.width != self.size.width {
            self.live_region.mark_all_dirty();
            self.pinned_region.mark_all_dirty();
        }

        self.size = size;
        self.needs_full_redraw = true;

        Ok(())
    }

    pub fn force_full_redraw(&mut self) {
        self.needs_full_redraw = true;
    }

    pub fn render(&mut self) -> Result<(), Error<B::Error>> {
        self.render_frame(true)
    }

    pub fn finish(&mut self) -> Result<(), Error<B::Error>> {
        self.render_frame(false)?;
        self.backend.show_cursor()?;
        self.backend.flush()?;

        Ok(())
    }

    fn render_frame(&mut self, include_pinned: bool) -> Result<(), Error<B::Error>> {
        let mut new_frame = self.live_region.render(self.size.width.saturating_sub(1));

        if include_pinned {
            new_frame.extend(self.pinned_region.render(self.size.width.saturating_sub(1)));
        }

        self.sentinel_row = match Renderer::new(
            &self.committed_frame,
            &new_frame,
            self.size.height,
            self.sentinel_row,
        )
        .render(&mut self.backend, self.needs_full_redraw)
        {
            Ok(sentinel_row) => sentinel_row,
            Err(err) => {
                self.needs_full_redraw = true;
                return Err(err.into());
            }
        };

        self.committed_frame = new_frame;

        self.live_region.mark_all_clean();
        if include_pinned {
            self.pinned_region.mark_all_clean();
        }

        self.needs_full_redraw = false;

        Ok(())
    }
}

fn validate_size<E>(size: TerminalSize) -> Result<(), Error<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if size.width == 0 || size.height == 0 {
        Err(Error::InvalidTerminalSize)
    } else {
        Ok(())
    }
}

fn validate_position<E>(size: TerminalSize, position: CursorPosition) -> Result<(), Error<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if position.row >= size.height || position.column >= size.width {
        Err(Error::InvalidCursorPosition)
    } else {
        Ok(())
    }
}

fn normalize_initial_position<B: Backend>(
    backend: &mut B,
    size: TerminalSize,
    position: CursorPosition,
) -> Result<CursorPosition, Error<B::Error>> {
    if position.column == 0 {
        return Ok(position);
    }

    backend.newline()?;
    Ok(CursorPosition {
        row: position.row.saturating_add(1).min(size.height - 1),
        column: 0,
    })
}
