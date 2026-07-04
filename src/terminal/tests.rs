use std::borrow::Cow;

use super::{
    diff::{DiffOp, DocumentPatch, patience_diff, translate_diff_to_patches},
    frame::CommittedFrame,
    rendering::{FramePlan, PlannedOperation, plan_frame_render},
    *,
};
use crate::test_utils::*;

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
    assert_eq!(terminal.last_committed_frame.sentinel_row, 4);
}

#[test]
fn changed_line_plus_trailing_append_appends_before_patching_old_coordinate() {
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
            Operation::Write("tail".to_owned()),
            Operation::Newline,
            Operation::MoveUp(3),
            Operation::CarriageReturn,
            Operation::ClearLine,
            Operation::Write("new".to_owned()),
            Operation::CarriageReturn,
            Operation::MoveDown(3),
            Operation::Flush,
        ]
    );
}

#[test]
fn append_induced_scroll_hiding_remaining_patch_falls_back_to_full_redraw() {
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
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("top".to_owned()),
            Operation::Newline,
            Operation::Write("new middle".to_owned()),
            Operation::Newline,
            Operation::Write("bottom".to_owned()),
            Operation::Newline,
            Operation::Write("tail one".to_owned()),
            Operation::Newline,
            Operation::Write("tail two".to_owned()),
            Operation::Newline,
            Operation::Flush,
        ]
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
        CommittedFrame {
            lines: vec!["new".to_owned()],
            sentinel_row: 1,
        }
    );
    assert!(!terminal.needs_full_redraw);
}

#[test]
fn patience_diff_uses_unique_anchors_and_translates_replacements_with_tails() {
    let old = vec!["header", "keep", "old one", "old two", "footer"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let current = vec![
        "header",
        "keep",
        "new one",
        "new two",
        "new three",
        "footer",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    let diff = patience_diff(&old, &current);
    let patches = translate_diff_to_patches(&diff);

    assert_eq!(
        diff,
        vec![
            DiffOp::Equal {
                old: 0..2,
                current: 0..2,
            },
            DiffOp::Delete { old: 2..4 },
            DiffOp::Insert { current: 2..5 },
            DiffOp::Equal {
                old: 4..5,
                current: 5..6,
            },
        ]
    );
    assert_eq!(
        patches,
        vec![
            DocumentPatch::ChangedLine {
                old_row: 2,
                current_row: 2,
            },
            DocumentPatch::ChangedLine {
                old_row: 3,
                current_row: 3,
            },
            DocumentPatch::InsertLines {
                old_row: 4,
                current: 4..5,
            },
        ]
    );
}

#[test]
fn changed_line_planning_is_side_effect_free_and_borrows_current_lines() {
    let last_frame = CommittedFrame {
        lines: vec!["old".to_owned()],
        sentinel_row: 1,
    };
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
        CommittedFrame {
            lines: Vec::new(),
            sentinel_row: 0,
        }
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
        CommittedFrame {
            lines: Vec::new(),
            sentinel_row: 0,
        }
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
        .err()
        .expect("backend operation failure should be reported");

    assert!(matches!(err, TerminalError::Backend(OperationFailed)));
    assert_eq!(
        terminal.last_committed_frame,
        CommittedFrame {
            lines: vec!["committed".to_owned()],
            sentinel_row: 1,
        }
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
            .err()
            .expect("selected backend operation should fail");

        assert!(matches!(err, TerminalError::Backend(OperationFailed)));
        assert_eq!(
            operations.operations()
                [operations_before_failure + successful_operations_before_failure],
            failed_operation
        );
        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: vec!["old".to_owned()],
                sentinel_row: 1,
            }
        );
        assert!(terminal.needs_full_redraw);

        terminal.render().unwrap();

        assert_eq!(
            terminal.last_committed_frame,
            CommittedFrame {
                lines: vec!["new".to_owned()],
                sentinel_row: 1,
            }
        );
        assert!(!terminal.needs_full_redraw);
    }
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
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("before".to_owned()),
            Operation::Newline,
            Operation::Write("after".to_owned()),
            Operation::Newline,
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
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
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
        .err()
        .expect("zero width resize should fail");
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
    let err = terminal
        .render()
        .err()
        .expect("render should stay rejected");

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
    let render_err = terminal
        .render()
        .err()
        .expect("render should stay rejected");
    let finish_err = terminal
        .finish()
        .err()
        .expect("finish should stay completed");

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
        .err()
        .expect("backend operation failure should be reported");

    assert!(matches!(err, TerminalError::Backend(OperationFailed)));
    assert_eq!(
        terminal.lifecycle,
        Lifecycle::Finishing {
            final_render_complete: false,
        }
    );
    assert_eq!(
        terminal.last_committed_frame,
        CommittedFrame {
            lines: vec!["committed".to_owned()],
            sentinel_row: 1,
        }
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

    let err = terminal.render().err().expect("render should be rejected");
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
    let err = terminal
        .render()
        .err()
        .expect("render should stay rejected");
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
        let err = terminal.finish().err().expect("second finish should fail");
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
            Operation::ClearScreen,
            Operation::PurgeScrollback,
            Operation::MoveToTopLeft,
            Operation::Write("live".to_owned()),
            Operation::Newline,
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
