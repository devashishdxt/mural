mod cursor;
mod viewport;

use std::cmp::min;

use crate::{
    differ::{DiffOp, NormalizedDiff},
    frame::Frame,
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

    pub fn full_redraw(frame: &'a Frame, height: usize) -> Self {
        let mut ops = Vec::with_capacity((frame.len() * 3) + 3);

        ops.push(RenderOp::ClearScreen);
        ops.push(RenderOp::PurgeScrollback);
        ops.push(RenderOp::MoveToTopLeft);

        for line in frame.iter() {
            ops.extend([
                RenderOp::Write(line),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
            ]);
        }

        Self {
            ops,
            final_sentinel_row: min(frame.len(), height - 1),
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
    new_frame: &'a Frame,
    height: usize,
    sentinel_row: usize,
}

impl<'a> DefaultPlanner<'a> {
    pub fn new(
        old_frame_len: usize,
        new_frame: &'a Frame,
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
    new_frame: &'a Frame,
    viewport: Viewport,
    cursor: Cursor,
    render_ops: Vec<RenderOp<'a>>,
}

impl<'a> IncrementalPlanner<'a> {
    pub fn new(
        old_frame_len: usize,
        new_frame: &'a Frame,
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
                    RenderOp::Write(&self.new_frame[i]),
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
                RenderOp::Write(&self.new_frame[i]),
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
                RenderOp::Write(&self.new_frame[i]),
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::rc::Rc;

    use similar::DiffOp as SimilarDiffOp;

    use super::{DefaultPlanner, IncrementalPlanner, Plan, Planner, RenderOp};
    use crate::{
        differ::{Diff, NormalizedDiff},
        frame::{Frame, RenderedLines},
    };

    fn frame(lines: &[&str]) -> Frame {
        [Rc::new(
            lines
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<RenderedLines>(),
        )]
        .into_iter()
        .collect()
    }

    fn diff(operations: impl IntoIterator<Item = SimilarDiffOp>) -> NormalizedDiff {
        operations.into_iter().collect::<Diff>().normalize()
    }

    #[test]
    fn full_redraw_resets_terminal_and_writes_every_line() {
        let frame = frame(&["one", "two"]);

        let plan = Plan::full_redraw(&frame, 2);

        assert_eq!(
            plan.render_ops(),
            [
                RenderOp::ClearScreen,
                RenderOp::PurgeScrollback,
                RenderOp::MoveToTopLeft,
                RenderOp::Write("one"),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
                RenderOp::Write("two"),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
            ]
        );
        assert_eq!(plan.final_sentinel_row(), 1);
    }

    #[test]
    fn unchanged_frames_produce_no_operations() {
        let frame = frame(&["same"]);
        let plan = DefaultPlanner::new(1, &frame, 4, 1).plan(diff([]));

        assert!(plan.render_ops().is_empty());
        assert_eq!(plan.final_sentinel_row(), 1);
    }

    #[test]
    fn an_offscreen_first_edit_requires_a_full_redraw() {
        let frame = frame(&["remaining"]);
        let changes = diff([SimilarDiffOp::Delete {
            old_index: 1,
            old_len: 1,
            new_index: 0,
        }]);

        let plan = DefaultPlanner::new(6, &frame, 3, 2).plan(changes);

        assert_eq!(plan.render_ops()[0], RenderOp::ClearScreen);
        assert_eq!(plan.final_sentinel_row(), 1);
    }

    #[test]
    fn visible_insert_is_planned_incrementally() {
        let frame = frame(&["a", "inserted", "b"]);
        let changes = diff([SimilarDiffOp::Insert {
            old_index: 1,
            new_index: 1,
            new_len: 1,
        }]);

        let plan = DefaultPlanner::new(2, &frame, 5, 2).plan(changes);

        assert_eq!(
            plan.render_ops(),
            [
                RenderOp::MoveUp(1),
                RenderOp::CarriageReturn,
                RenderOp::InsertLines(1),
                RenderOp::Write("inserted"),
                RenderOp::CarriageReturn,
                RenderOp::MoveDown(1),
                RenderOp::MoveDown(1),
                RenderOp::CarriageReturn,
            ]
        );
        assert_eq!(plan.final_sentinel_row(), 3);
    }

    #[test]
    fn append_uses_newlines_to_follow_terminal_scrolling() {
        let frame = frame(&["a", "b", "c", "d"]);
        let changes = diff([SimilarDiffOp::Insert {
            old_index: 2,
            new_index: 2,
            new_len: 2,
        }]);

        let plan = IncrementalPlanner::new(2, &frame, 3, 2).plan(changes);

        assert_eq!(
            plan.render_ops(),
            [
                RenderOp::Write("c"),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
                RenderOp::Write("d"),
                RenderOp::CarriageReturn,
                RenderOp::Newline,
            ]
        );
        assert_eq!(plan.final_sentinel_row(), 2);
    }

    #[test]
    fn delete_moves_to_the_edit_then_returns_to_the_sentinel() {
        let frame = frame(&["a", "c"]);
        let changes = diff([SimilarDiffOp::Delete {
            old_index: 1,
            old_len: 1,
            new_index: 1,
        }]);

        let plan = IncrementalPlanner::new(3, &frame, 5, 3).plan(changes);

        assert_eq!(
            plan.render_ops(),
            [
                RenderOp::MoveUp(2),
                RenderOp::CarriageReturn,
                RenderOp::DeleteLines(1),
                RenderOp::MoveDown(1),
                RenderOp::CarriageReturn,
            ]
        );
        assert_eq!(plan.final_sentinel_row(), 2);
    }

    #[test]
    fn equal_length_replace_rewrites_lines() {
        let frame = frame(&["new a", "new b"]);
        let changes = diff([SimilarDiffOp::Replace {
            old_index: 0,
            old_len: 2,
            new_index: 0,
            new_len: 2,
        }]);

        let plan = IncrementalPlanner::new(2, &frame, 5, 2).plan(changes);

        assert_eq!(
            plan.render_ops(),
            [
                RenderOp::MoveUp(2),
                RenderOp::CarriageReturn,
                RenderOp::ClearLine,
                RenderOp::Write("new a"),
                RenderOp::CarriageReturn,
                RenderOp::MoveDown(1),
                RenderOp::ClearLine,
                RenderOp::Write("new b"),
                RenderOp::CarriageReturn,
                RenderOp::MoveDown(1),
            ]
        );
    }

    #[test]
    fn growing_and_shrinking_replacements_use_line_edits() {
        let grown = frame(&["new", "added", "tail"]);
        let grow = diff([SimilarDiffOp::Replace {
            old_index: 0,
            old_len: 1,
            new_index: 0,
            new_len: 2,
        }]);
        let grow_plan = IncrementalPlanner::new(2, &grown, 5, 2).plan(grow);
        assert!(grow_plan.render_ops().contains(&RenderOp::InsertLines(1)));
        assert!(grow_plan.render_ops().contains(&RenderOp::Write("added")));

        let shrunk = frame(&["new"]);
        let shrink = diff([SimilarDiffOp::Replace {
            old_index: 0,
            old_len: 2,
            new_index: 0,
            new_len: 1,
        }]);
        let shrink_plan = IncrementalPlanner::new(2, &shrunk, 5, 2).plan(shrink);
        assert!(shrink_plan.render_ops().contains(&RenderOp::DeleteLines(1)));
    }

    #[test]
    fn large_middle_insert_is_split_and_scrolled() {
        let frame = frame(&["old", "a", "b", "c", "d"]);
        let changes = diff([SimilarDiffOp::Insert {
            old_index: 1,
            new_index: 1,
            new_len: 4,
        }]);

        let plan = IncrementalPlanner::new(2, &frame, 3, 2).plan(changes);

        assert_eq!(
            plan.render_ops()
                .iter()
                .filter(|operation| matches!(operation, RenderOp::Write(_)))
                .count(),
            4
        );
        assert!(
            plan.render_ops()
                .iter()
                .any(|operation| matches!(operation, RenderOp::ScrollUp(_)))
        );
        assert_eq!(plan.final_sentinel_row(), 2);
    }

    #[test]
    fn zero_length_operations_are_ignored() {
        let frame = frame(&["same"]);
        let changes = diff([
            SimilarDiffOp::Delete {
                old_index: 0,
                old_len: 0,
                new_index: 0,
            },
            SimilarDiffOp::Insert {
                old_index: 0,
                new_index: 0,
                new_len: 0,
            },
            SimilarDiffOp::Replace {
                old_index: 0,
                old_len: 0,
                new_index: 0,
                new_len: 0,
            },
        ]);

        let plan = IncrementalPlanner::new(1, &frame, 3, 1).plan(changes);

        assert!(plan.render_ops().is_empty());
        assert_eq!(plan.final_sentinel_row(), 1);
    }
}
