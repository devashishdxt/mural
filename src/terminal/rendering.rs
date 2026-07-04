use crate::{Backend, TerminalError};

use super::{
    diff::{DocumentPatch, patience_diff, translate_diff_to_patches},
    frame::{CommittedFrame, frame_changed, is_append_only},
};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PlannedOperation<'a> {
    MoveUp(usize),
    MoveDown(usize),
    CarriageReturn,
    ClearLine,
    InsertLines(usize),
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

pub(super) fn render_frame_transaction<B: Backend>(
    backend: &mut B,
    last_frame: &CommittedFrame,
    current_frame: &[String],
    height: usize,
    needs_full_redraw: bool,
) -> Result<(), TerminalError<B::Error>> {
    match plan_frame_render(last_frame, current_frame, height, needs_full_redraw) {
        FramePlan::NoChanges => {}
        FramePlan::Appended(lines) => render_appended_lines(backend, lines)?,
        FramePlan::ChangedLines(operations) => render_planned_operations(backend, &operations)?,
        FramePlan::FullRedraw => render_full_frame(backend, current_frame)?,
    }

    backend.flush()?;
    Ok(())
}

pub(super) fn plan_frame_render<'a>(
    last_frame: &CommittedFrame,
    current_frame: &'a [String],
    height: usize,
    needs_full_redraw: bool,
) -> FramePlan<'a> {
    if needs_full_redraw {
        return FramePlan::FullRedraw;
    }

    if is_append_only(&last_frame.lines, current_frame) {
        return FramePlan::Appended(&current_frame[last_frame.lines.len()..]);
    }

    if !frame_changed(&last_frame.lines, current_frame) {
        return FramePlan::NoChanges;
    }

    let patches = translate_diff_to_patches(&patience_diff(&last_frame.lines, current_frame));
    if let Some((append_range, remaining_patches)) = extract_trailing_append(
        patches.as_slice(),
        last_frame.lines.len(),
        current_frame.len(),
    ) {
        let simulated_sentinel_row = last_frame.sentinel_row + append_range.len();
        let first_visible_row = first_visible_row(simulated_sentinel_row, height);
        if remaining_patches.iter().any(|patch| match patch {
            DocumentPatch::ChangedLine { old_row, .. } => *old_row < first_visible_row,
            DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => true,
        }) {
            return FramePlan::FullRedraw;
        }

        let mut operations = plan_append_operations(&current_frame[append_range]);
        operations.extend(plan_changed_line_operations(
            simulated_sentinel_row,
            remaining_patches,
            current_frame,
        ));
        return FramePlan::ChangedLines(operations);
    }

    if let Some(operations) = plan_insert_line_operations(
        last_frame.sentinel_row,
        current_frame,
        height,
        patches.as_slice(),
    ) {
        return FramePlan::ChangedLines(operations);
    }

    if patches
        .iter()
        .any(|patch| !matches!(patch, DocumentPatch::ChangedLine { .. }))
    {
        return FramePlan::FullRedraw;
    }

    let first_visible_row = first_visible_row(last_frame.sentinel_row, height);
    if patches.iter().any(|patch| match patch {
        DocumentPatch::ChangedLine { old_row, .. } => *old_row < first_visible_row,
        DocumentPatch::InsertLines { .. } | DocumentPatch::DeleteLines { .. } => false,
    }) {
        return FramePlan::FullRedraw;
    }

    FramePlan::ChangedLines(plan_changed_line_operations(
        last_frame.sentinel_row,
        patches,
        current_frame,
    ))
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
            PlannedOperation::Write(line) => backend.write_str(line)?,
            PlannedOperation::Newline => backend.newline()?,
        }
    }

    Ok(())
}

fn first_visible_row(sentinel_row: usize, height: usize) -> usize {
    sentinel_row.saturating_sub(height.saturating_sub(1))
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
    sentinel_row: usize,
    current_frame: &'a [String],
    height: usize,
    patches: &[DocumentPatch],
) -> Option<Vec<PlannedOperation<'a>>> {
    let [DocumentPatch::InsertLines { old_row, current }] = patches else {
        return None;
    };
    if current_frame.len() >= height || *old_row < first_visible_row(sentinel_row, height) {
        return None;
    }

    let inserted_count = current.len();
    let mut operations = Vec::new();
    operations.push(PlannedOperation::MoveUp(sentinel_row - old_row));
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
    operations.push(PlannedOperation::MoveDown(sentinel_row - old_row + 1));
    Some(operations)
}

fn plan_changed_line_operations<'a>(
    sentinel_row: usize,
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
        let distance_from_sentinel = sentinel_row - old_row;
        operations.push(PlannedOperation::MoveUp(distance_from_sentinel));
        operations.push(PlannedOperation::CarriageReturn);
        operations.push(PlannedOperation::ClearLine);
        operations.push(PlannedOperation::Write(current_frame[current_row].as_str()));
        operations.push(PlannedOperation::CarriageReturn);
        operations.push(PlannedOperation::MoveDown(distance_from_sentinel));
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
