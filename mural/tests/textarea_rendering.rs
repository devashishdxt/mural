use std::{cell::RefCell, convert::Infallible, rc::Rc};

use mural::widget::Textarea;
use mural_core::{Backend, Block, ColorScheme, CursorPosition, Terminal, TerminalSize};

#[derive(Default)]
struct RecordingBackend {
    writes: Rc<RefCell<Vec<String>>>,
}

impl Backend for RecordingBackend {
    type Error = Infallible;

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn move_up(&mut self, _n: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn move_down(&mut self, _n: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn carriage_return(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newline(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn scroll_up(&mut self, _n: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn insert_lines(&mut self, _n: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete_lines(&mut self, _n: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn clear_line(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.writes.borrow_mut().push(text.to_owned());
        Ok(())
    }

    fn clear_screen(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn core_block_rendering_drives_output_and_remembered_navigation_width() {
    let backend = RecordingBackend::default();
    let writes = Rc::clone(&backend.writes);
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
    assert!(writes.borrow().iter().any(|line| line.contains("\x1b[7m")));

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
