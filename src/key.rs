//! Keyboard input types shared by interactive blocks.

use std::ops::{BitOr, BitOrAssign};

/// A logical keyboard event that interactive blocks can handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    code: KeyCode,
    modifiers: KeyModifiers,
    kind: KeyEventKind,
}

impl KeyEvent {
    /// Creates a key press with no modifiers.
    pub fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
        }
    }

    /// Sets the key event kind.
    pub fn kind(mut self, kind: KeyEventKind) -> Self {
        self.kind = kind;
        self
    }

    /// Adds a key modifier.
    pub fn modifier(mut self, modifier: KeyModifiers) -> Self {
        self.modifiers.insert(modifier);
        self
    }

    /// Returns the key code.
    pub fn code(&self) -> KeyCode {
        self.code
    }

    /// Returns the active key modifiers.
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Returns whether this is a press, repeat, or release event.
    pub fn kind_value(&self) -> KeyEventKind {
        self.kind
    }
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: event.code.into(),
            modifiers: event.modifiers.into(),
            kind: event.kind.into(),
        }
    }
}

/// Logical key codes understood by Mural widgets.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A printable character key.
    Char(char),
    /// The enter/return key.
    Enter,
    /// The backspace key.
    Backspace,
    /// The forward delete key.
    Delete,
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
    /// The up arrow key.
    Up,
    /// The down arrow key.
    Down,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// The tab key.
    Tab,
    /// The shift-tab key.
    BackTab,
    /// The escape key.
    Esc,
    /// The page up key.
    PageUp,
    /// The page down key.
    PageDown,
    /// A key not modeled by Mural's logical key API.
    Unsupported,
}

impl From<crossterm::event::KeyCode> for KeyCode {
    fn from(code: crossterm::event::KeyCode) -> Self {
        match code {
            crossterm::event::KeyCode::Backspace => Self::Backspace,
            crossterm::event::KeyCode::Enter => Self::Enter,
            crossterm::event::KeyCode::Left => Self::Left,
            crossterm::event::KeyCode::Right => Self::Right,
            crossterm::event::KeyCode::Up => Self::Up,
            crossterm::event::KeyCode::Down => Self::Down,
            crossterm::event::KeyCode::Home => Self::Home,
            crossterm::event::KeyCode::End => Self::End,
            crossterm::event::KeyCode::PageUp => Self::PageUp,
            crossterm::event::KeyCode::PageDown => Self::PageDown,
            crossterm::event::KeyCode::Tab => Self::Tab,
            crossterm::event::KeyCode::BackTab => Self::BackTab,
            crossterm::event::KeyCode::Delete => Self::Delete,
            crossterm::event::KeyCode::Char(ch) => Self::Char(ch),
            crossterm::event::KeyCode::Esc => Self::Esc,
            _ => Self::Unsupported,
        }
    }
}

/// Set of key modifiers active for a [`KeyEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// The shift modifier.
    pub const SHIFT: Self = Self(1 << 0);
    /// The alt/option modifier.
    pub const ALT: Self = Self(1 << 1);
    /// The control modifier.
    pub const CONTROL: Self = Self(1 << 2);
    /// The super/command/windows modifier.
    pub const SUPER: Self = Self(1 << 3);
    /// The meta modifier.
    pub const META: Self = Self(1 << 4);
    /// The hyper modifier.
    pub const HYPER: Self = Self(1 << 5);

    /// Returns an empty modifier set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns true when no modifiers are active.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when all bits in `other` are present.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns true when any bits in `other` are present.
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns this set with `other` included.
    pub fn with(mut self, other: Self) -> Self {
        self.insert(other);
        self
    }

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

impl From<crossterm::event::KeyModifiers> for KeyModifiers {
    fn from(modifiers: crossterm::event::KeyModifiers) -> Self {
        let mut mapped = Self::empty();
        if modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
            mapped.insert(Self::SHIFT);
        }
        if modifiers.contains(crossterm::event::KeyModifiers::ALT) {
            mapped.insert(Self::ALT);
        }
        if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
            mapped.insert(Self::CONTROL);
        }
        if modifiers.contains(crossterm::event::KeyModifiers::SUPER) {
            mapped.insert(Self::SUPER);
        }
        if modifiers.contains(crossterm::event::KeyModifiers::META) {
            mapped.insert(Self::META);
        }
        if modifiers.contains(crossterm::event::KeyModifiers::HYPER) {
            mapped.insert(Self::HYPER);
        }
        mapped
    }
}

/// The lifecycle kind of a key event.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    /// A key was pressed.
    Press,
    /// A held key repeated.
    Repeat,
    /// A key was released.
    Release,
}

impl From<crossterm::event::KeyEventKind> for KeyEventKind {
    fn from(kind: crossterm::event::KeyEventKind) -> Self {
        match kind {
            crossterm::event::KeyEventKind::Press => Self::Press,
            crossterm::event::KeyEventKind::Repeat => Self::Repeat,
            crossterm::event::KeyEventKind::Release => Self::Release,
        }
    }
}

/// The result of applying a key event to an interactive block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The event changed the block's visible value or cursor position.
    Changed,
    /// The event was handled but left the block unchanged.
    Unchanged,
    /// The event requested submission of the current value.
    Submit,
    /// The event is not part of the block's default key behavior.
    Ignored,
}
