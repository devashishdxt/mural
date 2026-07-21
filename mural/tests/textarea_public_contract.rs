mod support;

use mural::{
    key::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyOutcome},
    widget::Textarea,
};
use mural_core::{Block, ColorScheme, CursorPosition, Terminal, TerminalSize};
use support::{RecordingBackend, cursor_contents};
use termina::event::{
    KeyCode as TerminaCode, KeyEvent as TerminaEvent, KeyEventKind as TerminaKind,
    KeyEventState as TerminaState, Modifiers as TerminaModifiers,
};

#[test]
fn public_editing_and_key_contract_leaves_submission_to_the_application() {
    let mut editor = Textarea::from("\x1b[31mHello\rworld\x1b[0m");
    assert_eq!(editor.value(), "Hello\nworld");

    editor.move_to_buffer_end().insert("!").insert_char('界');
    assert_eq!(editor.value(), "Hello\nworld!界");

    let before_submission = editor.clone();
    let outcome = editor.handle_key_event(KeyEvent::new(KeyCode::Enter));
    assert_eq!(outcome, KeyOutcome::Submit);
    assert_eq!(editor, before_submission);

    let submitted = (outcome == KeyOutcome::Submit).then(|| editor.take());
    assert_eq!(submitted.as_deref(), Some("Hello\nworld!界"));
    assert!(editor.is_empty());
}

#[test]
fn complete_termina_events_convert_at_the_public_boundary() {
    let mut source = TerminaEvent::new(TerminaCode::Char('界'), TerminaModifiers::NONE);
    source.modifiers =
        TerminaModifiers::SHIFT | TerminaModifiers::CONTROL | TerminaModifiers::CAPS_LOCK;
    source.kind = TerminaKind::Repeat;
    source.state = TerminaState::KEYPAD | TerminaState::CAPS_LOCK;

    let mapped = KeyEvent::from(source);
    assert_eq!(mapped.code(), KeyCode::Char('界'));
    assert_eq!(
        mapped.modifiers(),
        KeyModifiers::SHIFT | KeyModifiers::CONTROL
    );
    assert_eq!(mapped.kind(), KeyEventKind::Repeat);

    let mut unsupported = TerminaEvent::new(TerminaCode::Function(12), TerminaModifiers::ALT);
    unsupported.kind = TerminaKind::Release;
    let mapped = KeyEvent::from(unsupported);
    assert_eq!(mapped.code(), KeyCode::Unsupported);
    assert_eq!(mapped.modifiers(), KeyModifiers::ALT);
    assert_eq!(mapped.kind(), KeyEventKind::Release);
}

#[test]
fn standard_traits_include_navigation_width_and_viewport_state() {
    fn assert_traits<T: std::fmt::Debug + Clone + Default + PartialEq + Eq>() {}
    fn assert_block<T: Block>() {}

    assert_traits::<Textarea>();
    assert_block::<Textarea>();
    assert_eq!(Textarea::default(), Textarea::new());

    let mut navigation = Textarea::from("abcd\nx\nabcd");
    navigation.set_cursor(3);
    let unchanged_cursor = navigation.clone();
    navigation.move_visual_up_with_width(10);
    assert_eq!(navigation.value(), unchanged_cursor.value());
    assert_eq!(navigation.cursor(), unchanged_cursor.cursor());
    assert_ne!(navigation, unchanged_cursor);
    assert_eq!(navigation, navigation.clone());

    let (narrow, narrow_output) = rendered_state(Textarea::from("abcdef"), 5);
    let (wide, _) = rendered_state(Textarea::from("abcdef"), 10);
    assert_eq!(cursor_contents(&narrow_output), ["a"]);
    assert_ne!(narrow, wide);

    let mut at_end = Textarea::from("0\n1\n2").max_height(1);
    at_end.set_cursor(usize::MAX);
    let (mut scrolled, _) = rendered_state(at_end, 8);
    scrolled.set_cursor(0);

    let (unscrolled, _) = rendered_state(Textarea::from("0\n1\n2").max_height(1), 8);
    assert_eq!(scrolled.value(), unscrolled.value());
    assert_eq!(scrolled.cursor(), unscrolled.cursor());
    assert_ne!(scrolled, unscrolled);
}

fn rendered_state(textarea: Textarea, terminal_width: usize) -> (Textarea, String) {
    let backend = RecordingBackend::default();
    let writes = backend.writes();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            height: 20,
            width: terminal_width,
        },
        CursorPosition { row: 0, column: 0 },
        ColorScheme::Dark,
    )
    .unwrap();
    terminal.insert_live("editor", textarea);
    terminal.render().unwrap();

    let textarea = terminal
        .get_live_mut::<Textarea>("editor")
        .expect("textarea remains available after rendering")
        .clone();
    let output = writes.borrow().concat();
    (textarea, output)
}
