use std::{any::Any, borrow::Cow};

pub trait Block {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>>;

    fn render_every_frame(&self) -> bool {
        false
    }
}

impl Block for &str {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
        drape::wrap(self, width)
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::borrow::Cow;

    use super::{Block, ErasedBlock};

    #[test]
    fn string_types_wrap_text() {
        let borrowed = "hello world";
        let owned = borrowed.to_owned();
        let cow = Cow::Borrowed(borrowed);

        for lines in [borrowed.render(5), owned.render(5), cow.render(5)] {
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
