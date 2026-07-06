use crate::{Backend, TerminalError};

use super::{
    diff::{DocumentPatch, patience_diff, translate_diff_to_patches},
    frame::{CommittedFrame, ViewportState, frame_changed, is_append_only},
};

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

#[derive(Clone, Copy)]
enum PlannedOperationKind {
    VerticalMove,
    CursorControl,
    LineEdit,
    Write,
}

impl PlannedOperation<'_> {
    fn execute<B: Backend>(&self, backend: &mut B) -> Result<(), TerminalError<B::Error>> {
        match self.kind() {
            PlannedOperationKind::VerticalMove => self.execute_vertical_move(backend),
            PlannedOperationKind::CursorControl => self.execute_cursor_control(backend),
            PlannedOperationKind::LineEdit => self.execute_line_edit(backend),
            PlannedOperationKind::Write => self.execute_write(backend),
        }
    }

    fn kind(&self) -> PlannedOperationKind {
        match self {
            PlannedOperation::MoveUp(_) | PlannedOperation::MoveDown(_) => {
                PlannedOperationKind::VerticalMove
            }
            PlannedOperation::CarriageReturn | PlannedOperation::Newline => {
                PlannedOperationKind::CursorControl
            }
            PlannedOperation::ClearLine
            | PlannedOperation::InsertLines(_)
            | PlannedOperation::DeleteLines(_)
            | PlannedOperation::ScrollUp(_) => PlannedOperationKind::LineEdit,
            PlannedOperation::Write(_) => PlannedOperationKind::Write,
        }
    }

    fn execute_vertical_move<B: Backend>(
        &self,
        backend: &mut B,
    ) -> Result<(), TerminalError<B::Error>> {
        match self {
            PlannedOperation::MoveUp(count) => backend.move_up(*count)?,
            PlannedOperation::MoveDown(count) => backend.move_down(*count)?,
            _ => unreachable!("vertical-move operation expected"),
        }

        Ok(())
    }

    fn execute_cursor_control<B: Backend>(
        &self,
        backend: &mut B,
    ) -> Result<(), TerminalError<B::Error>> {
        match self {
            PlannedOperation::CarriageReturn => backend.carriage_return()?,
            PlannedOperation::Newline => backend.newline()?,
            _ => unreachable!("cursor-control operation expected"),
        }

        Ok(())
    }

    fn execute_line_edit<B: Backend>(
        &self,
        backend: &mut B,
    ) -> Result<(), TerminalError<B::Error>> {
        match self {
            PlannedOperation::ClearLine => backend.clear_line()?,
            PlannedOperation::InsertLines(count) => backend.insert_lines(*count)?,
            PlannedOperation::DeleteLines(count) => backend.delete_lines(*count)?,
            PlannedOperation::ScrollUp(count) => backend.scroll_up(*count)?,
            _ => unreachable!("line-edit operation expected"),
        }

        Ok(())
    }

    fn execute_write<B: Backend>(&self, backend: &mut B) -> Result<(), TerminalError<B::Error>> {
        let PlannedOperation::Write(line) = self else {
            unreachable!("write operation expected");
        };

        backend.write_str(line)?;
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

    let patches = translate_diff_to_patches(&patience_diff(&last_frame.lines, current_frame));
    match plan_changed_lines_before_trailing_append(
        last_frame.viewport,
        current_frame,
        height,
        &patches,
    ) {
        MixedAppendPlan::Planned {
            operations,
            viewport,
        } => {
            return PlannedFrameRender::changed_lines(operations, viewport);
        }
        MixedAppendPlan::NeedsFullRedraw => {
            return PlannedFrameRender::full_redraw(current_frame.len(), height);
        }
        MixedAppendPlan::Unsupported => {}
    }

    match plan_structural_patches_top_down(last_frame.viewport, current_frame, height, &patches) {
        MixedAppendPlan::Planned {
            operations,
            viewport,
        } => {
            return PlannedFrameRender::changed_lines(operations, viewport);
        }
        MixedAppendPlan::NeedsFullRedraw => {
            return PlannedFrameRender::full_redraw(current_frame.len(), height);
        }
        MixedAppendPlan::Unsupported => {}
    }

    if let Some((operations, viewport)) =
        plan_patch_operations(last_frame.viewport, current_frame, height, &patches)
    {
        return PlannedFrameRender::changed_lines(operations, viewport);
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

enum MixedAppendPlan<'a> {
    Planned {
        operations: Vec<PlannedOperation<'a>>,
        viewport: ViewportState,
    },
    NeedsFullRedraw,
    Unsupported,
}

fn plan_changed_lines_before_trailing_append<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> MixedAppendPlan<'a> {
    let Some((DocumentPatch::InsertLines { old_row, current }, changed_patches)) =
        patches.split_last()
    else {
        return MixedAppendPlan::Unsupported;
    };

    if *old_row != viewport.cursor_managed_row
        || current.end != current_frame.len()
        || changed_patches.is_empty()
        || !changed_patches
            .iter()
            .all(|patch| matches!(patch, DocumentPatch::ChangedLine { .. }))
    {
        return MixedAppendPlan::Unsupported;
    }

    let mut operations = Vec::new();
    let mut actual_cursor_row = viewport.cursor_managed_row;
    for patch in changed_patches {
        let DocumentPatch::ChangedLine {
            old_row,
            current_row,
        } = patch
        else {
            unreachable!("changed patches already filtered");
        };

        if !row_is_visible(viewport, *old_row, height) {
            return MixedAppendPlan::NeedsFullRedraw;
        }

        move_cursor_to_managed_row(&mut operations, &mut actual_cursor_row, *old_row);
        operations.push(PlannedOperation::CarriageReturn);
        operations.push(PlannedOperation::ClearLine);
        operations.push(PlannedOperation::Write(
            current_frame[*current_row].as_str(),
        ));
        operations.push(PlannedOperation::CarriageReturn);
    }

    move_cursor_to_managed_row(
        &mut operations,
        &mut actual_cursor_row,
        viewport.cursor_managed_row,
    );
    operations.extend(plan_append_operations(&current_frame[current.clone()]));

    MixedAppendPlan::Planned {
        operations,
        viewport: viewport.after_newlines(current.len(), height),
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

fn old_row_after_delta(old_row: usize, row_delta: isize) -> Option<usize> {
    (old_row as isize).checked_add(row_delta)?.try_into().ok()
}

fn plan_structural_patches_top_down<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> MixedAppendPlan<'a> {
    if !patches.iter().any(|patch| {
        matches!(
            patch,
            DocumentPatch::DeleteLines { .. } | DocumentPatch::InsertLines { .. }
        )
    }) {
        return MixedAppendPlan::Unsupported;
    }

    let mut operations = Vec::new();
    let mut simulated_viewport = viewport;
    let mut actual_cursor_row = viewport.cursor_managed_row;
    let mut row_delta = 0;

    for (index, patch) in patches.iter().enumerate() {
        match patch {
            DocumentPatch::ChangedLine {
                old_row,
                current_row,
            } => {
                let Some(target_row) = old_row_after_delta(*old_row, row_delta) else {
                    return MixedAppendPlan::NeedsFullRedraw;
                };
                if !row_is_visible(simulated_viewport, target_row, height) {
                    return MixedAppendPlan::NeedsFullRedraw;
                }

                move_cursor_to_managed_row(&mut operations, &mut actual_cursor_row, target_row);
                operations.push(PlannedOperation::CarriageReturn);
                operations.push(PlannedOperation::ClearLine);
                operations.push(PlannedOperation::Write(
                    current_frame[*current_row].as_str(),
                ));
                operations.push(PlannedOperation::CarriageReturn);
            }
            DocumentPatch::DeleteLines { old } => {
                let Some(target_row) = old_row_after_delta(old.start, row_delta) else {
                    return MixedAppendPlan::NeedsFullRedraw;
                };
                if !row_is_visible(simulated_viewport, target_row, height) {
                    return MixedAppendPlan::NeedsFullRedraw;
                }

                let Some(final_sentinel_row) =
                    simulated_viewport.cursor_managed_row.checked_sub(old.len())
                else {
                    return MixedAppendPlan::NeedsFullRedraw;
                };
                if target_row > final_sentinel_row {
                    return MixedAppendPlan::NeedsFullRedraw;
                }

                move_cursor_to_managed_row(&mut operations, &mut actual_cursor_row, target_row);
                operations.push(PlannedOperation::CarriageReturn);
                operations.push(PlannedOperation::DeleteLines(old.len()));
                simulated_viewport = simulated_viewport.with_cursor_managed_row(final_sentinel_row);
                if !simulated_viewport.cursor_is_visible(height) {
                    return MixedAppendPlan::NeedsFullRedraw;
                }
                row_delta -= old.len() as isize;
            }
            DocumentPatch::InsertLines { old_row, current } => {
                let Some(target_row) = old_row_after_delta(*old_row, row_delta) else {
                    return MixedAppendPlan::NeedsFullRedraw;
                };

                if target_row == simulated_viewport.cursor_managed_row
                    && index == patches.len() - 1
                    && current.end == current_frame.len()
                {
                    move_cursor_to_managed_row(
                        &mut operations,
                        &mut actual_cursor_row,
                        simulated_viewport.cursor_managed_row,
                    );
                    operations.extend(plan_append_operations(&current_frame[current.clone()]));
                    simulated_viewport = simulated_viewport.after_newlines(current.len(), height);
                    actual_cursor_row = simulated_viewport.cursor_managed_row;
                    row_delta += current.len() as isize;
                    continue;
                }

                if target_row >= simulated_viewport.cursor_managed_row
                    || !row_is_visible(simulated_viewport, target_row, height)
                {
                    return MixedAppendPlan::NeedsFullRedraw;
                }

                let mut next_target_row = target_row;
                let mut next_current_row = current.start;
                while next_current_row < current.end {
                    if !row_is_viewport_visible(simulated_viewport, actual_cursor_row, height)
                        || !row_is_viewport_visible(simulated_viewport, next_target_row, height)
                    {
                        return MixedAppendPlan::NeedsFullRedraw;
                    }

                    let bottom_visible_row = simulated_viewport.first_visible_managed_row
                        + height.saturating_sub(1) as isize;
                    let safe_capacity =
                        bottom_visible_row - simulated_viewport.cursor_managed_row as isize + 1;
                    if safe_capacity <= 0 {
                        return MixedAppendPlan::NeedsFullRedraw;
                    }

                    let remaining = current.end - next_current_row;
                    let chunk_len = remaining.min(safe_capacity as usize);
                    let Some(final_sentinel_row) =
                        simulated_viewport.cursor_managed_row.checked_add(chunk_len)
                    else {
                        return MixedAppendPlan::NeedsFullRedraw;
                    };

                    move_cursor_to_managed_row(
                        &mut operations,
                        &mut actual_cursor_row,
                        next_target_row,
                    );
                    operations.push(PlannedOperation::CarriageReturn);
                    operations.push(PlannedOperation::InsertLines(chunk_len));
                    for offset in 0..chunk_len {
                        if offset > 0 {
                            operations.push(PlannedOperation::Newline);
                        }
                        let current_row = next_current_row + offset;
                        operations.push(PlannedOperation::ClearLine);
                        operations
                            .push(PlannedOperation::Write(current_frame[current_row].as_str()));
                    }
                    operations.push(PlannedOperation::CarriageReturn);

                    simulated_viewport =
                        simulated_viewport.with_cursor_managed_row(final_sentinel_row);
                    row_delta += chunk_len as isize;
                    actual_cursor_row = next_target_row + chunk_len.saturating_sub(1);
                    next_target_row += chunk_len;
                    next_current_row += chunk_len;

                    let scroll_up_count =
                        scroll_up_count_to_reveal_cursor(simulated_viewport, height);
                    if scroll_up_count > 0 {
                        operations.push(PlannedOperation::ScrollUp(scroll_up_count));
                        simulated_viewport.first_visible_managed_row += scroll_up_count as isize;
                        let Some(scrolled_cursor_row) =
                            actual_cursor_row.checked_add(scroll_up_count)
                        else {
                            return MixedAppendPlan::NeedsFullRedraw;
                        };
                        actual_cursor_row = scrolled_cursor_row;
                    }
                }
            }
        }
    }

    move_cursor_to_managed_row(
        &mut operations,
        &mut actual_cursor_row,
        simulated_viewport.cursor_managed_row,
    );

    MixedAppendPlan::Planned {
        operations,
        viewport: simulated_viewport,
    }
}

fn plan_patch_operations<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> Option<(Vec<PlannedOperation<'a>>, ViewportState)> {
    let mut operations = Vec::new();
    let mut simulated_viewport = viewport;

    for patch in patches.iter().rev() {
        match patch {
            DocumentPatch::ChangedLine {
                old_row,
                current_row,
            } => plan_changed_line_patch(
                &mut operations,
                simulated_viewport,
                height,
                *old_row,
                current_frame[*current_row].as_str(),
            )?,
            DocumentPatch::InsertLines { old_row, current } => {
                simulated_viewport = plan_insert_patch(
                    &mut operations,
                    simulated_viewport,
                    current_frame,
                    height,
                    *old_row,
                    current.clone(),
                )?;
            }
            DocumentPatch::DeleteLines { old } => {
                simulated_viewport =
                    plan_delete_patch(&mut operations, simulated_viewport, height, old.clone())?;
            }
        }
    }

    Some((operations, simulated_viewport))
}

fn plan_changed_line_patch<'a>(
    operations: &mut Vec<PlannedOperation<'a>>,
    viewport: ViewportState,
    height: usize,
    old_row: usize,
    current_line: &'a str,
) -> Option<()> {
    if !row_is_visible(viewport, old_row, height) {
        return None;
    }

    let distance_from_cursor = viewport.cursor_managed_row.checked_sub(old_row)?;
    operations.push(PlannedOperation::MoveUp(distance_from_cursor));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::ClearLine);
    operations.push(PlannedOperation::Write(current_line));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::MoveDown(distance_from_cursor));
    Some(())
}

fn plan_insert_patch<'a>(
    operations: &mut Vec<PlannedOperation<'a>>,
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    old_row: usize,
    current: std::ops::Range<usize>,
) -> Option<ViewportState> {
    if !row_is_visible(viewport, old_row, height) {
        return None;
    }

    if old_row == viewport.cursor_managed_row {
        operations.extend(plan_append_operations(&current_frame[current.clone()]));
        return Some(viewport.after_newlines(current.len(), height));
    }

    let mut resulting_viewport =
        viewport.with_cursor_managed_row(viewport.cursor_managed_row.checked_add(current.len())?);
    let scroll_up_count = scroll_up_count_to_reveal_cursor(resulting_viewport, height);
    resulting_viewport.first_visible_managed_row += scroll_up_count as isize;

    let distance_from_cursor = viewport.cursor_managed_row.checked_sub(old_row)?;
    operations.push(PlannedOperation::MoveUp(distance_from_cursor));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::InsertLines(current.len()));
    for (offset, current_row) in current.enumerate() {
        if offset > 0 {
            operations.push(PlannedOperation::Newline);
        }
        operations.push(PlannedOperation::ClearLine);
        operations.push(PlannedOperation::Write(current_frame[current_row].as_str()));
    }
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::MoveDown(distance_from_cursor + 1));
    if scroll_up_count > 0 {
        operations.push(PlannedOperation::ScrollUp(scroll_up_count));
    }
    Some(resulting_viewport)
}

fn scroll_up_count_to_reveal_cursor(viewport: ViewportState, height: usize) -> usize {
    viewport
        .visible_cursor_row()
        .saturating_sub(height.saturating_sub(1))
}

fn plan_delete_patch<'a>(
    operations: &mut Vec<PlannedOperation<'a>>,
    viewport: ViewportState,
    height: usize,
    old: std::ops::Range<usize>,
) -> Option<ViewportState> {
    if !row_is_visible(viewport, old.start, height) {
        return None;
    }

    let resulting_viewport =
        viewport.with_cursor_managed_row(viewport.cursor_managed_row.checked_sub(old.len())?);
    if !resulting_viewport.cursor_is_visible(height) {
        return None;
    }

    operations.push(PlannedOperation::MoveUp(
        viewport.cursor_managed_row.checked_sub(old.start)?,
    ));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::DeleteLines(old.len()));
    operations.push(PlannedOperation::MoveDown(
        resulting_viewport
            .cursor_managed_row
            .checked_sub(old.start)?,
    ));
    Some(resulting_viewport)
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
