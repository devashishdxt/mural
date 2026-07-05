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
            | PlannedOperation::DeleteLines(_) => PlannedOperationKind::LineEdit,
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
    if let Some((append_range, remaining_patches)) = extract_trailing_append(
        patches.as_slice(),
        last_frame.lines.len(),
        current_frame.len(),
    ) {
        let simulated_viewport = last_frame
            .viewport
            .after_newlines(append_range.len(), height);
        if !patches_are_visible_changed_lines(remaining_patches, simulated_viewport, height) {
            return PlannedFrameRender::full_redraw(current_frame.len(), height);
        }

        let mut operations = plan_append_operations(&current_frame[append_range]);
        operations.extend(plan_changed_line_operations(
            simulated_viewport,
            changed_line_patches(remaining_patches),
            current_frame,
        ));
        return PlannedFrameRender::changed_lines(operations, simulated_viewport);
    }

    if let Some((operations, viewport)) = plan_insert_line_operations(
        last_frame.viewport,
        current_frame,
        height,
        patches.as_slice(),
    ) {
        return PlannedFrameRender::changed_lines(operations, viewport);
    }

    if let Some((operations, viewport)) = plan_delete_line_operations(
        last_frame.viewport,
        current_frame,
        height,
        patches.as_slice(),
    ) {
        return PlannedFrameRender::changed_lines(operations, viewport);
    }

    if !patches_are_visible_changed_lines(&patches, last_frame.viewport, height) {
        return PlannedFrameRender::full_redraw(current_frame.len(), height);
    }

    PlannedFrameRender::changed_lines(
        plan_changed_line_operations(
            last_frame.viewport,
            changed_line_patches(&patches),
            current_frame,
        ),
        last_frame.viewport,
    )
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
    let managed_row = row as isize;
    let visible_row = managed_row - viewport.first_visible_managed_row;

    managed_row >= viewport.first_visible_managed_row
        && row <= viewport.cursor_managed_row
        && visible_row < height as isize
}

fn patches_are_visible_changed_lines(
    patches: &[DocumentPatch],
    viewport: ViewportState,
    height: usize,
) -> bool {
    patches.iter().all(|patch| match patch {
        DocumentPatch::ChangedLine { old_row, .. } => row_is_visible(viewport, *old_row, height),
        DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => false,
    })
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

fn extract_trailing_append(
    patches: &[DocumentPatch],
    old_len: usize,
    current_len: usize,
) -> Option<(std::ops::Range<usize>, &[DocumentPatch])> {
    let (last_patch, remaining_patches) = patches.split_last()?;
    let DocumentPatch::InsertLines { old_row, current } = last_patch else {
        return None;
    };
    if *old_row != old_len || current.end != current_len {
        return None;
    }

    Some((current.clone(), remaining_patches))
}

fn plan_insert_line_operations<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> Option<(Vec<PlannedOperation<'a>>, ViewportState)> {
    let [DocumentPatch::InsertLines { old_row, current }] = patches else {
        return None;
    };
    let resulting_viewport = viewport.with_cursor_managed_row(current_frame.len());
    if !row_is_visible(viewport, *old_row, height) || !resulting_viewport.cursor_is_visible(height)
    {
        return None;
    }

    let inserted_count = current.len();
    let mut operations = Vec::new();
    operations.push(PlannedOperation::MoveUp(
        viewport.cursor_managed_row.checked_sub(*old_row)?,
    ));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::InsertLines(inserted_count));
    for (offset, current_row) in current.clone().enumerate() {
        if offset > 0 {
            operations.push(PlannedOperation::Newline);
        }
        operations.push(PlannedOperation::ClearLine);
        operations.push(PlannedOperation::Write(current_frame[current_row].as_str()));
    }
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::MoveDown(
        viewport.cursor_managed_row.checked_sub(*old_row)? + 1,
    ));
    Some((operations, resulting_viewport))
}

fn plan_delete_line_operations<'a>(
    viewport: ViewportState,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> Option<(Vec<PlannedOperation<'a>>, ViewportState)> {
    let [DocumentPatch::DeleteLines { old }] = patches else {
        return None;
    };
    let resulting_viewport = viewport.with_cursor_managed_row(current_frame.len());
    if !row_is_visible(viewport, old.start, height) || !resulting_viewport.cursor_is_visible(height)
    {
        return None;
    }

    let distance_from_cursor = viewport.cursor_managed_row.checked_sub(old.start)?;
    let distance_to_current_cursor = current_frame.len().checked_sub(old.start)?;

    let operations = vec![
        PlannedOperation::MoveUp(distance_from_cursor),
        PlannedOperation::CarriageReturn,
        PlannedOperation::DeleteLines(old.len()),
        PlannedOperation::MoveDown(distance_to_current_cursor),
    ];
    Some((operations, resulting_viewport))
}

fn changed_line_patches(patches: &[DocumentPatch]) -> Vec<(usize, usize)> {
    patches
        .iter()
        .filter_map(|patch| match patch {
            DocumentPatch::ChangedLine {
                old_row,
                current_row,
            } => Some((*old_row, *current_row)),
            DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => None,
        })
        .collect()
}

fn plan_changed_line_operations<'a>(
    viewport: ViewportState,
    patches: Vec<(usize, usize)>,
    current_frame: &'a [String],
) -> Vec<PlannedOperation<'a>> {
    let mut operations = Vec::new();
    for (old_row, current_row) in patches {
        let distance_from_cursor = viewport.cursor_managed_row - old_row;
        operations.push(PlannedOperation::MoveUp(distance_from_cursor));
        operations.push(PlannedOperation::CarriageReturn);
        operations.push(PlannedOperation::ClearLine);
        operations.push(PlannedOperation::Write(current_frame[current_row].as_str()));
        operations.push(PlannedOperation::CarriageReturn);
        operations.push(PlannedOperation::MoveDown(distance_from_cursor));
    }
    operations
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
