use std::cmp::min;

pub struct Cursor {
    height: usize,
    cursor_row: usize,
}

impl Cursor {
    pub fn new(height: usize, cursor_row: usize) -> Self {
        debug_assert!(height > 0, "height must be greater than zero");
        debug_assert!(cursor_row < height, "cursor row must be visible");

        Self { height, cursor_row }
    }

    pub fn cursor_row(&self) -> usize {
        self.cursor_row
    }

    pub fn move_down(&mut self, rows: usize) {
        self.cursor_row = min(self.cursor_row.saturating_add(rows), self.height - 1);
    }

    pub fn move_up(&mut self, rows: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(rows);
    }
}
