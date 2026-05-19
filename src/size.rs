#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    width: u16,
    height: u16,
}

impl Size {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}
