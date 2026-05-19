pub trait Render {
    fn render(&self, width: u16) -> Vec<String>;
}
