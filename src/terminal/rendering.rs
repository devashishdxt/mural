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

struct PlannedFrameRender<'a> {
    frame_plan: FramePlan<'a>,
    viewport: ViewportState,
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
        return PlannedFrameRender {
            frame_plan: FramePlan::FullRedraw,
            viewport: ViewportState::after_full_redraw(current_frame.len(), height),
        };
    }

    if is_append_only(&last_frame.lines, current_frame) {
        let appended = &current_frame[last_frame.lines.len()..];
        return PlannedFrameRender {
            frame_plan: FramePlan::Appended(appended),
            viewport: last_frame.viewport.after_newlines(appended.len(), height),
        };
    }

    if !frame_changed(&last_frame.lines, current_frame) {
        return PlannedFrameRender {
            frame_plan: FramePlan::NoChanges,
            viewport: last_frame.viewport,
        };
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
        if remaining_patches.iter().any(|patch| match patch {
            DocumentPatch::ChangedLine { old_row, .. } => {
                !row_is_visible(simulated_viewport, *old_row, height)
            }
            DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => true,
        }) {
            return PlannedFrameRender {
                frame_plan: FramePlan::FullRedraw,
                viewport: ViewportState::after_full_redraw(current_frame.len(), height),
            };
        }

        let mut operations = plan_append_operations(&current_frame[append_range]);
        operations.extend(plan_changed_line_operations(
            simulated_viewport,
            remaining_patches,
            current_frame,
        ));
        return PlannedFrameRender {
            frame_plan: FramePlan::ChangedLines(operations),
            viewport: simulated_viewport,
        };
    }

    if let Some((operations, viewport)) = plan_insert_line_operations(
        last_frame.viewport,
        current_frame,
        height,
        patches.as_slice(),
    ) {
        return PlannedFrameRender {
            frame_plan: FramePlan::ChangedLines(operations),
            viewport,
        };
    }

    if let Some((operations, viewport)) = plan_delete_line_operations(
        last_frame.viewport,
        current_frame,
        height,
        patches.as_slice(),
    ) {
        return PlannedFrameRender {
            frame_plan: FramePlan::ChangedLines(operations),
            viewport,
        };
    }

    if patches
        .iter()
        .any(|patch| !matches!(patch, DocumentPatch::ChangedLine { .. }))
    {
        return PlannedFrameRender {
            frame_plan: FramePlan::FullRedraw,
            viewport: ViewportState::after_full_redraw(current_frame.len(), height),
        };
    }

    if patches.iter().any(|patch| match patch {
        DocumentPatch::ChangedLine { old_row, .. } => {
            !row_is_visible(last_frame.viewport, *old_row, height)
        }
        DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => false,
    }) {
        return PlannedFrameRender {
            frame_plan: FramePlan::FullRedraw,
            viewport: ViewportState::after_full_redraw(current_frame.len(), height),
        };
    }

    PlannedFrameRender {
        frame_plan: FramePlan::ChangedLines(plan_changed_line_operations(
            last_frame.viewport,
            patches,
            current_frame,
        )),
        viewport: last_frame.viewport,
    }
}

fn render_planned_operations<B: Backend>(
    backend: &mut B,
    operations: &[PlannedOperation<'_>],
) -> Result<(), TerminalError<B::Error>> {
    for operation in operations {
        match operation {
            PlannedOperation::MoveUp(count) => backend.move_up(*count)?,
            PlannedOperation::MoveDown(count) => backend.move_down(*count)?,
            PlannedOperation::CarriageReturn => backend.carriage_return()?,
            PlannedOperation::ClearLine => backend.clear_line()?,
            PlannedOperation::InsertLines(count) => backend.insert_lines(*count)?,
            PlannedOperation::DeleteLines(count) => backend.delete_lines(*count)?,
            PlannedOperation::Write(line) => backend.write_str(line)?,
            PlannedOperation::Newline => backend.newline()?,
        }
    }

    Ok(())
}

fn row_is_visible(viewport: ViewportState, row: usize, height: usize) -> bool {
    row as isize >= viewport.first_visible_managed_row
        && row <= viewport.cursor_managed_row
        && (row as isize - viewport.first_visible_managed_row) < height as isize
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
) -> Option<(std::ops::Range<usize>, Vec<DocumentPatch>)> {
    let (last_patch, remaining_patches) = patches.split_last()?;
    let DocumentPatch::InsertLines { old_row, current } = last_patch else {
        return None;
    };
    if *old_row != old_len || current.end != current_len {
        return None;
    }

    Some((current.clone(), remaining_patches.to_vec()))
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

    let mut operations = Vec::new();
    operations.push(PlannedOperation::MoveUp(distance_from_cursor));
    operations.push(PlannedOperation::CarriageReturn);
    operations.push(PlannedOperation::DeleteLines(old.len()));
    operations.push(PlannedOperation::MoveDown(distance_to_current_cursor));
    Some((operations, resulting_viewport))
}

fn plan_changed_line_operations<'a>(
    viewport: ViewportState,
    patches: Vec<DocumentPatch>,
    current_frame: &'a [String],
) -> Vec<PlannedOperation<'a>> {
    let mut operations = Vec::new();
    for patch in patches {
        let DocumentPatch::ChangedLine {
            old_row,
            current_row,
        } = patch
        else {
            continue;
        };
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
