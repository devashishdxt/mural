/// Set of text style modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// Bold/intense text.
    pub const BOLD: Self = Self(1 << 0);
    /// Dim/faint text.
    pub const DIM: Self = Self(1 << 1);
    /// Italic text.
    pub const ITALIC: Self = Self(1 << 2);
    /// Underlined text.
    pub const UNDERLINE: Self = Self(1 << 3);
    /// Reversed foreground/background text.
    pub const REVERSED: Self = Self(1 << 4);

    /// Returns true when all bits in `other` are present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
