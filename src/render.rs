use crate::Text;
use std::any::{Any, type_name};

pub trait Render {
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
