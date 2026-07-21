use mural::{
    key::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyOutcome},
    widget::Textarea,
};

#[derive(Clone, Copy)]
struct BindingCase {
    name: &'static str,
    event: KeyEvent,
    initial: &'static str,
    cursor: usize,
    expected_value: &'static str,
    expected_cursor: usize,
    expected_outcome: KeyOutcome,
}

#[test]
fn every_default_binding_handles_press_and_repeat_but_not_release() {
    let cases = [
        binding(
            "character",
            KeyCode::Char('界'),
            KeyModifiers::empty(),
            "ab",
            1,
            "a界b",
            4,
        ),
        binding(
            "shift character",
            KeyCode::Char('X'),
            KeyModifiers::SHIFT,
            "ab",
            1,
            "aXb",
            2,
        ),
        BindingCase {
            name: "plain enter",
            event: KeyEvent::new(KeyCode::Enter),
            initial: "ab",
            cursor: 1,
            expected_value: "ab",
            expected_cursor: 1,
            expected_outcome: KeyOutcome::Submit,
        },
        binding(
            "shift enter",
            KeyCode::Enter,
            KeyModifiers::SHIFT,
            "ab",
            1,
            "a\nb",
            2,
        ),
        binding(
            "alt enter",
            KeyCode::Enter,
            KeyModifiers::ALT,
            "ab",
            1,
            "a\nb",
            2,
        ),
        binding(
            "shift alt enter",
            KeyCode::Enter,
            KeyModifiers::SHIFT | KeyModifiers::ALT,
            "ab",
            1,
            "a\nb",
            2,
        ),
        binding(
            "backspace",
            KeyCode::Backspace,
            KeyModifiers::empty(),
            "ab",
            2,
            "a",
            1,
        ),
        binding(
            "delete",
            KeyCode::Delete,
            KeyModifiers::empty(),
            "ab",
            0,
            "b",
            0,
        ),
        binding(
            "left",
            KeyCode::Left,
            KeyModifiers::empty(),
            "ab",
            2,
            "ab",
            1,
        ),
        binding(
            "right",
            KeyCode::Right,
            KeyModifiers::empty(),
            "ab",
            0,
            "ab",
            1,
        ),
        binding(
            "up",
            KeyCode::Up,
            KeyModifiers::empty(),
            "abc\ndef",
            5,
            "abc\ndef",
            1,
        ),
        binding(
            "down",
            KeyCode::Down,
            KeyModifiers::empty(),
            "abc\ndef",
            1,
            "abc\ndef",
            5,
        ),
        binding(
            "home",
            KeyCode::Home,
            KeyModifiers::empty(),
            "abc\ndef",
            5,
            "abc\ndef",
            4,
        ),
        binding(
            "end",
            KeyCode::End,
            KeyModifiers::empty(),
            "abc\ndef",
            5,
            "abc\ndef",
            7,
        ),
        binding(
            "control left",
            KeyCode::Left,
            KeyModifiers::CONTROL,
            "one two",
            7,
            "one two",
            4,
        ),
        binding(
            "alt left",
            KeyCode::Left,
            KeyModifiers::ALT,
            "one two",
            7,
            "one two",
            4,
        ),
        binding(
            "control right",
            KeyCode::Right,
            KeyModifiers::CONTROL,
            "one two",
            0,
            "one two",
            3,
        ),
        binding(
            "alt right",
            KeyCode::Right,
            KeyModifiers::ALT,
            "one two",
            0,
            "one two",
            3,
        ),
        binding(
            "alt b",
            KeyCode::Char('b'),
            KeyModifiers::ALT,
            "one two",
            7,
            "one two",
            4,
        ),
        binding(
            "alt shift B",
            KeyCode::Char('B'),
            KeyModifiers::ALT | KeyModifiers::SHIFT,
            "one two",
            7,
            "one two",
            4,
        ),
        binding(
            "alt f",
            KeyCode::Char('f'),
            KeyModifiers::ALT,
            "one two",
            0,
            "one two",
            3,
        ),
        binding(
            "alt shift F",
            KeyCode::Char('F'),
            KeyModifiers::ALT | KeyModifiers::SHIFT,
            "one two",
            0,
            "one two",
            3,
        ),
        binding(
            "control a",
            KeyCode::Char('a'),
            KeyModifiers::CONTROL,
            "abc\ndef",
            5,
            "abc\ndef",
            4,
        ),
        binding(
            "control shift A",
            KeyCode::Char('A'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            "abc\ndef",
            5,
            "abc\ndef",
            4,
        ),
        binding(
            "control e",
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
            "abc\ndef",
            5,
            "abc\ndef",
            7,
        ),
        binding(
            "control shift E",
            KeyCode::Char('E'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            "abc\ndef",
            5,
            "abc\ndef",
            7,
        ),
        binding(
            "control home",
            KeyCode::Home,
            KeyModifiers::CONTROL,
            "abc\ndef",
            5,
            "abc\ndef",
            0,
        ),
        binding(
            "control end",
            KeyCode::End,
            KeyModifiers::CONTROL,
            "abc\ndef",
            1,
            "abc\ndef",
            7,
        ),
        binding(
            "tab",
            KeyCode::Tab,
            KeyModifiers::empty(),
            "ab",
            1,
            "a\tb",
            2,
        ),
    ];

    for case in cases {
        for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
            let mut textarea = initialized(case);
            let outcome = textarea.handle_key_event(case.event.with_kind(kind));

            assert_eq!(outcome, case.expected_outcome, "{} {kind:?}", case.name);
            assert_eq!(
                textarea.value(),
                case.expected_value,
                "{} {kind:?}",
                case.name
            );
            assert_eq!(
                textarea.cursor(),
                case.expected_cursor,
                "{} {kind:?}",
                case.name
            );
        }

        let mut textarea = initialized(case);
        let unchanged = textarea.clone();
        assert_eq!(
            textarea.handle_key_event(case.event.with_kind(KeyEventKind::Release)),
            KeyOutcome::Ignored,
            "{} release",
            case.name
        );
        assert_eq!(textarea, unchanged, "{} release mutated state", case.name);
    }
}

#[test]
fn command_modifiers_gate_character_insertion_and_unbound_events_are_ignored() {
    let command_modifiers = [
        KeyModifiers::ALT,
        KeyModifiers::CONTROL,
        KeyModifiers::SUPER,
        KeyModifiers::META,
        KeyModifiers::HYPER,
        KeyModifiers::SHIFT | KeyModifiers::META,
        KeyModifiers::ALT | KeyModifiers::CONTROL,
    ];

    let mut events = command_modifiers
        .map(|modifiers| KeyEvent::new(KeyCode::Char('x')).with_modifiers(modifiers))
        .to_vec();
    events.extend([
        KeyEvent::new(KeyCode::Char('a')).with_modifiers(KeyModifiers::CONTROL | KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('b')).with_modifiers(KeyModifiers::ALT | KeyModifiers::SUPER),
        KeyEvent::new(KeyCode::Enter).with_modifier(KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::BackTab),
        KeyEvent::new(KeyCode::Escape),
        KeyEvent::new(KeyCode::PageUp),
        KeyEvent::new(KeyCode::PageDown),
        KeyEvent::new(KeyCode::Unsupported),
    ]);

    for event in events {
        let mut textarea = Textarea::from("unchanged");
        textarea.set_cursor(3);
        let initial = textarea.clone();

        assert_eq!(
            textarea.handle_key_event(event),
            KeyOutcome::Ignored,
            "{event:?}"
        );
        assert_eq!(textarea, initial, "{event:?} mutated state");
    }
}

#[test]
fn recognized_operations_report_unchanged_at_boundaries() {
    let cases = [
        (KeyEvent::new(KeyCode::Backspace), "x", 0),
        (KeyEvent::new(KeyCode::Delete), "x", 1),
        (KeyEvent::new(KeyCode::Left), "x", 0),
        (KeyEvent::new(KeyCode::Right), "x", 1),
        (KeyEvent::new(KeyCode::Up), "x", 0),
        (KeyEvent::new(KeyCode::Down), "x", 1),
        (KeyEvent::new(KeyCode::Home), "x", 0),
        (KeyEvent::new(KeyCode::End), "x", 1),
        (
            KeyEvent::new(KeyCode::Left).with_modifier(KeyModifiers::CONTROL),
            "one",
            0,
        ),
        (
            KeyEvent::new(KeyCode::Right).with_modifier(KeyModifiers::ALT),
            "one",
            3,
        ),
        (
            KeyEvent::new(KeyCode::Char('b')).with_modifier(KeyModifiers::ALT),
            "one",
            0,
        ),
        (
            KeyEvent::new(KeyCode::Char('f')).with_modifier(KeyModifiers::ALT),
            "one",
            3,
        ),
        (
            KeyEvent::new(KeyCode::Char('a')).with_modifier(KeyModifiers::CONTROL),
            "one",
            0,
        ),
        (
            KeyEvent::new(KeyCode::Char('e')).with_modifier(KeyModifiers::CONTROL),
            "one",
            3,
        ),
        (
            KeyEvent::new(KeyCode::Home).with_modifier(KeyModifiers::CONTROL),
            "one",
            0,
        ),
        (
            KeyEvent::new(KeyCode::End).with_modifier(KeyModifiers::CONTROL),
            "one",
            3,
        ),
    ];

    for (event, value, cursor) in cases {
        let mut textarea = Textarea::from(value);
        textarea.set_cursor(cursor);

        assert_eq!(
            textarea.handle_key_event(event),
            KeyOutcome::Unchanged,
            "{event:?}"
        );
        assert_eq!(textarea.value(), value);
        assert_eq!(textarea.cursor(), cursor);
    }
}

const fn binding(
    name: &'static str,
    code: KeyCode,
    modifiers: KeyModifiers,
    initial: &'static str,
    cursor: usize,
    expected_value: &'static str,
    expected_cursor: usize,
) -> BindingCase {
    BindingCase {
        name,
        event: KeyEvent::new(code).with_modifiers(modifiers),
        initial,
        cursor,
        expected_value,
        expected_cursor,
        expected_outcome: KeyOutcome::Changed,
    }
}

fn initialized(case: BindingCase) -> Textarea {
    let mut textarea = Textarea::from(case.initial);
    textarea.set_cursor(case.cursor);
    textarea
}
