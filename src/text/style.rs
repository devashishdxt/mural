use super::{Color, Modifiers};

/// Style attributes applied to a [`Span`](crate::Span).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    modifiers: Modifiers,
}

impl Style {
    /// Creates the default style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the foreground color.
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Sets the background color.
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Adds the bold modifier.
    pub fn bold(mut self) -> Self {
        self.modifiers.insert(Modifiers::BOLD);
        self
    }

    /// Adds the dim modifier.
    pub fn dim(mut self) -> Self {
        self.modifiers.insert(Modifiers::DIM);
        self
    }

    /// Adds the italic modifier.
    pub fn italic(mut self) -> Self {
        self.modifiers.insert(Modifiers::ITALIC);
        self
    }

    /// Adds the underline modifier.
    pub fn underline(mut self) -> Self {
        self.modifiers.insert(Modifiers::UNDERLINE);
        self
    }

    /// Adds the reversed foreground/background modifier.
    pub fn reversed(mut self) -> Self {
        self.modifiers.insert(Modifiers::REVERSED);
        self
    }

    /// Adds the given modifier.
    pub fn modifier(mut self, modifier: Modifiers) -> Self {
        self.modifiers.insert(modifier);
        self
    }

    /// Overlays another style on this style.
    ///
    /// Foreground/background colors from `overlay` replace existing colors when set;
    /// modifiers are unioned.
    pub fn overlay(mut self, overlay: Style) -> Self {
        if let Some(color) = overlay.fg {
            self.fg = Some(color);
        }
        if let Some(color) = overlay.bg {
            self.bg = Some(color);
        }
        self.modifiers.insert(overlay.modifiers);
        self
    }

    /// Returns the foreground color, if any.
    pub fn foreground(&self) -> Option<Color> {
        self.fg
    }

    /// Returns the background color, if any.
    pub fn background(&self) -> Option<Color> {
        self.bg
    }

    pub(crate) fn is_plain(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.modifiers.is_empty()
    }

    /// Returns the active style modifiers.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}
