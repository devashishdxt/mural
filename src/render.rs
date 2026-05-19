use crate::Text;

pub trait Render {
    fn render(&self, width: u16) -> Text;
}
