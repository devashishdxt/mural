use std::{any::Any, borrow::Cow};

/// Renderable content stored by a terminal region.
pub trait Block {
    /// Render this block into terminal visual lines for the supplied safe width.
    fn render(&self, width: usize) -> Vec<Cow<'_, str>>;

    /// Whether this block should be rendered again on every frame even when clean.
    fn render_every_frame(&self) -> bool {
        false
    }
}

impl Block for &str {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
        drape::wrap(*self, width)
    }
}

impl Block for String {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
        drape::wrap(self.as_str(), width)
    }
}

impl Block for Cow<'_, str> {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
        drape::wrap(self.as_ref(), width)
    }
}

pub(crate) trait ErasedBlock: Block {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> ErasedBlock for T
where
    T: Block + Any,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
