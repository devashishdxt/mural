use crate::Text;
use std::any::{Any, type_name};

/// A value that can render itself into terminal text for a printable width.
///
/// Blocks receive the safe printable width chosen by [`Terminal`](crate::Terminal)
/// and return styled text. Brisk wraps the returned text again if needed before
/// writing it to the backend.
pub trait Render {
    /// Renders this value for `width` terminal columns.
    fn render(&self, width: u16) -> Text;
}

pub(crate) trait RenderBlock: Render {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T: Render + 'static> RenderBlock for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        type_name::<T>()
    }
}
