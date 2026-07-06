use std::ops::Range;

use similar::{Algorithm, DiffOp, capture_diff_slices};

use crate::{Backend, TerminalError};

use super::frame::{CommittedFrame, ViewportState, frame_changed, is_append_only};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PlannedOperation<'a> {
    MoveUp(usize),
    MoveDown(usize),
    CarriageReturn,
    ClearLine,
    InsertLines(usize),
    DeleteLines(usize),
    ScrollUp(usize),
    Write(&'a str),
    Newline,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum FramePlan<'a> {
    NoChanges,
    Appended(&'a [String]),
    ChangedLines(Vec<PlannedOperation<'a>>),
    FullRedraw,
}

impl PlannedOperation<'_> {
    fn execute<B: Backend>(&self, backend: &mut B) -> Result<(), TerminalError<B::Error>> {
        match self {
            PlannedOperation::MoveUp(count) => backend.move_up(*count)?,
            PlannedOperation::MoveDown(count) => backend.move_down(*count)?,
            PlannedOperation::CarriageReturn => backend.carriage_return()?,
            PlannedOperation::ClearLine => backend.clear_line()?,
            PlannedOperation::InsertLines(count) => backend.insert_lines(*count)?,
            PlannedOperation::DeleteLines(count) => backend.delete_lines(*count)?,
            PlannedOperation::ScrollUp(count) => backend.scroll_up(*count)?,
            PlannedOperation::Write(line) => backend.write_str(line)?,
            PlannedOperation::Newline => backend.newline()?,
        }

        Ok(())
    }
}

struct PlannedFrameRender<'a> {
    frame_plan: FramePlan<'a>,
    viewport: ViewportState,
}

impl<'a> PlannedFrameRender<'a> {
    fn no_changes(viewport: ViewportState) -> Self {
        Self {
            frame_plan: FramePlan::NoChanges,
            viewport,
        }
    }

    fn appended(lines: &'a [String], viewport: ViewportState) -> Self {
        Self {
            frame_plan: FramePlan::Appended(lines),
            viewport,
        }
    }

    fn changed_lines(operations: Vec<PlannedOperation<'a>>, viewport: ViewportState) -> Self {
        Self {
            frame_plan: FramePlan::ChangedLines(operations),
            viewport,
        }
    }

    fn full_redraw(content_len: usize, height: usize) -> Self {
        Self {
            frame_plan: FramePlan::FullRedraw,
            viewport: ViewportState::after_full_redraw(content_len, height),
        }
    }
}

pub(super) fn render_frame_transaction<B: Backend>(
    backend: &mut B,
    last_frame: &CommittedFrame,
    current_frame: &[String],
    height: usize,
    needs_full_redraw: bool,
) -> Result<ViewportState, TerminalError<B::Error>> {
    let planned = plan_frame_render_update(last_frame, current_frame, height, needs_full_redraw);
    match &planned.frame_plan {
        FramePlan::NoChanges => {}
        FramePlan::Appended(lines) => render_appended_lines(backend, lines)?,
        FramePlan::ChangedLines(operations) => render_planned_operations(backend, operations)?,
        FramePlan::FullRedraw => render_full_frame(backend, current_frame)?,
    }

    backend.flush()?;
    Ok(planned.viewport)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub(super) fn plan_frame_render<'a>(
    last_frame: &CommittedFrame,
    current_frame: &'a [String],
    height: usize,
    needs_full_redraw: bool,
) -> FramePlan<'a> {
    plan_frame_render_update(last_frame, current_frame, height, needs_full_redraw).frame_plan
}

fn plan_frame_render_update<'a>(
    last_frame: &CommittedFrame,
    current_frame: &'a [String],
    height: usize,
    needs_full_redraw: bool,
) -> PlannedFrameRender<'a> {
    if needs_full_redraw {
        return PlannedFrameRender::full_redraw(current_frame.len(), height);
    }

    if is_append_only(&last_frame.lines, current_frame) {
        let appended = &current_frame[last_frame.lines.len()..];
        return PlannedFrameRender::appended(
            appended,
            last_frame.viewport.after_newlines(appended.len(), height),
        );
    }

    if !frame_changed(&last_frame.lines, current_frame) {
        return PlannedFrameRender::no_changes(last_frame.viewport);
    }

    let diff = capture_diff_slices(Algorithm::Patience, &last_frame.lines, current_frame);
    match plan_structural_patches_top_down(last_frame.viewport, current_frame, height, &diff) {
        PatchPlan::Planned {
            operations,
            viewport,
        } => {
            return PlannedFrameRender::changed_lines(operations, viewport);
        }
        PatchPlan::NeedsFullRedraw => {
            return PlannedFrameRender::full_redraw(current_frame.len(), height);
        }
        PatchPlan::Unsupported => {}
    }

    PlannedFrameRender::full_redraw(current_frame.len(), height)
}

fn render_planned_operations<B: Backend>(
    backend: &mut B,
    operations: &[PlannedOperation<'_>],
) -> Result<(), TerminalError<B::Error>> {
    for operation in operations {
        operation.execute(backend)?;
    }

    Ok(())
}

fn row_is_visible(viewport: ViewportState, row: usize, height: usize) -> bool {
    row_is_viewport_visible(viewport, row, height) && row <= viewport.cursor_managed_row
}

fn row_is_viewport_visible(viewport: ViewportState, row: usize, height: usize) -> bool {
    let managed_row = row as isize;
    let visible_row = managed_row - viewport.first_visible_managed_row;

    managed_row >= viewport.first_visible_managed_row && visible_row < height as isize
}

fn can_move_cursor_to_viewport_row(
    viewport: ViewportState,
    actual_cursor_row: usize,
    target_row: usize,
    height: usize,
) -> bool {
    row_is_viewport_visible(viewport, actual_cursor_row, height)
        && row_is_viewport_visible(viewport, target_row, height)
}

fn can_move_cursor_to_managed_target(
    viewport: ViewportState,
    actual_cursor_row: usize,
    target_row: usize,
    height: usize,
) -> bool {
    row_is_visible(viewport, target_row, height)
        && can_move_cursor_to_viewport_row(viewport, actual_cursor_row, target_row, height)
}

fn plan_append_operations(lines: &[String]) -> Vec<PlannedOperation<'_>> {
    lines
        .iter()
        .flat_map(|line| {
            [
                PlannedOperation::Write(line.as_str()),
                PlannedOperation::Newline,
            ]
        })
        .collect()
}

enum PatchPlan<'a> {
    Planned {
        operations: Vec<PlannedOperation<'a>>,
        viewport: ViewportState,
    },
    NeedsFullRedraw,
    Unsupported,
}

#[derive(Debug)]
struct PatchPlanningFailed;

type PatchPlanningResult<T> = Result<T, PatchPlanningFailed>;

struct PatchPlanner<'a> {
    current_frame: &'a [String],
    height: usize,
    operations: Vec<PlannedOperation<'a>>,
    viewport: ViewportState,
    actual_cursor_row: usize,
    row_delta: isize,
    trailing_append_ranges: Vec<Range<usize>>,
}

impl<'a> PatchPlanner<'a> {
    fn new(viewport: ViewportState, current_frame: &'a [String], height: usize) -> Self {
        Self {
            current_frame,
            height,
            operations: Vec::new(),
            viewport,
            actual_cursor_row: viewport.cursor_managed_row,
            row_delta: 0,
            trailing_append_ranges: Vec::new(),
        }
    }

    fn plan_patch(&mut self, patch: DiffOp) -> PatchPlanningResult<()> {
        match patch {
            DiffOp::Equal { .. } => Ok(()),
            DiffOp::Delete {
                old_index, old_len, ..
            } => self.plan_delete(old_index..old_index + old_len),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => self.plan_insert(old_index, new_index..new_index + new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => self.plan_replace(
                old_index..old_index + old_len,
                new_index..new_index + new_len,
            ),
        }
    }

    fn finish(mut self) -> PatchPlanningResult<PatchPlan<'a>> {
        self.return_to_sentinel()?;
        self.append_trailing_ranges();

        Ok(PatchPlan::Planned {
            operations: self.operations,
            viewport: self.viewport,
        })
    }

    fn translated_old_row(&self, old_row: usize) -> PatchPlanningResult<usize> {
        (old_row as isize)
            .checked_add(self.row_delta)
            .and_then(|row| row.try_into().ok())
            .ok_or(PatchPlanningFailed)
    }

    fn ensure_managed_target_reachable(&self, target_row: usize) -> PatchPlanningResult<()> {
        if can_move_cursor_to_managed_target(
            self.viewport,
            self.actual_cursor_row,
            target_row,
            self.height,
        ) {
            Ok(())
        } else {
            Err(PatchPlanningFailed)
        }
    }

    fn ensure_viewport_target_reachable(&self, target_row: usize) -> PatchPlanningResult<()> {
        if can_move_cursor_to_viewport_row(
            self.viewport,
            self.actual_cursor_row,
            target_row,
            self.height,
        ) {
            Ok(())
        } else {
            Err(PatchPlanningFailed)
        }
    }

    fn move_to(&mut self, target_row: usize) {
        move_cursor_to_managed_row(
            &mut self.operations,
            &mut self.actual_cursor_row,
            target_row,
        );
    }

    fn plan_changed_line(&mut self, old_row: usize, current_row: usize) -> PatchPlanningResult<()> {
        let target_row = self.translated_old_row(old_row)?;
        self.ensure_managed_target_reachable(target_row)?;

        self.move_to(target_row);
        self.operations.push(PlannedOperation::CarriageReturn);
        self.operations.push(PlannedOperation::ClearLine);
        self.operations.push(PlannedOperation::Write(
            self.current_frame[current_row].as_str(),
        ));
        self.operations.push(PlannedOperation::CarriageReturn);

        Ok(())
    }

    fn plan_replace(
        &mut self,
        old: Range<usize>,
        current: Range<usize>,
    ) -> PatchPlanningResult<()> {
        let changed_line_count = old.len().min(current.len());
        for offset in 0..changed_line_count {
            self.plan_changed_line(old.start + offset, current.start + offset)?;
        }

        if current.len() > changed_line_count {
            self.plan_insert(
                old.start + changed_line_count,
                current.start + changed_line_count..current.end,
            )?;
        } else if old.len() > changed_line_count {
            self.plan_delete(old.start + changed_line_count..old.end)?;
        }

        Ok(())
    }

    fn plan_delete(&mut self, old: Range<usize>) -> PatchPlanningResult<()> {
        let target_row = self.translated_old_row(old.start)?;
        self.ensure_managed_target_reachable(target_row)?;

        let final_sentinel_row = self
            .viewport
            .cursor_managed_row
            .checked_sub(old.len())
            .ok_or(PatchPlanningFailed)?;
        if target_row > final_sentinel_row {
            return Err(PatchPlanningFailed);
        }

        self.move_to(target_row);
        self.operations.push(PlannedOperation::CarriageReturn);
        self.operations
            .push(PlannedOperation::DeleteLines(old.len()));
        self.viewport = self.viewport.with_cursor_managed_row(final_sentinel_row);
        if !self.viewport.cursor_is_visible(self.height) {
            return Err(PatchPlanningFailed);
        }
        self.row_delta -= old.len() as isize;

        Ok(())
    }

    fn plan_insert(&mut self, old_row: usize, current: Range<usize>) -> PatchPlanningResult<()> {
        let target_row = self.translated_old_row(old_row)?;

        if target_row == self.viewport.cursor_managed_row {
            self.trailing_append_ranges.push(current);
            return Ok(());
        }

        if target_row >= self.viewport.cursor_managed_row
            || !row_is_visible(self.viewport, target_row, self.height)
        {
            return Err(PatchPlanningFailed);
        }

        let mut next_target_row = target_row;
        let mut next_current_row = current.start;
        while next_current_row < current.end {
            self.ensure_viewport_target_reachable(next_target_row)?;

            let chunk_len = self.safe_insert_chunk_len(current.end - next_current_row)?;
            let final_sentinel_row = self
                .viewport
                .cursor_managed_row
                .checked_add(chunk_len)
                .ok_or(PatchPlanningFailed)?;

            self.plan_insert_chunk(next_target_row, next_current_row, chunk_len);
            self.viewport = self.viewport.with_cursor_managed_row(final_sentinel_row);
            self.row_delta += chunk_len as isize;
            next_target_row += chunk_len;
            next_current_row += chunk_len;

            self.scroll_up_to_reveal_cursor()?;
        }

        Ok(())
    }

    fn safe_insert_chunk_len(&self, remaining: usize) -> PatchPlanningResult<usize> {
        let bottom_visible_row =
            self.viewport.first_visible_managed_row + self.height.saturating_sub(1) as isize;
        let safe_capacity = bottom_visible_row - self.viewport.cursor_managed_row as isize + 1;

        if safe_capacity <= 0 {
            return Err(PatchPlanningFailed);
        }

        Ok(remaining.min(safe_capacity as usize))
    }

    fn plan_insert_chunk(&mut self, target_row: usize, current_start: usize, chunk_len: usize) {
        self.move_to(target_row);
        self.operations.push(PlannedOperation::CarriageReturn);
        self.operations
            .push(PlannedOperation::InsertLines(chunk_len));

        for offset in 0..chunk_len {
            if offset > 0 {
                self.operations.push(PlannedOperation::Newline);
            }
            let current_row = current_start + offset;
            self.operations.push(PlannedOperation::ClearLine);
            self.operations.push(PlannedOperation::Write(
                self.current_frame[current_row].as_str(),
            ));
        }
        self.operations.push(PlannedOperation::CarriageReturn);

        self.actual_cursor_row = target_row + chunk_len.saturating_sub(1);
    }

    fn scroll_up_to_reveal_cursor(&mut self) -> PatchPlanningResult<()> {
        let scroll_up_count = scroll_up_count_to_reveal_cursor(self.viewport, self.height);
        if scroll_up_count == 0 {
            return Ok(());
        }

        self.operations
            .push(PlannedOperation::ScrollUp(scroll_up_count));
        self.viewport.first_visible_managed_row += scroll_up_count as isize;
        self.actual_cursor_row = self
            .actual_cursor_row
            .checked_add(scroll_up_count)
            .ok_or(PatchPlanningFailed)?;

        Ok(())
    }

    fn return_to_sentinel(&mut self) -> PatchPlanningResult<()> {
        self.ensure_viewport_target_reachable(self.viewport.cursor_managed_row)?;
        self.move_to(self.viewport.cursor_managed_row);
        Ok(())
    }

    fn append_trailing_ranges(&mut self) {
        if self.trailing_append_ranges.is_empty() {
            return;
        }

        let mut ranges = std::mem::take(&mut self.trailing_append_ranges);
        ranges.sort_by_key(|range| range.start);

        let appended_len = ranges.iter().map(std::ops::Range::len).sum();
        for range in ranges {
            self.operations
                .extend(plan_append_operations(&self.current_frame[range]));
        }
        self.viewport = self.viewport.after_newlines(appended_len, self.height);
    }
}

fn move_cursor_to_managed_row(
    operations: &mut Vec<PlannedOperation<'_>>,
    actual_cursor_row: &mut usize,
    target_row: usize,
) {
    if target_row < *actual_cursor_row {
        operations.push(PlannedOperation::MoveUp(*actual_cursor_row - target_row));
    } else if target_row > *actual_cursor_row {
        operations.push(PlannedOperation::MoveDown(target_row - *actual_cursor_row));
    }
    *actual_cursor_row = target_row;
}

fn plan_structural_patches_top_down<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    diff: &[DiffOp],
) -> PatchPlan<'a> {
    if diff.is_empty() {
        return PatchPlan::Unsupported;
    }

    let mut planner = PatchPlanner::new(viewport, current_frame, height);
    for patch in diff {
        if planner.plan_patch(*patch).is_err() {
            return PatchPlan::NeedsFullRedraw;
        }
    }

    planner.finish().unwrap_or(PatchPlan::NeedsFullRedraw)
}

fn scroll_up_count_to_reveal_cursor(viewport: ViewportState, height: usize) -> usize {
    viewport
        .visible_cursor_row()
        .saturating_sub(height.saturating_sub(1))
}

fn render_appended_lines<B: Backend>(
    backend: &mut B,
    lines: &[String],
) -> Result<(), TerminalError<B::Error>> {
    for line in lines {
        backend.write_str(line)?;
        backend.newline()?;
    }

    Ok(())
}

fn render_full_frame<B: Backend>(
    backend: &mut B,
    frame: &[String],
) -> Result<(), TerminalError<B::Error>> {
    backend.clear_screen()?;
    backend.purge_scrollback()?;
    backend.move_to_top_left()?;

    for line in frame {
        backend.write_str(line)?;
        backend.newline()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_planner_coalesces_multiple_trailing_append_ranges_at_final_sentinel() {
        let viewport = ViewportState {
            first_visible_managed_row: 0,
            cursor_managed_row: 2,
        };
        let current_frame = vec![
            "new".to_owned(),
            "stable".to_owned(),
            "tail one".to_owned(),
            "tail two".to_owned(),
        ];
        let diff = vec![
            DiffOp::Replace {
                old_index: 0,
                old_len: 1,
                new_index: 0,
                new_len: 1,
            },
            DiffOp::Insert {
                old_index: 2,
                new_index: 2,
                new_len: 1,
            },
            DiffOp::Insert {
                old_index: 2,
                new_index: 3,
                new_len: 1,
            },
        ];

        let plan = plan_structural_patches_top_down(viewport, &current_frame, 10, &diff);

        let PatchPlan::Planned {
            operations,
            viewport,
        } = plan
        else {
            panic!("expected incremental plan");
        };
        assert_eq!(
            operations,
            vec![
                PlannedOperation::MoveUp(2),
                PlannedOperation::CarriageReturn,
                PlannedOperation::ClearLine,
                PlannedOperation::Write("new"),
                PlannedOperation::CarriageReturn,
                PlannedOperation::MoveDown(2),
                PlannedOperation::Write("tail one"),
                PlannedOperation::Newline,
                PlannedOperation::Write("tail two"),
                PlannedOperation::Newline,
            ]
        );
        assert_eq!(
            viewport,
            ViewportState {
                first_visible_managed_row: 0,
                cursor_managed_row: 4,
            }
        );
    }
}
