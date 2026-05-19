use super::{Color, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    modifiers: Modifiers,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.modifiers.insert(Modifiers::BOLD);
        self
    }

    pub fn dim(mut self) -> Self {
        self.modifiers.insert(Modifiers::DIM);
        self
    }

    pub fn italic(mut self) -> Self {
        self.modifiers.insert(Modifiers::ITALIC);
        self
    }

    pub fn underline(mut self) -> Self {
        self.modifiers.insert(Modifiers::UNDERLINE);
        self
    }

    pub fn reversed(mut self) -> Self {
        self.modifiers.insert(Modifiers::REVERSED);
        self
    }

    pub fn foreground(&self) -> Option<Color> {
        self.fg
    }

    pub fn background(&self) -> Option<Color> {
        self.bg
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}
