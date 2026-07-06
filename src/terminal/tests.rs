use std::borrow::Cow;

use super::{
    frame::{CommittedFrame, ViewportState},
    rendering::{FramePlan, PlannedOperation, plan_frame_render},
    *,
};
use crate::{Block, test_utils::*};

fn committed_frame(
    lines: Vec<String>,
    first_visible_managed_row: isize,
    cursor_managed_row: usize,
) -> CommittedFrame {
    CommittedFrame {
        lines,
        viewport: ViewportState {
            first_visible_managed_row,
            cursor_managed_row,
        },
    }
}

#[test]
fn terminal_rejects_zero_sized_dimensions() {
    let err = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 0,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .err()
    .expect("invalid size should fail");

    assert!(matches!(err, TerminalError::InvalidTerminalSize));
}

#[test]
fn terminal_rejects_cursor_positions_outside_size() {
    let err = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 24, column: 0 },
    )
    .err()
    .expect("invalid cursor position should fail");

    assert!(matches!(err, TerminalError::InvalidCursorPosition));
}

#[test]
fn construction_hides_cursor_and_flushes() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();

    let _terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 3, column: 0 },
    )
    .unwrap();

    assert_eq!(
        operations.operations(),
        vec![Operation::HideCursor, Operation::Flush]
    );
}

#[test]
fn construction_normalizes_nonzero_initial_column_with_newline() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();

    let _terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 3, column: 5 },
    )
    .unwrap();

    assert_eq!(
        operations.operations(),
        vec![Operation::HideCursor, Operation::Newline, Operation::Flush]
    );
}

#[test]
fn bottom_row_nonzero_column_normalizes_after_scroll() {
    let mut backend = RecordingBackend::default();

    let cursor = normalize_initial_position(
        &mut backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 23, column: 5 },
    )
    .unwrap();

    assert_eq!(backend.operations(), vec![Operation::Newline]);
    assert_eq!(cursor, CursorPosition { row: 23, column: 0 });
}

#[test]
fn drop_restores_cursor_and_flushes_best_effort() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();

    {
        let _terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 3, column: 0 },
        )
        .unwrap();
    }

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn render_writes_live_blocks_before_pinned_blocks() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("live");
    terminal.push_pinned("pinned");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("live".to_owned()),
            Operation::Newline,
            Operation::Write("pinned".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn built_in_string_blocks_wrap_at_terminal_width_minus_one() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 6,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("hello world");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("hello".to_owned()),
            Operation::Newline,
            Operation::Write("world".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn changed_line_render_patches_row_and_restores_cursor_to_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("old");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("status", block.clone());
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("status")
        .expect("status block should exist")
        .set_text("new");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("old".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("new".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
}

#[test]
fn viewport_tracks_initial_unmanaged_rows_and_append_scrolling() {
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 4,
        },
        CursorPosition { row: 2, column: 0 },
    )
    .unwrap();

    terminal.push_live("bottom");
    terminal.render().unwrap();

    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: -2,
            cursor_managed_row: 1,
        }
    );
    assert_eq!(
        terminal.last_committed_frame.viewport.visible_cursor_row(),
        3
    );

    terminal.push_live("scrolls");
    terminal.render().unwrap();

    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: -1,
            cursor_managed_row: 2,
        }
    );
    assert_eq!(
        terminal.last_committed_frame.viewport.visible_cursor_row(),
        3
    );
}

#[test]
fn full_redraw_resets_viewport_for_short_exact_footprint_and_long_content() {
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 4,
        },
        CursorPosition { row: 2, column: 0 },
    )
    .unwrap();

    terminal.push_live("short");
    terminal.force_full_redraw();
    terminal.render().unwrap();
    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: 0,
            cursor_managed_row: 1,
        }
    );

    terminal.clear_live();
    terminal.push_live("one");
    terminal.push_live("two");
    terminal.push_live("three");
    terminal.force_full_redraw();
    terminal.render().unwrap();
    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: 0,
            cursor_managed_row: 3,
        }
    );

    terminal.push_live("four");
    terminal.push_live("five");
    terminal.force_full_redraw();
    terminal.render().unwrap();
    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: 2,
            cursor_managed_row: 5,
        }
    );
    assert_eq!(
        terminal.last_committed_frame.viewport.visible_cursor_row(),
        3
    );
}

#[test]
fn middle_insert_scrolls_up_immediately_when_chunk_pushes_sentinel_below_viewport() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 4,
        },
        CursorPosition { row: 2, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::InsertLines(1),
            Operation::ClearLine,
            Operation::Write("top".to_owned()),
            Operation::CarriageReturn,
            Operation::ScrollUp(1),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: -1,
            cursor_managed_row: 2,
        }
    );
}

#[test]
fn visible_middle_insert_uses_insert_lines_without_redrawing_shifted_rows() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 6,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "inserted", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("top".to_owned()),
            Operation::Newline,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::InsertLines(1),
            Operation::ClearLine,
            Operation::Write("inserted".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Flush,
        ]
    );
}

#[test]
fn top_boundary_insert_preserves_shifted_rows_and_restores_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 4,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::InsertLines(1),
            Operation::ClearLine,
            Operation::Write("top".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Flush,
        ]
    );
}

#[test]
fn multi_line_insert_clears_each_inserted_row_and_restores_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["head", "tail"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 6,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["head", "one", "two", "tail"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[7..],
        [
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::InsertLines(2),
            Operation::ClearLine,
            Operation::Write("one".to_owned()),
            Operation::Newline,
            Operation::ClearLine,
            Operation::Write("two".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Flush,
        ]
    );
}

#[test]
fn multi_line_middle_insert_splits_into_safe_chunks_and_preserves_insert_order() {
    let last_frame = committed_frame(vec!["top".to_owned(), "bottom".to_owned()], -1, 2);
    let current_frame = vec![
        "one".to_owned(),
        "two".to_owned(),
        "top".to_owned(),
        "BOTTOM".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 4, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(2),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("one"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ScrollUp(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("two"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ScrollUp(1),
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("BOTTOM"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
        ])
    );
}

#[test]
fn middle_insert_may_discard_old_sentinel_when_scroll_repair_reveals_new_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 3,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "inserted", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[7..],
        [
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::InsertLines(1),
            Operation::ClearLine,
            Operation::Write("inserted".to_owned()),
            Operation::CarriageReturn,
            Operation::ScrollUp(1),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame.viewport,
        ViewportState {
            first_visible_managed_row: 1,
            cursor_managed_row: 3,
        }
    );
}

#[test]
fn visible_middle_delete_uses_delete_lines_without_redrawing_shifted_rows() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "removed", "bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 6,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[9..],
        [
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
}

#[test]
fn tail_delete_clears_exposed_rows_with_delete_lines_and_restores_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["head", "tail one", "tail two"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 6,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["head"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[9..],
        [
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::DeleteLines(2),
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(vec!["head".to_owned()], 0, 1)
    );
}

#[test]
fn multi_line_middle_delete_preserves_shifted_suffix_without_redrawing_it() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "removed one", "removed two", "bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 6,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[11..],
        [
            Operation::MoveUp(3),
            Operation::CarriageReturn,
            Operation::DeleteLines(2),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
}

#[test]
fn separate_deletes_translate_rows_and_track_cursor_continuously() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&[
        "keep zero",
        "remove one",
        "keep two",
        "remove three",
        "keep four",
    ]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 8,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["keep zero", "keep two", "keep four"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[13..],
        [
            Operation::MoveUp(4),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec![
                "keep zero".to_owned(),
                "keep two".to_owned(),
                "keep four".to_owned(),
            ],
            0,
            3,
        )
    );
}

#[test]
fn trailing_append_after_delete_waits_until_cursor_returns_to_final_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "removed", "bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 8,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "bottom", "appended"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[9..],
        [
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::Write("appended".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn changed_line_after_delete_uses_translated_row_and_preserves_view() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["top", "removed", "stable", "old bottom"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 8,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["top", "stable", "new bottom"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[11..],
        [
            Operation::MoveUp(3),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("new bottom".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec![
                "top".to_owned(),
                "stable".to_owned(),
                "new bottom".to_owned()
            ],
            0,
            3,
        )
    );
}

#[test]
fn viewport_visible_patch_target_below_sentinel_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        vec![
            "managed".to_owned(),
            "gap".to_owned(),
            "old unmanaged".to_owned(),
        ],
        0,
        1,
    );
    let current_frame = vec![
        "managed".to_owned(),
        "gap".to_owned(),
        "new unmanaged".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn invisible_cursor_row_before_patch_movement_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        vec![
            "old".to_owned(),
            "stable".to_owned(),
            "tail".to_owned(),
            "sentinel".to_owned(),
        ],
        0,
        4,
    );
    let current_frame = vec![
        "new".to_owned(),
        "stable".to_owned(),
        "tail".to_owned(),
        "sentinel".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn invisible_cursor_row_before_delete_movement_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        vec![
            "remove one".to_owned(),
            "remove two".to_owned(),
            "keep".to_owned(),
            "tail".to_owned(),
        ],
        0,
        4,
    );
    let current_frame = vec!["keep".to_owned(), "tail".to_owned()];

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn delete_target_above_visible_viewport_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        ["zero", "one", "two", "three"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        2,
        4,
    );
    let current_frame = ["zero", "two", "three"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn delete_that_would_remove_the_final_sentinel_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        vec![
            "keep".to_owned(),
            "remove one".to_owned(),
            "remove two".to_owned(),
            "remove three".to_owned(),
        ],
        0,
        2,
    );
    let current_frame = vec!["keep".to_owned()];

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn insert_target_above_visible_viewport_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        ["zero", "one", "two", "three"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        2,
        4,
    );
    let current_frame = ["zero", "inserted", "one", "two", "three"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn pure_append_writes_at_sentinel_without_clearing_and_tracks_scrolled_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 2,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("committed one");
    terminal.push_live("committed two");
    terminal.render().unwrap();
    terminal.push_live("appended three");
    terminal.push_live("appended four");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("committed one".to_owned()),
            Operation::Newline,
            Operation::Write("committed two".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Write("appended three".to_owned()),
            Operation::Newline,
            Operation::Write("appended four".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
    assert_eq!(terminal.last_committed_frame.viewport.cursor_managed_row, 4);
}

#[test]
fn changed_line_plus_trailing_append_patches_old_coordinate_before_appending() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let status = CountingBlock::new("old");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("status", status.clone());
    terminal.push_live("stable");
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("status")
        .expect("status block should exist")
        .set_text("new");
    terminal.push_live("tail");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("old".to_owned()),
            Operation::Newline,
            Operation::Write("stable".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("new".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Write("tail".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec!["new".to_owned(), "stable".to_owned(), "tail".to_owned()],
            0,
            3,
        )
    );
}

#[test]
fn bottom_row_initial_cursor_can_append_and_patch_visible_prior_line_without_full_redraw() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["one", "two", "three"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 10,
        },
        CursorPosition { row: 9, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["one", "TWO", "three", "four"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("one".to_owned()),
            Operation::Newline,
            Operation::Write("two".to_owned()),
            Operation::Newline,
            Operation::Write("three".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("TWO".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Write("four".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn multiple_changed_lines_before_trailing_append_track_cursor_without_zero_moves() {
    let last_frame = committed_frame(
        vec![
            "old one".to_owned(),
            "old two".to_owned(),
            "stable".to_owned(),
        ],
        0,
        3,
    );
    let current_frame = vec![
        "new one".to_owned(),
        "new two".to_owned(),
        "stable".to_owned(),
        "tail".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(3),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new one"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new two"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
            PlannedOperation::Write("tail"),
            PlannedOperation::Newline,
        ])
    );
}

#[test]
fn invisible_cursor_row_before_changed_line_plus_append_falls_back_to_full_redraw() {
    let last_frame = committed_frame(
        vec![
            "old".to_owned(),
            "stable".to_owned(),
            "tail".to_owned(),
            "sentinel".to_owned(),
        ],
        0,
        4,
    );
    let current_frame = vec![
        "new".to_owned(),
        "stable".to_owned(),
        "tail".to_owned(),
        "sentinel".to_owned(),
        "append".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 3, false);

    assert_eq!(plan, FramePlan::FullRedraw);
}

#[test]
fn append_induced_scroll_patches_initially_visible_target_before_appending() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let middle = CountingBlock::new("old middle");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 3,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("top");
    terminal.insert_live("middle", middle.clone());
    terminal.push_live("bottom");
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("middle")
        .expect("middle block should exist")
        .set_text("new middle");
    terminal.push_live("tail one");
    terminal.push_live("tail two");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("top".to_owned()),
            Operation::Newline,
            Operation::Write("old middle".to_owned()),
            Operation::Newline,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("new middle".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(2),
            Operation::Write("tail one".to_owned()),
            Operation::Newline,
            Operation::Write("tail two".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec![
                "top".to_owned(),
                "new middle".to_owned(),
                "bottom".to_owned(),
                "tail one".to_owned(),
                "tail two".to_owned(),
            ],
            3,
            5,
        )
    );
}

#[test]
fn multiple_changed_lines_patch_top_down_without_bottom_up_fallback() {
    let last_frame = committed_frame(
        vec![
            "old one".to_owned(),
            "old two".to_owned(),
            "stable".to_owned(),
        ],
        0,
        3,
    );
    let current_frame = vec![
        "new one".to_owned(),
        "new two".to_owned(),
        "stable".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(3),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new one"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new two"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
        ])
    );
}

#[test]
fn shorter_changed_line_is_cleared_before_replacement_text() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("much longer");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("line", block.clone());
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("line")
        .expect("line block should exist")
        .set_text("short");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations()[5..11],
        [
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("short".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(1),
        ]
    );
}

#[test]
fn changed_line_targets_above_visible_viewport_fall_back_to_full_redraw() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let top = CountingBlock::new("top");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 2,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("top", top.clone());
    terminal.push_live("bottom");
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("top")
        .expect("top block should exist")
        .set_text("changed top");
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("top".to_owned()),
            Operation::Newline,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("changed top".to_owned()),
            Operation::Newline,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn changed_line_commit_clones_borrowed_frame_after_successful_patch() {
    let block = CountingBlock::new("old");
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("status", block.clone());
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("status")
        .expect("status block should exist")
        .set_text("new");
    terminal.render().unwrap();

    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(vec!["new".to_owned()], 0, 1)
    );
    assert!(!terminal.needs_full_redraw);
}

#[test]
fn replacement_with_extra_current_tail_updates_changed_lines_then_inserts_tail() {
    let last_frame = committed_frame(
        vec![
            "header".to_owned(),
            "keep".to_owned(),
            "old one".to_owned(),
            "old two".to_owned(),
            "footer".to_owned(),
        ],
        0,
        5,
    );
    let current_frame = vec![
        "header".to_owned(),
        "keep".to_owned(),
        "new one".to_owned(),
        "new two".to_owned(),
        "new three".to_owned(),
        "footer".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(3),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new one"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new two"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new three"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
        ])
    );
}

#[test]
fn changed_line_planning_is_side_effect_free_and_borrows_current_lines() {
    let last_frame = committed_frame(vec!["old".to_owned()], 0, 1);
    let current_frame = vec!["new".to_owned()];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write(current_frame[0].as_str()),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
        ])
    );
}

#[test]
fn unchanged_render_flushes_without_redrawing_visible_content() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("same");
    terminal.render().unwrap();
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("same".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Flush,
        ]
    );
}

#[test]
fn mutation_apis_are_memory_only_until_render() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("live");
    terminal.push_pinned("pinned");

    assert_eq!(
        operations.operations(),
        vec![Operation::HideCursor, Operation::Flush]
    );
}

#[test]
fn empty_document_render_commits_empty_frame_with_sentinel() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![Operation::HideCursor, Operation::Flush, Operation::Flush]
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(Vec::new(), 0, 0)
    );
}

#[test]
fn blocks_receive_safe_width_saturated_at_zero() {
    let backend = RecordingBackend::default();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 1,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();
    let block = WidthRecordingBlock::default();
    let widths = block.clone();

    terminal.push_live(block);
    terminal.render().unwrap();

    assert_eq!(widths.widths(), vec![0]);
}

#[test]
fn failed_flush_does_not_commit_frame() {
    let backend = FailOnSecondFlushBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("retry");
    assert!(terminal.render().is_err());
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(Vec::new(), 0, 0)
    );
    assert!(terminal.needs_full_redraw);

    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("retry".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("retry".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn backend_operation_failure_during_render_preserves_frame_and_repairs_with_full_redraw() {
    let backend = FailOnArmedOperationBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("committed");
    terminal.render().unwrap();
    terminal.push_live("uncommitted");
    operations.fail_next_operation();

    let err = terminal
        .render()
        .expect_err("backend operation failure should be reported");

    assert!(matches!(err, TerminalError::Backend(OperationFailed)));
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(vec!["committed".to_owned()], 0, 1)
    );
    assert!(terminal.needs_full_redraw);

    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("committed".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Write("uncommitted".to_owned()),
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("committed".to_owned()),
            Operation::Newline,
            Operation::Write("uncommitted".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn selected_changed_line_operation_failures_are_transactional() {
    let selected_failures = [
        (0, Operation::MoveUp(1)),
        (1, Operation::CarriageReturn),
        (2, Operation::ClearLine),
        (3, Operation::Write("new".to_owned())),
        (4, Operation::CarriageReturn),
        (5, Operation::MoveDown(1)),
    ];

    for (successful_operations_before_failure, failed_operation) in selected_failures {
        let backend = FailOnArmedOperationBackend::default();
        let operations = backend.clone();
        let block = CountingBlock::new("old");
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.insert_live("status", block.clone());
        terminal.render().unwrap();
        terminal
            .get_live_mut::<CountingBlock, _>("status")
            .expect("status block should exist")
            .set_text("new");
        let operations_before_failure = operations.operations().len();
        operations.fail_after_successful_operations(successful_operations_before_failure);

        let err = terminal
            .render()
            .expect_err("selected backend operation should fail");

        assert!(matches!(err, TerminalError::Backend(OperationFailed)));
        assert_eq!(
            operations.operations()
                [operations_before_failure + successful_operations_before_failure],
            failed_operation
        );
        assert_eq!(
            terminal.last_committed_frame,
            committed_frame(vec!["old".to_owned()], 0, 1)
        );
        assert!(terminal.needs_full_redraw);

        terminal.render().unwrap();

        assert_eq!(
            terminal.last_committed_frame,
            committed_frame(vec!["new".to_owned()], 0, 1)
        );
        assert!(!terminal.needs_full_redraw);
    }
}

#[test]
fn structural_planner_operation_failure_is_transactional_and_repairs_with_full_redraw() {
    let backend = FailOnArmedOperationBackend::default();
    let operations = backend.clone();
    let block = LinesBlock::new(&["one", "two", "three"]);
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("lines", block.clone());
    terminal.render().unwrap();
    block.set_lines(&["one", "inserted", "two", "three"]);
    terminal
        .get_live_mut::<LinesBlock, _>("lines")
        .expect("lines block should exist");
    operations.fail_after_successful_operations(2);

    let err = terminal
        .render()
        .expect_err("structural planner operation failure should be reported");

    assert!(matches!(err, TerminalError::Backend(OperationFailed)));
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec!["one".to_owned(), "two".to_owned(), "three".to_owned()],
            0,
            3,
        )
    );
    assert!(terminal.needs_full_redraw);

    terminal.render().unwrap();

    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(
            vec![
                "one".to_owned(),
                "inserted".to_owned(),
                "two".to_owned(),
                "three".to_owned(),
            ],
            0,
            4,
        )
    );
    assert!(!terminal.needs_full_redraw);
}

#[test]
fn failed_render_preserves_dirty_state_until_successful_repair() {
    let backend = FailOnArmedOperationBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("clean");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("status", block.clone());
    terminal.render().unwrap();
    terminal
        .get_live_mut::<CountingBlock, _>("status")
        .expect("status block should exist")
        .set_text("dirty");
    operations.fail_next_operation();

    assert!(terminal.render().is_err());
    assert_eq!(block.render_count(), 2);
    assert!(terminal.needs_full_redraw);

    terminal.render().unwrap();

    assert_eq!(block.render_count(), 3);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("clean".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("dirty".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn identified_blocks_are_scoped_by_region_and_empty_ids_can_be_replaced_in_place() {
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("", NamedBlock("live first"));
    terminal.insert_pinned("", NamedBlock("pinned"));
    terminal.insert_live(String::from(""), NamedBlock("live second"));

    assert_eq!(
        terminal.get_live::<NamedBlock, _>("").map(|block| block.0),
        Some("live second")
    );
    assert_eq!(
        terminal
            .get_pinned::<NamedBlock, _>("")
            .map(|block| block.0),
        Some("pinned")
    );
}

#[test]
fn identified_replacement_preserves_order_and_remove_reports_whether_anything_was_removed() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("before");
    terminal.insert_live("stream", "first");
    terminal.push_live("after");
    terminal.insert_live("stream", "second");
    terminal.render().unwrap();

    assert!(!terminal.remove_live("missing"));
    let id = String::from("stream");
    assert!(terminal.remove_live(&id));
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("before".to_owned()),
            Operation::Newline,
            Operation::Write("second".to_owned()),
            Operation::Newline,
            Operation::Write("after".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::MoveDown(1),
            Operation::Flush,
        ]
    );
}

#[test]
fn clear_live_and_clear_pinned_remove_region_contents_independently() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("anonymous live");
    terminal.insert_live("live", NamedBlock("live"));
    terminal.push_pinned("anonymous pinned");
    terminal.insert_pinned("pinned", NamedBlock("pinned"));

    terminal.clear_pinned();
    assert!(terminal.get_pinned::<NamedBlock, _>("pinned").is_none());
    terminal.render().unwrap();

    terminal.clear_live();
    assert!(terminal.get_live::<NamedBlock, _>("live").is_none());
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("anonymous live".to_owned()),
            Operation::Newline,
            Operation::Write("live".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(2),
            Operation::CarriageReturn,
            Operation::DeleteLines(2),
            Operation::Flush,
        ]
    );
}

#[test]
fn mutable_typed_lookup_marks_only_matching_block_dirty() {
    let first = CountingBlock::new("first");
    let second = CountingBlock::new("second");
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("first", first.clone());
    terminal.insert_live("second", second.clone());
    terminal.render().unwrap();
    terminal.render().unwrap();
    assert_eq!((first.render_count(), second.render_count()), (1, 1));

    assert!(terminal.get_live_mut::<OtherBlock, _>("first").is_none());
    terminal.render().unwrap();
    assert_eq!((first.render_count(), second.render_count()), (1, 1));

    terminal
        .get_live_mut::<CountingBlock, _>("first")
        .expect("matching mutable lookup should succeed")
        .set_text("changed");
    terminal.render().unwrap();

    assert_eq!((first.render_count(), second.render_count()), (2, 1));
}

#[test]
fn clean_caches_are_reused_while_every_frame_blocks_render_each_attempt() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let regular = CountingBlock::new("regular");
    let every_frame = CountingBlock::every_frame("dynamic");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live(regular.clone());
    terminal.push_live(every_frame.clone());
    terminal.render().unwrap();
    terminal.render().unwrap();

    assert_eq!(regular.render_count(), 1);
    assert_eq!(every_frame.render_count(), 2);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("regular".to_owned()),
            Operation::Newline,
            Operation::Write("dynamic".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Flush,
        ]
    );
}

#[test]
fn invalid_resize_is_memory_only_and_leaves_committed_state_unchanged() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("stable");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live(block.clone());
    terminal.render().unwrap();
    let err = terminal
        .resize(TerminalSize {
            width: 0,
            height: 40,
        })
        .expect_err("zero width resize should fail");
    terminal.render().unwrap();

    assert!(matches!(err, TerminalError::InvalidTerminalSize));
    assert_eq!(block.render_count(), 1);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("stable".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Flush,
        ]
    );
}

#[test]
fn unchanged_size_resize_forces_full_redraw_without_dirtying_clean_caches() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("same size");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live(block.clone());
    terminal.render().unwrap();
    terminal
        .resize(TerminalSize {
            width: 80,
            height: 24,
        })
        .unwrap();
    terminal.render().unwrap();

    assert_eq!(block.render_count(), 1);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("same size".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("same size".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn width_changes_dirty_all_blocks_but_height_only_resize_reuses_clean_caches() {
    let live = CountingBlock::new("live");
    let pinned = CountingBlock::new("pinned");
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live(live.clone());
    terminal.push_pinned(pinned.clone());
    terminal.render().unwrap();
    assert_eq!((live.render_count(), pinned.render_count()), (1, 1));

    terminal
        .resize(TerminalSize {
            width: 80,
            height: 40,
        })
        .unwrap();
    terminal.render().unwrap();
    assert_eq!((live.render_count(), pinned.render_count()), (1, 1));

    terminal
        .resize(TerminalSize {
            width: 40,
            height: 40,
        })
        .unwrap();
    terminal.render().unwrap();
    assert_eq!((live.render_count(), pinned.render_count()), (2, 2));
}

#[test]
fn resize_after_finish_start_or_completion_does_not_write_or_reopen_rendering() {
    let backend = FailOnSecondFlushBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("partial finish");
    assert!(terminal.finish().is_err());
    let before_resize = operations.operations();
    terminal
        .resize(TerminalSize {
            width: 40,
            height: 12,
        })
        .unwrap();
    let err = terminal.render().expect_err("render should stay rejected");

    assert_eq!(operations.operations(), before_resize);
    assert!(matches!(
        err,
        TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
    ));

    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.finish().unwrap();
    let before_resize = operations.operations();
    terminal
        .resize(TerminalSize {
            width: 100,
            height: 30,
        })
        .unwrap();
    let render_err = terminal.render().expect_err("render should stay rejected");
    let finish_err = terminal.finish().expect_err("finish should stay completed");

    assert_eq!(operations.operations(), before_resize);
    assert!(matches!(
        render_err,
        TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
    ));
    assert!(matches!(
        finish_err,
        TerminalError::Lifecycle(LifecycleError::AlreadyFinished)
    ));
}

#[test]
fn force_full_redraw_is_memory_only_and_preserves_cache_dirty_state() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let block = CountingBlock::new("clean");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("status", block.clone());
    terminal.render().unwrap();
    let before_force = operations.operations();
    terminal.force_full_redraw();
    assert_eq!(operations.operations(), before_force);
    terminal.render().unwrap();
    assert_eq!(block.render_count(), 1);

    terminal
        .get_live_mut::<CountingBlock, _>("status")
        .expect("status block should be present")
        .set_text("dirty");
    terminal.force_full_redraw();
    terminal.render().unwrap();

    assert_eq!(block.render_count(), 2);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("clean".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("clean".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("dirty".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn force_full_redraw_before_finish_renders_live_frame_before_cursor_restore() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("durable");
    terminal.render().unwrap();
    terminal.force_full_redraw();
    terminal.finish().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("durable".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("durable".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn backend_operation_failure_during_finish_preserves_retry_state_and_repairs() {
    let backend = FailOnArmedOperationBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("committed");
    terminal.render().unwrap();
    terminal.push_live("failed final");
    operations.fail_next_operation();

    let err = terminal
        .finish()
        .expect_err("backend operation failure should be reported");

    assert!(matches!(err, TerminalError::Backend(OperationFailed)));
    assert_eq!(
        terminal.lifecycle,
        Lifecycle::Finishing {
            final_render_complete: false,
        }
    );
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(vec!["committed".to_owned()], 0, 1)
    );
    assert!(terminal.needs_full_redraw);

    terminal.clear_live();
    terminal.push_live("repaired final");
    terminal.finish().unwrap();

    assert_eq!(terminal.lifecycle, Lifecycle::Finished);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("committed".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::Write("failed final".to_owned()),
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("repaired final".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn finish_renders_live_only_restores_cursor_flushes_and_completes() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("final live");
    terminal.push_pinned("transient pinned");
    terminal.finish().unwrap();

    assert_eq!(terminal.lifecycle, Lifecycle::Finished);
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("final live".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn render_errors_after_finish_starts_even_when_finish_fails() {
    let backend = FailOnSecondFlushBackend::default();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("final live");
    assert!(terminal.finish().is_err());

    let err = terminal.render().expect_err("render should be rejected");
    assert!(matches!(
        err,
        TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
    ));
}

#[test]
fn retry_finish_after_failed_final_render_rerenders_current_live_region() {
    let backend = FailOnSecondFlushBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("first");
    assert!(terminal.finish().is_err());
    terminal.clear_live();
    terminal.push_live("second");

    terminal.finish().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("first".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("second".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn retry_finish_after_final_render_succeeds_retries_only_cursor_restore_and_flush() {
    let backend = FailOnSecondFlushBackend::fail_on_flush(3);
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("already rendered");
    assert!(terminal.finish().is_err());
    terminal.clear_live();
    terminal.push_live("should stay memory-only");

    terminal.finish().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("already rendered".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn mutations_after_finished_are_memory_only_and_do_not_enable_render() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.finish().unwrap();
    terminal.insert_live("live", NamedBlock("stored live"));
    terminal.insert_pinned("pinned", NamedBlock("stored pinned"));

    assert_eq!(
        terminal
            .get_live::<NamedBlock, _>("live")
            .map(|block| block.0),
        Some("stored live")
    );
    assert_eq!(
        terminal
            .get_pinned::<NamedBlock, _>("pinned")
            .map(|block| block.0),
        Some("stored pinned")
    );
    let err = terminal.render().expect_err("render should stay rejected");
    assert!(matches!(
        err,
        TerminalError::Lifecycle(LifecycleError::RenderAfterFinishStarted)
    ));
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn finish_after_finished_errors_and_drop_writes_no_cleanup() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();

    {
        let mut terminal = Terminal::new(
            backend,
            TerminalSize {
                width: 80,
                height: 24,
            },
            CursorPosition { row: 0, column: 0 },
        )
        .unwrap();

        terminal.push_live("done");
        terminal.finish().unwrap();
        let err = terminal.finish().expect_err("second finish should fail");
        assert!(matches!(
            err,
            TerminalError::Lifecycle(LifecycleError::AlreadyFinished)
        ));
    }

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("done".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn finish_preserves_pinned_cache_without_rendering_it() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let pinned = CountingBlock::new("pinned");
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live("live");
    terminal.insert_pinned("status", pinned.clone());
    terminal.render().unwrap();
    assert_eq!(pinned.render_count(), 1);

    terminal.finish().unwrap();

    assert_eq!(pinned.render_count(), 1);
    assert!(terminal.get_pinned::<CountingBlock, _>("status").is_some());
    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("live".to_owned()),
            Operation::Newline,
            Operation::Write("pinned".to_owned()),
            Operation::Newline,
            Operation::Flush,
            Operation::MoveUp(1),
            Operation::CarriageReturn,
            Operation::DeleteLines(1),
            Operation::Flush,
            Operation::ShowCursor,
            Operation::Flush,
        ]
    );
}

#[test]
fn block_every_frame_hook_defaults_to_false() {
    struct MinimalBlock;

    impl Block for MinimalBlock {
        fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
            Vec::new()
        }
    }

    assert!(!MinimalBlock.render_every_frame());
}

#[test]
fn built_in_owned_string_and_cow_blocks_render() {
    let backend = RecordingBackend::default();
    let operations = backend.clone();
    let mut terminal = Terminal::new(
        backend,
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.push_live(String::from("owned string"));
    terminal.push_pinned(Cow::Borrowed("borrowed cow"));
    terminal.render().unwrap();

    assert_eq!(
        operations.operations(),
        vec![
            Operation::HideCursor,
            Operation::Flush,
            Operation::Write("owned string".to_owned()),
            Operation::Newline,
            Operation::Write("borrowed cow".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
    );
}

#[test]
fn pinned_mutation_and_removal_update_only_pinned_region() {
    let live = CountingBlock::new("live");
    let pinned = CountingBlock::new("old pinned");
    let mut terminal = Terminal::new(
        RecordingBackend::default(),
        TerminalSize {
            width: 80,
            height: 24,
        },
        CursorPosition { row: 0, column: 0 },
    )
    .unwrap();

    terminal.insert_live("live", live.clone());
    terminal.insert_pinned("status", pinned.clone());
    terminal.render().unwrap();

    terminal
        .get_pinned_mut::<CountingBlock, _>("status")
        .expect("pinned status should exist")
        .set_text("new pinned");
    terminal.render().unwrap();

    assert_eq!(live.render_count(), 1);
    assert_eq!(pinned.render_count(), 2);
    assert_eq!(
        terminal.last_committed_frame,
        committed_frame(vec!["live".to_owned(), "new pinned".to_owned()], 0, 2)
    );
    assert!(terminal.remove_pinned("status"));
    assert!(!terminal.remove_pinned("status"));
}

#[test]
fn replacement_with_shorter_current_updates_changed_line_then_deletes_tail() {
    let last_frame = committed_frame(
        vec![
            "header".to_owned(),
            "keep".to_owned(),
            "old one".to_owned(),
            "old two".to_owned(),
            "old three".to_owned(),
            "footer".to_owned(),
        ],
        0,
        6,
    );
    let current_frame = vec![
        "header".to_owned(),
        "keep".to_owned(),
        "new one".to_owned(),
        "footer".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(4),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("new one"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::DeleteLines(2),
            PlannedOperation::MoveDown(1),
        ])
    );
}

#[test]
fn trailing_append_with_earlier_insert_restores_to_final_sentinel() {
    let last_frame = committed_frame(vec!["a".to_owned(), "b".to_owned()], 0, 2);
    let current_frame = vec![
        "x".to_owned(),
        "a".to_owned(),
        "b".to_owned(),
        "c".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 24, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(2),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("x"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(3),
            PlannedOperation::Write("c"),
            PlannedOperation::Newline,
        ])
    );
}

#[test]
fn middle_insert_translates_following_changed_line_without_sentinel_round_trip() {
    let last_frame = committed_frame(
        vec![
            "a".to_owned(),
            "b".to_owned(),
            "c".to_owned(),
            "d".to_owned(),
        ],
        0,
        4,
    );
    let current_frame = vec![
        "a".to_owned(),
        "x".to_owned(),
        "b".to_owned(),
        "c2".to_owned(),
        "d".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 8, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(3),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("x"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("c2"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
        ])
    );
}

#[test]
fn mixed_structural_patches_do_not_force_full_redraw_when_targets_remain_visible() {
    let last_frame = committed_frame(
        vec!["card".to_owned(), "notice".to_owned(), "prompt".to_owned()],
        0,
        3,
    );
    let current_frame = vec![
        "expanded card".to_owned(),
        "details".to_owned(),
        "notice".to_owned(),
        "paragraph".to_owned(),
        "next prompt".to_owned(),
    ];

    let plan = plan_frame_render(&last_frame, &current_frame, 10, false);

    assert_eq!(
        plan,
        FramePlan::ChangedLines(vec![
            PlannedOperation::MoveUp(3),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("expanded card"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::CarriageReturn,
            PlannedOperation::InsertLines(1),
            PlannedOperation::ClearLine,
            PlannedOperation::Write("details"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(2),
            PlannedOperation::CarriageReturn,
            PlannedOperation::ClearLine,
            PlannedOperation::Write("paragraph"),
            PlannedOperation::CarriageReturn,
            PlannedOperation::MoveDown(1),
            PlannedOperation::Write("next prompt"),
            PlannedOperation::Newline,
        ])
    );
}
