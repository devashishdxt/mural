mod cursor;
mod viewport;

use std::cmp::min;

use crate::{
    differ::{DiffOp, NormalizedDiff},
    planner::{cursor::Cursor, viewport::Viewport},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOp<'a> {
    // Cursor movement
    MoveUp(usize),
    MoveDown(usize),
    CarriageReturn,
    Newline,

    // Viewport movement
    ScrollUp(usize),

    // Line editing
    InsertLines(usize),
    DeleteLines(usize),
    ClearLine,
    Write(&'a str),

    // Terminal management
    ClearScreen,
    PurgeScrollback,
    MoveToTopLeft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan<'a> {
    ops: Vec<RenderOp<'a>>,
    final_sentinel_row: usize,
}

impl<'a> Plan<'a> {
    pub fn render_ops(&self) -> &[RenderOp<'a>] {
        &self.ops
    }

    pub fn final_sentinel_row(&self) -> usize {
        self.final_sentinel_row
    }

    pub fn full_redraw(lines: &'a [String], height: usize) -> Self {
        let mut ops = Vec::with_capacity((lines.len() * 3) + 3);

        ops.push(RenderOp::ClearScreen);
        ops.push(RenderOp::PurgeScrollback);
        ops.push(RenderOp::MoveToTopLeft);

        for line in lines {
            ops.extend([
                RenderOp::Write(line.as_str()),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
            ]);
        }

        Self {
            ops,
            final_sentinel_row: min(lines.len(), height - 1),
        }
    }

    fn no_changes(final_sentinel_row: usize) -> Self {
        Self {
            ops: Vec::with_capacity(0),
            final_sentinel_row,
        }
    }
}

pub trait Planner<'a> {
    fn plan(self, diff: NormalizedDiff) -> Plan<'a>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultPlanner<'a> {
    old_frame_len: usize,
    new_frame: &'a [String],
    height: usize,
    sentinel_row: usize,
}

impl<'a> DefaultPlanner<'a> {
    pub fn new(
        old_frame_len: usize,
        new_frame: &'a [String],
        height: usize,
        sentinel_row: usize,
    ) -> Self {
        Self {
            old_frame_len,
            new_frame,
            height,
            sentinel_row,
        }
    }
}

impl<'a> Planner<'a> for DefaultPlanner<'a> {
    fn plan(self, diff: NormalizedDiff) -> Plan<'a> {
        if diff.is_empty() {
            return Plan::no_changes(self.sentinel_row);
        }

        let viewport = Viewport::new(self.old_frame_len, self.height, self.sentinel_row);

        if !viewport.is_line_visible(diff[0].old_index()) {
            return Plan::full_redraw(self.new_frame, self.height);
        }

        IncrementalPlanner::new(
            self.old_frame_len,
            self.new_frame,
            self.height,
            self.sentinel_row,
        )
        .plan(diff)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalPlanner<'a> {
    new_frame: &'a [String],
    viewport: Viewport,
    cursor: Cursor,
    render_ops: Vec<RenderOp<'a>>,
}

impl<'a> IncrementalPlanner<'a> {
    pub fn new(
        old_frame_len: usize,
        new_frame: &'a [String],
        height: usize,
        sentinel_row: usize,
    ) -> Self {
        Self {
            new_frame,
            viewport: Viewport::new(old_frame_len, height, sentinel_row),
            cursor: Cursor::new(height, sentinel_row),
            render_ops: Vec::new(),
        }
    }

    fn delete(&mut self, index: usize, len: usize) {
        if len == 0 {
            return;
        }

        debug_assert!(
            index + len <= self.viewport.frame_len(),
            "delete range must fit inside the current frame"
        );

        let row = self.viewport.row_for_index(index);

        self.move_to(row);
        self.render_ops.push(RenderOp::DeleteLines(len));
        self.viewport.delete(len);
    }

    fn insert(&mut self, mut index: usize, mut new_index: usize, mut len: usize) {
        if len == 0 {
            return;
        }

        debug_assert!(
            new_index + len <= self.new_frame.len(),
            "insert range must fit inside the new frame"
        );

        if index == self.viewport.frame_len() {
            self.append(new_index, len);
            return;
        }

        while len > 0 {
            let (chunk_len, pre_scroll_up) = self.viewport.insert_chunk_len(index, len);

            if pre_scroll_up > 0 {
                self.render_ops.push(RenderOp::ScrollUp(pre_scroll_up));
                self.viewport.scroll_up(pre_scroll_up);
            }

            let row = self.viewport.row_for_index(index);

            self.move_to(row);
            self.render_ops.push(RenderOp::InsertLines(chunk_len));
            let scroll_up = self.viewport.insert(chunk_len);

            for i in new_index..new_index + chunk_len {
                self.render_ops.extend([
                    RenderOp::Write(self.new_frame[i].as_str()),
                    RenderOp::CarriageReturn,
                    RenderOp::MoveDown(1),
                ]);
                self.cursor.move_down(1);
            }

            if scroll_up > 0 {
                self.render_ops.push(RenderOp::ScrollUp(scroll_up));
                self.viewport.scroll_up(scroll_up);
            }

            index += chunk_len;
            new_index += chunk_len;
            len = len.saturating_sub(chunk_len);
        }
    }

    fn append(&mut self, new_index: usize, len: usize) {
        self.move_to(self.viewport.sentinel_row());

        for i in new_index..new_index + len {
            self.render_ops.extend([
                RenderOp::Write(self.new_frame[i].as_str()),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
            ]);

            self.cursor.move_down(1);
            self.viewport.newline();
        }
    }

    fn replace(&mut self, index: usize, len: usize, new_index: usize, new_len: usize) {
        let shared_len = min(len, new_len);

        self.rewrite(index, new_index, shared_len);

        if new_len > len {
            self.insert(
                index + shared_len,
                new_index + shared_len,
                new_len - shared_len,
            );
        } else if len > new_len {
            self.delete(index + shared_len, len - shared_len);
        }
    }

    fn rewrite(&mut self, index: usize, new_index: usize, len: usize) {
        if len == 0 {
            return;
        }

        debug_assert!(
            index + len <= self.viewport.frame_len(),
            "rewrite range must fit inside the current frame"
        );
        debug_assert!(
            new_index + len <= self.new_frame.len(),
            "rewrite range must fit inside the new frame"
        );

        let row = self.viewport.row_for_index(index);
        self.move_to(row);

        for i in new_index..new_index + len {
            self.render_ops.extend([
                RenderOp::ClearLine,
                RenderOp::Write(self.new_frame[i].as_str()),
                RenderOp::CarriageReturn,
                RenderOp::MoveDown(1),
            ]);

            self.cursor.move_down(1);
        }
    }

    fn move_to(&mut self, row: usize) {
        let cursor_row = self.cursor.cursor_row();

        if row == cursor_row {
        } else if row > cursor_row {
            let rows = row - cursor_row;

            self.render_ops
                .extend([RenderOp::MoveDown(rows), RenderOp::CarriageReturn]);
            self.cursor.move_down(rows);
        } else if row < cursor_row {
            let rows = cursor_row - row;

            self.render_ops
                .extend([RenderOp::MoveUp(rows), RenderOp::CarriageReturn]);
            self.cursor.move_up(rows);
        }
    }
}

impl<'a> Planner<'a> for IncrementalPlanner<'a> {
    fn plan(mut self, diff: NormalizedDiff) -> Plan<'a> {
        for diff_op in diff {
            match diff_op {
                DiffOp::Delete {
                    old_index, old_len, ..
                } => self.delete(old_index, old_len),
                DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => self.insert(old_index, new_index, new_len),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => self.replace(old_index, old_len, new_index, new_len),
            }
        }

        self.move_to(self.viewport.sentinel_row());

        Plan {
            ops: self.render_ops,
            final_sentinel_row: self.viewport.sentinel_row(),
        }
    }
}
