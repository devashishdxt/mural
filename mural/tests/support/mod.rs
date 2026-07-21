use std::{cell::RefCell, convert::Infallible, rc::Rc};

use mural_core::Backend;

const CURSOR_ON: &str = "\x1b[7m";
const CURSOR_OFF: &str = "\x1b[27m";

#[derive(Default)]
pub struct RecordingBackend {
    writes: Rc<RefCell<Vec<String>>>,
}

impl RecordingBackend {
    pub fn writes(&self) -> Rc<RefCell<Vec<String>>> {
        Rc::clone(&self.writes)
    }
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

/// Returns the user-visible content covered by each balanced software cursor sequence.
pub fn cursor_contents(output: &str) -> Vec<&str> {
    let mut contents = Vec::new();
    let mut remaining = output;

    while let Some(start) = remaining.find(CURSOR_ON) {
        remaining = &remaining[start + CURSOR_ON.len()..];
        let end = remaining
            .find(CURSOR_OFF)
            .expect("software cursor must be reset in the same output stream");
        contents.push(&remaining[..end]);
        remaining = &remaining[end + CURSOR_OFF.len()..];
    }

    contents
}
