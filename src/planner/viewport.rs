#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    frame_len: usize,
    height: usize,
    sentinel_row: usize,

    top_row: usize,
    top_line_index: usize,
}

impl Viewport {
    pub fn new(frame_len: usize, height: usize, sentinel_row: usize) -> Self {
        debug_assert!(height > 0, "height must be greater than zero");
        debug_assert!(
            sentinel_row < height,
            "sentinel row must be inside the viewport"
        );

        let mut this = Self {
            frame_len,
            top_line_index: 0,
            top_row: 0,
            height,
            sentinel_row,
        };

        this.calculate_boundary();

        this
    }

    pub fn sentinel_row(&self) -> usize {
        self.sentinel_row
    }

    pub fn frame_len(&self) -> usize {
        self.frame_len
    }

    pub fn is_line_visible(&self, line_index: usize) -> bool {
        debug_assert!(
            line_index <= self.frame_len,
            "line index must refer to an existing line or the insertion position after the last line"
        );

        line_index >= self.top_line_index
    }

    pub fn row_for_index(&self, line_index: usize) -> usize {
        debug_assert!(
            line_index >= self.top_line_index,
            "line index must be within the visible viewport"
        );
        debug_assert!(
            line_index <= self.frame_len,
            "line index must refer to an existing line or the insertion position after the last line"
        );

        self.top_row + (line_index - self.top_line_index)
    }

    pub fn available_space(&self) -> usize {
        self.height.saturating_sub(self.sentinel_row)
    }

    pub fn insert_chunk_len(&self, line_index: usize, len: usize) -> (usize, usize) {
        debug_assert!(
            line_index >= self.top_line_index,
            "line index must be within the visible viewport"
        );
        debug_assert!(
            line_index <= self.frame_len,
            "line index must refer to an existing line or the insertion position after the last line"
        );

        let available_space_above = self.row_for_index(line_index);

        let available_space_below = self.available_space();
        let pre_scroll_up = available_space_above.min(len.saturating_sub(available_space_below));
        let chunk_len = len.min(available_space_below + pre_scroll_up);

        let available_space_below_without_post_scroll =
            self.height.saturating_sub(self.sentinel_row + 1);
        let pre_scroll_up_without_post_scroll = available_space_above
            .min(len.saturating_sub(available_space_below_without_post_scroll));
        let chunk_len_without_post_scroll =
            len.min(available_space_below_without_post_scroll + pre_scroll_up_without_post_scroll);

        if chunk_len_without_post_scroll == chunk_len {
            (
                chunk_len_without_post_scroll,
                pre_scroll_up_without_post_scroll,
            )
        } else {
            (chunk_len, pre_scroll_up)
        }
    }

    pub fn newline(&mut self) {
        let scroll_up = self.insert(1);

        if scroll_up > 0 {
            self.scroll_up(scroll_up);
        }
    }

    pub fn scroll_up(&mut self, rows: usize) {
        self.sentinel_row = self.sentinel_row.saturating_sub(rows);
        self.calculate_boundary();

        debug_assert!(
            self.sentinel_row < self.height,
            "scrolling up must leave the sentinel row inside the viewport"
        );
    }

    pub fn insert(&mut self, rows: usize) -> usize {
        debug_assert!(
            rows <= self.available_space(),
            "insert chunk length must not exceed available viewport space"
        );

        self.frame_len += rows;
        self.sentinel_row += rows;

        self.sentinel_row.saturating_sub(self.height - 1)
    }

    pub fn delete(&mut self, rows: usize) {
        debug_assert!(
            rows <= self.frame_len && rows <= self.sentinel_row,
            "delete chunk length must not exceed frame length or visible lines before the sentinel"
        );

        self.frame_len -= rows;
        self.sentinel_row -= rows;
    }

    fn calculate_boundary(&mut self) {
        self.top_line_index = self.frame_len.saturating_sub(self.sentinel_row);
        self.top_row = self.sentinel_row.saturating_sub(self.frame_len);
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::Viewport;

    #[test]
    fn boundary_tracks_short_and_scrolled_frames() {
        let short = Viewport::new(2, 5, 4);
        assert!(short.is_line_visible(0));
        assert_eq!(short.row_for_index(0), 2);
        assert_eq!(short.row_for_index(2), 4);
        assert_eq!(short.available_space(), 1);

        let scrolled = Viewport::new(8, 5, 4);
        assert!(!scrolled.is_line_visible(3));
        assert!(scrolled.is_line_visible(4));
        assert_eq!(scrolled.row_for_index(4), 0);
        assert_eq!(scrolled.row_for_index(8), 4);
    }

    #[test]
    fn chunk_size_avoids_a_post_scroll_when_possible() {
        let viewport = Viewport::new(2, 5, 2);

        assert_eq!(viewport.insert_chunk_len(1, 1), (1, 0));
        assert_eq!(viewport.insert_chunk_len(2, 3), (3, 1));
        assert_eq!(viewport.insert_chunk_len(0, 3), (3, 0));
    }

    #[test]
    fn insert_reports_required_scroll_and_scroll_updates_boundary() {
        let mut viewport = Viewport::new(2, 3, 2);

        assert_eq!(viewport.insert(1), 1);
        assert_eq!(viewport.frame_len(), 3);
        assert_eq!(viewport.sentinel_row(), 3);

        viewport.scroll_up(1);
        assert_eq!(viewport.sentinel_row(), 2);
        assert_eq!(viewport.row_for_index(1), 0);
    }

    #[test]
    fn newline_inserts_a_line_and_keeps_sentinel_visible() {
        let mut viewport = Viewport::new(2, 3, 2);

        viewport.newline();

        assert_eq!(viewport.frame_len(), 3);
        assert_eq!(viewport.sentinel_row(), 2);
        assert!(!viewport.is_line_visible(0));
    }

    #[test]
    fn delete_reduces_frame_and_sentinel() {
        let mut viewport = Viewport::new(4, 6, 4);

        viewport.delete(2);

        assert_eq!(viewport.frame_len(), 2);
        assert_eq!(viewport.sentinel_row(), 2);
        assert_eq!(viewport.row_for_index(0), 0);
    }
}
