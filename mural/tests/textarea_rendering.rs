mod support;

use mural::widget::Textarea;
use mural_core::{Block, ColorScheme, CursorPosition, Terminal, TerminalSize};
use support::{RecordingBackend, cursor_contents};

#[test]
fn core_block_rendering_drives_output_and_remembered_navigation_width() {
    let backend = RecordingBackend::default();
    let writes = backend.writes();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            height: 10,
            width: 5,
        },
        CursorPosition { row: 0, column: 0 },
        ColorScheme::Dark,
    )
    .unwrap();
    terminal.insert_live("editor", Textarea::from("abcdef"));

    terminal.render().unwrap();
    let output = writes.borrow().concat();
    assert_eq!(cursor_contents(&output), ["a"]);

    let editor = terminal.get_live_mut::<Textarea>("editor").unwrap();
    editor.move_visual_down();
    assert_eq!(editor.cursor(), 3);

    editor.set_value("a\nb");
    terminal
        .resize(TerminalSize {
            height: 10,
            width: 1,
        })
        .unwrap();
    terminal.render().unwrap();

    let editor = terminal.get_live_mut::<Textarea>("editor").unwrap();
    editor.move_visual_down();
    assert_eq!(editor.cursor(), 2);
    assert!(!editor.render_every_frame());
}
