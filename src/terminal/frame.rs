use crate::{CursorPosition, region::Region};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewportState {
    pub(super) first_visible_managed_row: isize,
    pub(super) cursor_managed_row: usize,
}

impl ViewportState {
    pub(super) fn initial(position: CursorPosition) -> Self {
        Self {
            first_visible_managed_row: -(position.row as isize),
            cursor_managed_row: 0,
        }
    }

    pub(super) fn after_full_redraw(content_len: usize, height: usize) -> Self {
        Self {
            first_visible_managed_row: content_len.saturating_sub(height.saturating_sub(1))
                as isize,
            cursor_managed_row: content_len,
        }
    }

    pub(super) fn after_newlines(self, count: usize, height: usize) -> Self {
        let cursor_managed_row = self.cursor_managed_row + count;
        Self {
            first_visible_managed_row: self
                .first_visible_managed_row
                .max(cursor_managed_row as isize - height.saturating_sub(1) as isize),
            cursor_managed_row,
        }
    }

    pub(super) fn with_cursor_managed_row(self, cursor_managed_row: usize) -> Self {
        Self {
            first_visible_managed_row: self.first_visible_managed_row,
            cursor_managed_row,
        }
    }

    pub(super) fn cursor_is_visible(self, height: usize) -> bool {
        self.cursor_managed_row as isize >= self.first_visible_managed_row
            && self.visible_cursor_row() < height
    }

    pub(super) fn visible_cursor_row(self) -> usize {
        (self.cursor_managed_row as isize - self.first_visible_managed_row) as usize
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommittedFrame {
    pub(super) lines: Vec<String>,
    pub(super) viewport: ViewportState,
}

pub(super) fn current_frame(
    live_blocks: &mut Region,
    pinned_blocks: &mut Region,
    width: usize,
) -> Vec<String> {
    let mut lines = live_blocks.render_lines(width);
    lines.extend(pinned_blocks.render_lines(width));
    lines
}

pub(super) fn frame_changed(last_frame: &[String], current_frame: &[String]) -> bool {
    last_frame.len() != current_frame.len()
        || last_frame
            .iter()
            .zip(current_frame)
            .any(|(last, current)| last != current)
}

pub(super) fn is_append_only(last_frame: &[String], current_frame: &[String]) -> bool {
    current_frame.len() > last_frame.len()
        && current_frame
            .iter()
            .zip(last_frame)
            .all(|(current, last)| current == last)
}
