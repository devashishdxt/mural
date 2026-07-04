use std::{collections::HashMap, ops::Range};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum DiffOp {
    Equal {
        old: Range<usize>,
        current: Range<usize>,
    },
    Delete {
        old: Range<usize>,
    },
    Insert {
        current: Range<usize>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DocumentPatch {
    ChangedLine {
        old_row: usize,
        current_row: usize,
    },
    InsertLines {
        old_row: usize,
        current: Range<usize>,
    },
    DeleteLines {
        old: Range<usize>,
    },
}

pub(super) fn translate_diff_to_patches(diff: &[DiffOp]) -> Vec<DocumentPatch> {
    let mut patches = Vec::new();
    let mut index = 0;

    while index < diff.len() {
        match &diff[index] {
            DiffOp::Equal { .. } => index += 1,
            DiffOp::Delete { old } => {
                if let Some(DiffOp::Insert { current }) = diff.get(index + 1) {
                    push_replace_patches(&mut patches, old.clone(), current.clone());
                    index += 2;
                } else {
                    patches.push(DocumentPatch::DeleteLines { old: old.clone() });
                    index += 1;
                }
            }
            DiffOp::Insert { current } => {
                patches.push(DocumentPatch::InsertLines {
                    old_row: insertion_old_row(diff, index),
                    current: current.clone(),
                });
                index += 1;
            }
        }
    }

    patches
}

fn push_replace_patches(
    patches: &mut Vec<DocumentPatch>,
    old: Range<usize>,
    current: Range<usize>,
) {
    let changed_line_count = old.len().min(current.len());
    for offset in 0..changed_line_count {
        patches.push(DocumentPatch::ChangedLine {
            old_row: old.start + offset,
            current_row: current.start + offset,
        });
    }

    if current.len() > changed_line_count {
        patches.push(DocumentPatch::InsertLines {
            old_row: old.start + changed_line_count,
            current: current.start + changed_line_count..current.end,
        });
    } else if old.len() > changed_line_count {
        patches.push(DocumentPatch::DeleteLines {
            old: old.start + changed_line_count..old.end,
        });
    }
}

fn insertion_old_row(diff: &[DiffOp], insert_index: usize) -> usize {
    diff[..insert_index]
        .iter()
        .rev()
        .find_map(|operation| match operation {
            DiffOp::Equal { old, .. } | DiffOp::Delete { old } => Some(old.end),
            DiffOp::Insert { .. } => None,
        })
        .unwrap_or(0)
}

pub(super) fn patience_diff(old: &[String], current: &[String]) -> Vec<DiffOp> {
    let mut operations = Vec::new();
    patience_diff_range(
        old,
        0..old.len(),
        current,
        0..current.len(),
        &mut operations,
    );
    coalesce_diff_operations(operations)
}

fn patience_diff_range(
    old: &[String],
    old_range: Range<usize>,
    current: &[String],
    current_range: Range<usize>,
    operations: &mut Vec<DiffOp>,
) {
    if old_range.is_empty() {
        push_insert(operations, current_range);
        return;
    }
    if current_range.is_empty() {
        push_delete(operations, old_range);
        return;
    }

    let mut old_start = old_range.start;
    let mut current_start = current_range.start;
    while old_start < old_range.end
        && current_start < current_range.end
        && old[old_start] == current[current_start]
    {
        old_start += 1;
        current_start += 1;
    }
    push_equal(
        operations,
        old_range.start..old_start,
        current_range.start..current_start,
    );

    let mut old_end = old_range.end;
    let mut current_end = current_range.end;
    while old_start < old_end
        && current_start < current_end
        && old[old_end - 1] == current[current_end - 1]
    {
        old_end -= 1;
        current_end -= 1;
    }

    let anchors = patience_anchors(old, old_start..old_end, current, current_start..current_end);
    if anchors.is_empty() {
        push_delete(operations, old_start..old_end);
        push_insert(operations, current_start..current_end);
    } else {
        let mut previous_old = old_start;
        let mut previous_current = current_start;
        for (old_index, current_index) in anchors {
            patience_diff_range(
                old,
                previous_old..old_index,
                current,
                previous_current..current_index,
                operations,
            );
            push_equal(
                operations,
                old_index..old_index + 1,
                current_index..current_index + 1,
            );
            previous_old = old_index + 1;
            previous_current = current_index + 1;
        }
        patience_diff_range(
            old,
            previous_old..old_end,
            current,
            previous_current..current_end,
            operations,
        );
    }

    push_equal(
        operations,
        old_end..old_range.end,
        current_end..current_range.end,
    );
}

fn patience_anchors(
    old: &[String],
    old_range: Range<usize>,
    current: &[String],
    current_range: Range<usize>,
) -> Vec<(usize, usize)> {
    let mut old_counts: HashMap<&str, (usize, usize)> = HashMap::new();
    for index in old_range.clone() {
        let entry = old_counts.entry(old[index].as_str()).or_insert((0, index));
        entry.0 += 1;
        entry.1 = index;
    }

    let mut current_counts: HashMap<&str, (usize, usize)> = HashMap::new();
    for index in current_range {
        let entry = current_counts
            .entry(current[index].as_str())
            .or_insert((0, index));
        entry.0 += 1;
        entry.1 = index;
    }

    let candidates = old_range
        .filter_map(|old_index| {
            let line = old[old_index].as_str();
            let (1, _) = old_counts.get(line).copied()? else {
                return None;
            };
            let (1, current_index) = current_counts.get(line).copied()? else {
                return None;
            };
            Some((old_index, current_index))
        })
        .collect::<Vec<_>>();

    longest_increasing_subsequence_by_current_index(candidates)
}

fn longest_increasing_subsequence_by_current_index(
    candidates: Vec<(usize, usize)>,
) -> Vec<(usize, usize)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut lengths = vec![1; candidates.len()];
    let mut previous = vec![None; candidates.len()];
    for index in 0..candidates.len() {
        for preceding in 0..index {
            if candidates[preceding].1 < candidates[index].1
                && lengths[preceding] + 1 > lengths[index]
            {
                lengths[index] = lengths[preceding] + 1;
                previous[index] = Some(preceding);
            }
        }
    }

    let mut best_index = (0..candidates.len())
        .max_by_key(|&index| (lengths[index], std::cmp::Reverse(candidates[index].1)))
        .expect("non-empty candidates should have a best index");
    let mut sequence = Vec::new();
    loop {
        sequence.push(candidates[best_index]);
        let Some(preceding) = previous[best_index] else {
            break;
        };
        best_index = preceding;
    }
    sequence.reverse();
    sequence
}

fn coalesce_diff_operations(operations: Vec<DiffOp>) -> Vec<DiffOp> {
    let mut coalesced = Vec::new();
    for operation in operations {
        match (coalesced.last_mut(), operation) {
            (
                Some(DiffOp::Equal { old, current }),
                DiffOp::Equal {
                    old: next_old,
                    current: next_current,
                },
            ) if old.end == next_old.start && current.end == next_current.start => {
                old.end = next_old.end;
                current.end = next_current.end;
            }
            (Some(DiffOp::Delete { old }), DiffOp::Delete { old: next_old })
                if old.end == next_old.start =>
            {
                old.end = next_old.end;
            }
            (
                Some(DiffOp::Insert { current }),
                DiffOp::Insert {
                    current: next_current,
                },
            ) if current.end == next_current.start => {
                current.end = next_current.end;
            }
            (_, DiffOp::Equal { old, current }) if old.is_empty() && current.is_empty() => {}
            (_, DiffOp::Delete { old }) if old.is_empty() => {}
            (_, DiffOp::Insert { current }) if current.is_empty() => {}
            (_, operation) => coalesced.push(operation),
        }
    }
    coalesced
}

fn push_equal(operations: &mut Vec<DiffOp>, old: Range<usize>, current: Range<usize>) {
    if !old.is_empty() || !current.is_empty() {
        operations.push(DiffOp::Equal { old, current });
    }
}

fn push_delete(operations: &mut Vec<DiffOp>, old: Range<usize>) {
    if !old.is_empty() {
        operations.push(DiffOp::Delete { old });
    }
}

fn push_insert(operations: &mut Vec<DiffOp>, current: Range<usize>) {
    if !current.is_empty() {
        operations.push(DiffOp::Insert { current });
    }
}
