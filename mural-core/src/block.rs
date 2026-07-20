use std::{any::Any, borrow::Cow};

use crate::color_scheme::ColorScheme;

/// Terminal state available to a block while rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderContext {
    pub(crate) width: usize,
    pub(crate) color_scheme: ColorScheme,
}

impl RenderContext {
    /// Returns the available content width in terminal cells.
    ///
    /// This may be zero and can be smaller than the terminal's raw width.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the terminal's preferred color scheme.
    pub fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }
}

pub trait Block {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>>;

    fn render_every_frame(&self) -> bool {
        false
    }
}

impl Block for &str {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
        drape::wrap(self, context.width())
    }
}

impl Block for String {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
        drape::wrap(self.as_str(), context.width())
    }
}

impl Block for Cow<'_, str> {
    fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
        drape::wrap(self.as_ref(), context.width())
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::borrow::Cow;

    use super::{Block, ErasedBlock, RenderContext};
    use crate::ColorScheme;

    #[test]
    fn string_types_wrap_text() {
        let borrowed = "hello world";
        let owned = borrowed.to_owned();
        let cow = Cow::Borrowed(borrowed);
        let context = RenderContext {
            width: 5,
            color_scheme: ColorScheme::Light,
        };

        assert_eq!(context.width(), 5);
        assert_eq!(context.color_scheme(), ColorScheme::Light);

        for lines in [
            borrowed.render(&context),
            owned.render(&context),
            cow.render(&context),
        ] {
            assert_eq!(lines, ["hello", "world"]);
        }
    }

    #[test]
    fn blocks_do_not_render_every_frame_by_default() {
        assert!(!"text".render_every_frame());
    }

    #[test]
    fn erased_blocks_can_be_downcast_and_mutated() {
        let mut block: Box<dyn ErasedBlock> = Box::new(String::from("before"));

        assert_eq!(block.as_any().downcast_ref::<String>().unwrap(), "before");
        block
            .as_any_mut()
            .downcast_mut::<String>()
            .unwrap()
            .push_str(" after");
        assert_eq!(
            block.as_any().downcast_ref::<String>().unwrap(),
            "before after"
        );
    }
}
