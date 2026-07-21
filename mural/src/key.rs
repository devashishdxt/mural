//! Backend-independent keyboard input types for Mural widgets.

use std::ops::{BitOr, BitOrAssign};

/// A semantic keyboard event that a Mural widget can handle.
///
/// The event contains only widget-relevant information. Backend-specific state is deliberately
/// excluded so applications and future backend adapters can construct events without depending on
/// Termina's representation.
///
/// ```
/// use mural::key::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
///
/// let event = KeyEvent::new(KeyCode::Char('x'))
///     .with_modifier(KeyModifiers::CONTROL)
///     .with_kind(KeyEventKind::Repeat);
///
/// assert_eq!(event.code(), KeyCode::Char('x'));
/// assert!(event.modifiers().contains(KeyModifiers::CONTROL));
/// assert_eq!(event.kind(), KeyEventKind::Repeat);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    code: KeyCode,
    modifiers: KeyModifiers,
    kind: KeyEventKind,
}

impl KeyEvent {
    /// Creates a key-press event with no active modifiers.
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
        }
    }

    /// Returns this event with its lifecycle kind set to `kind`.
    #[must_use]
    pub const fn with_kind(mut self, kind: KeyEventKind) -> Self {
        self.kind = kind;
        self
    }

    /// Returns this event with exactly the supplied `modifiers`.
    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Returns this event with `modifier` added to its active modifiers.
    #[must_use]
    pub const fn with_modifier(mut self, modifier: KeyModifiers) -> Self {
        self.modifiers = self.modifiers.union(modifier);
        self
    }

    /// Returns the event's semantic key code.
    pub const fn code(&self) -> KeyCode {
        self.code
    }

    /// Returns the event's active semantic modifiers.
    pub const fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Returns whether the key was pressed, repeated, or released.
    pub const fn kind(&self) -> KeyEventKind {
        self.kind
    }
}

impl From<termina::event::KeyEvent> for KeyEvent {
    fn from(event: termina::event::KeyEvent) -> Self {
        Self {
            code: map_termina_code(event.code),
            modifiers: map_termina_modifiers(event.modifiers),
            kind: map_termina_kind(event.kind),
        }
    }
}

/// A widget-relevant semantic key code.
///
/// This intentionally models only keys needed by Mural widgets rather than every physical,
/// function, media, keypad, or lock key. Backends map keys outside this set to
/// [`Self::Unsupported`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A printable Unicode character.
    Char(char),
    /// The Enter or Return key.
    Enter,
    /// The Backspace key.
    Backspace,
    /// The forward Delete key.
    Delete,
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
    /// The up arrow key.
    Up,
    /// The down arrow key.
    Down,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The Tab key.
    Tab,
    /// Shift+Tab or another backwards-tab key sequence.
    BackTab,
    /// The Escape key.
    Escape,
    /// The Page Up key.
    PageUp,
    /// The Page Down key.
    PageDown,
    /// A key outside Mural's widget-focused semantic model.
    Unsupported,
}

/// A set of semantic modifier keys active for a [`KeyEvent`].
///
/// Lock keys are not part of this set. Combining constants with `|` creates a set containing
/// multiple modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// The Shift modifier.
    pub const SHIFT: Self = Self(1 << 0);
    /// The Alt or Option modifier.
    pub const ALT: Self = Self(1 << 1);
    /// The Control modifier.
    pub const CONTROL: Self = Self(1 << 2);
    /// The Super, Command, or Windows modifier.
    pub const SUPER: Self = Self(1 << 3);
    /// The Meta modifier.
    pub const META: Self = Self(1 << 4);
    /// The Hyper modifier.
    pub const HYPER: Self = Self(1 << 5);

    /// Returns an empty modifier set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns `true` when no modifiers are active.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` when every modifier in `other` is active.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` when any modifier in `other` is active.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns a set containing the modifiers from both sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns this set with `other` included.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        self.union(other)
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

/// The lifecycle kind of a key event.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    /// A key was pressed.
    Press,
    /// A held key generated another event.
    Repeat,
    /// A key was released.
    Release,
}

/// The result of applying a key event to a textarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Text, the byte cursor, or visual cursor affinity changed, so rendered output may differ.
    Changed,
    /// The event belongs to the textarea's default behavior, but made no logical or visual change.
    Unchanged,
    /// The event requests application-controlled submission without mutating or clearing the text.
    Submit,
    /// The event is not handled by the textarea's default key behavior.
    Ignored,
}

fn map_termina_code(code: termina::event::KeyCode) -> KeyCode {
    use termina::event::KeyCode as TerminaCode;

    match code {
        TerminaCode::Char(character) => KeyCode::Char(character),
        TerminaCode::Enter => KeyCode::Enter,
        TerminaCode::Backspace => KeyCode::Backspace,
        TerminaCode::Delete => KeyCode::Delete,
        TerminaCode::Left => KeyCode::Left,
        TerminaCode::Right => KeyCode::Right,
        TerminaCode::Up => KeyCode::Up,
        TerminaCode::Down => KeyCode::Down,
        TerminaCode::Home => KeyCode::Home,
        TerminaCode::End => KeyCode::End,
        TerminaCode::Tab => KeyCode::Tab,
        TerminaCode::BackTab => KeyCode::BackTab,
        TerminaCode::Escape => KeyCode::Escape,
        TerminaCode::PageUp => KeyCode::PageUp,
        TerminaCode::PageDown => KeyCode::PageDown,
        _ => KeyCode::Unsupported,
    }
}

fn map_termina_modifiers(modifiers: termina::event::Modifiers) -> KeyModifiers {
    use termina::event::Modifiers as TerminaModifiers;

    const MAPPINGS: [(TerminaModifiers, KeyModifiers); 6] = [
        (TerminaModifiers::SHIFT, KeyModifiers::SHIFT),
        (TerminaModifiers::ALT, KeyModifiers::ALT),
        (TerminaModifiers::CONTROL, KeyModifiers::CONTROL),
        (TerminaModifiers::SUPER, KeyModifiers::SUPER),
        (TerminaModifiers::META, KeyModifiers::META),
        (TerminaModifiers::HYPER, KeyModifiers::HYPER),
    ];

    MAPPINGS
        .into_iter()
        .filter(|(source, _)| modifiers.contains(*source))
        .fold(KeyModifiers::empty(), |mapped, (_, target)| {
            mapped.union(target)
        })
}

fn map_termina_kind(kind: termina::event::KeyEventKind) -> KeyEventKind {
    match kind {
        termina::event::KeyEventKind::Press => KeyEventKind::Press,
        termina::event::KeyEventKind::Repeat => KeyEventKind::Repeat,
        termina::event::KeyEventKind::Release => KeyEventKind::Release,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termina::event::{
        KeyCode as TerminaCode, KeyEvent as TerminaEvent, KeyEventKind as TerminaKind,
        KeyEventState as TerminaState, MediaKeyCode, ModifierKeyCode,
        Modifiers as TerminaModifiers,
    };

    #[test]
    fn applications_can_build_and_inspect_semantic_events() {
        let codes = [
            KeyCode::Char('界'),
            KeyCode::Enter,
            KeyCode::Backspace,
            KeyCode::Delete,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Escape,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Unsupported,
        ];

        for code in codes {
            let event = KeyEvent::new(code)
                .with_modifier(KeyModifiers::SHIFT)
                .with_modifiers(KeyModifiers::ALT | KeyModifiers::CONTROL)
                .with_kind(KeyEventKind::Repeat);

            assert_eq!(event.code(), code);
            assert_eq!(event.modifiers(), KeyModifiers::ALT | KeyModifiers::CONTROL);
            assert_eq!(event.kind(), KeyEventKind::Repeat);
        }
    }

    #[test]
    fn modifier_sets_support_composition_and_queries() {
        let mut modifiers = KeyModifiers::SHIFT.with(KeyModifiers::ALT);
        modifiers |= KeyModifiers::CONTROL;

        assert!(modifiers.contains(KeyModifiers::SHIFT | KeyModifiers::ALT));
        assert!(modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER));
        assert!(!modifiers.contains(KeyModifiers::SUPER));
        assert!(!modifiers.is_empty());
        assert!(KeyModifiers::default().is_empty());
    }

    #[test]
    fn termina_supported_codes_map_through_complete_events() {
        let cases = [
            (TerminaCode::Char('ß'), KeyCode::Char('ß')),
            (TerminaCode::Enter, KeyCode::Enter),
            (TerminaCode::Backspace, KeyCode::Backspace),
            (TerminaCode::Delete, KeyCode::Delete),
            (TerminaCode::Left, KeyCode::Left),
            (TerminaCode::Right, KeyCode::Right),
            (TerminaCode::Up, KeyCode::Up),
            (TerminaCode::Down, KeyCode::Down),
            (TerminaCode::Home, KeyCode::Home),
            (TerminaCode::End, KeyCode::End),
            (TerminaCode::Tab, KeyCode::Tab),
            (TerminaCode::BackTab, KeyCode::BackTab),
            (TerminaCode::Escape, KeyCode::Escape),
            (TerminaCode::PageUp, KeyCode::PageUp),
            (TerminaCode::PageDown, KeyCode::PageDown),
        ];

        for (source, expected) in cases {
            assert_eq!(KeyEvent::from(termina_event(source)).code(), expected);
        }
    }

    #[test]
    fn termina_unsupported_code_families_collapse_to_unsupported() {
        let unsupported = [
            TerminaCode::Insert,
            TerminaCode::KeypadBegin,
            TerminaCode::CapsLock,
            TerminaCode::ScrollLock,
            TerminaCode::NumLock,
            TerminaCode::PrintScreen,
            TerminaCode::Pause,
            TerminaCode::Menu,
            TerminaCode::Null,
            TerminaCode::Function(12),
            TerminaCode::Modifier(ModifierKeyCode::LeftShift),
            TerminaCode::Media(MediaKeyCode::Play),
        ];

        for source in unsupported {
            assert_eq!(
                KeyEvent::from(termina_event(source)).code(),
                KeyCode::Unsupported
            );
        }
    }

    #[test]
    fn termina_semantic_modifiers_map_and_lock_modifiers_are_discarded() {
        let cases = [
            (TerminaModifiers::SHIFT, KeyModifiers::SHIFT),
            (TerminaModifiers::ALT, KeyModifiers::ALT),
            (TerminaModifiers::CONTROL, KeyModifiers::CONTROL),
            (TerminaModifiers::SUPER, KeyModifiers::SUPER),
            (TerminaModifiers::META, KeyModifiers::META),
            (TerminaModifiers::HYPER, KeyModifiers::HYPER),
        ];

        for (source, expected) in cases {
            let mut event = termina_event(TerminaCode::Enter);
            event.modifiers = source;
            assert_eq!(KeyEvent::from(event).modifiers(), expected);
        }

        let mut event = termina_event(TerminaCode::Enter);
        event.modifiers = TerminaModifiers::CAPS_LOCK | TerminaModifiers::NUM_LOCK;
        assert!(KeyEvent::from(event).modifiers().is_empty());
    }

    #[test]
    fn termina_event_kinds_map_and_additional_state_is_discarded() {
        let cases = [
            (TerminaKind::Press, KeyEventKind::Press),
            (TerminaKind::Repeat, KeyEventKind::Repeat),
            (TerminaKind::Release, KeyEventKind::Release),
        ];

        for (source, expected) in cases {
            let mut event = termina_event(TerminaCode::Enter);
            event.kind = source;
            event.state = TerminaState::KEYPAD | TerminaState::CAPS_LOCK | TerminaState::NUM_LOCK;

            let mapped = KeyEvent::from(event);
            assert_eq!(mapped.kind(), expected);
            assert_eq!(mapped, KeyEvent::new(KeyCode::Enter).with_kind(expected));
        }
    }

    #[test]
    fn key_outcomes_have_distinct_textarea_meanings() {
        let outcomes = [
            KeyOutcome::Changed,
            KeyOutcome::Unchanged,
            KeyOutcome::Submit,
            KeyOutcome::Ignored,
        ];

        for (index, outcome) in outcomes.into_iter().enumerate() {
            assert_eq!(
                outcomes.iter().position(|candidate| *candidate == outcome),
                Some(index)
            );
        }
    }

    fn termina_event(code: TerminaCode) -> TerminaEvent {
        TerminaEvent::new(code, TerminaModifiers::NONE)
    }
}
