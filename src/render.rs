use std::any::{Any, type_name};

use crate::Text;

/// A value that can render itself into terminal text for a printable width.
///
/// Blocks receive the safe printable width chosen by [`Terminal`](crate::Terminal)
/// and return styled text. Mural wraps the returned text again if needed before
/// writing it to the backend.
pub trait Render {
    /// Renders this value for `width` terminal columns.
    fn render(&self, width: u16) -> Text;

    /// Reports whether this value should be rerendered on every frame.
    ///
    /// This is a scheduling hint for blocks whose rendered output may change
    /// without mutable access through [`Terminal`](crate::Terminal), such as
    /// spinners or clocks. Mural still diffs the rendered output against the
    /// last successful frame and skips terminal writes when the visual lines are
    /// unchanged.
    ///
    /// Keep this method cheap and side-effect-free. It may be called whenever
    /// Mural checks whether a block needs rerendering, and callers should not
    /// rely on exact call counts.
    fn render_every_frame(&self) -> bool {
        false
    }
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
