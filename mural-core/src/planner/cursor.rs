use std::cmp::min;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::Cursor;

    #[test]
    fn movement_is_clamped_to_the_viewport() {
        let mut cursor = Cursor::new(5, 2);

        cursor.move_down(1);
        assert_eq!(cursor.cursor_row(), 3);

        cursor.move_down(usize::MAX);
        assert_eq!(cursor.cursor_row(), 4);

        cursor.move_up(2);
        assert_eq!(cursor.cursor_row(), 2);

        cursor.move_up(usize::MAX);
        assert_eq!(cursor.cursor_row(), 0);
    }
}
