use crossterm::event as crossterm_event;
use mural::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[test]
fn key_event_maps_from_crossterm_key_events() {
    let event = crossterm_event::KeyEvent::new_with_kind(
        crossterm_event::KeyCode::Char('x'),
        crossterm_event::KeyModifiers::CONTROL
            | crossterm_event::KeyModifiers::ALT
            | crossterm_event::KeyModifiers::SUPER
            | crossterm_event::KeyModifiers::META
            | crossterm_event::KeyModifiers::HYPER,
        crossterm_event::KeyEventKind::Repeat,
    );

    let mapped = KeyEvent::from(event);

    assert_eq!(mapped.code(), KeyCode::Char('x'));
    assert_eq!(mapped.kind_value(), KeyEventKind::Repeat);
    assert!(mapped.modifiers().contains(KeyModifiers::CONTROL));
    assert!(mapped.modifiers().contains(KeyModifiers::ALT));
    assert!(mapped.modifiers().contains(KeyModifiers::SUPER));
    assert!(mapped.modifiers().contains(KeyModifiers::META));
    assert!(mapped.modifiers().contains(KeyModifiers::HYPER));
}
