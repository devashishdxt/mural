use std::ops::Deref;

use similar::Algorithm;

use crate::frame::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOp {
    Delete {
        old_index: usize,
        old_len: usize,
        new_index: usize,
    },
    Insert {
        old_index: usize,
        new_index: usize,
        new_len: usize,
    },
    Replace {
        old_index: usize,
        old_len: usize,
        new_index: usize,
        new_len: usize,
    },
}

impl DiffOp {
    pub fn old_index(&self) -> usize {
        match self {
            Self::Delete { old_index, .. } => *old_index,
            Self::Insert { old_index, .. } => *old_index,
            Self::Replace { old_index, .. } => *old_index,
        }
    }
}

pub struct Diff {
    diff: Vec<DiffOp>,
}

impl Diff {
    pub fn sort(mut self) -> SortedDiff {
        self.diff.sort_by_key(DiffOp::old_index);
        SortedDiff { diff: self.diff }
    }

    pub fn normalize(self) -> NormalizedDiff {
        self.sort().normalize()
    }
}

impl FromIterator<similar::DiffOp> for Diff {
    fn from_iter<T: IntoIterator<Item = similar::DiffOp>>(iter: T) -> Self {
        let diff = iter
            .into_iter()
            .filter_map(|diff_op| match diff_op {
                similar::DiffOp::Equal { .. } => None,
                similar::DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                } => Some(DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                }),
                similar::DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => Some(DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                }),
                similar::DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => Some(DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                }),
            })
            .collect();

        Self { diff }
    }
}

pub struct SortedDiff {
    diff: Vec<DiffOp>,
}

impl SortedDiff {
    fn normalize(mut self) -> NormalizedDiff {
        let mut index_shift = 0isize;

        for diff_op in self.diff.iter_mut() {
            match diff_op {
                DiffOp::Delete {
                    old_index, old_len, ..
                } => {
                    *old_index = shifted_index(*old_index, index_shift);
                    index_shift -= *old_len as isize;
                }
                DiffOp::Insert {
                    old_index, new_len, ..
                } => {
                    *old_index = shifted_index(*old_index, index_shift);
                    index_shift += *new_len as isize;
                }
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_len,
                    ..
                } => {
                    *old_index = shifted_index(*old_index, index_shift);
                    index_shift += *new_len as isize - *old_len as isize;
                }
            }
        }

        NormalizedDiff { diff: self.diff }
    }
}

pub struct NormalizedDiff {
    diff: Vec<DiffOp>,
}

impl IntoIterator for NormalizedDiff {
    type Item = DiffOp;

    type IntoIter = std::vec::IntoIter<DiffOp>;

    fn into_iter(self) -> Self::IntoIter {
        self.diff.into_iter()
    }
}

impl Deref for NormalizedDiff {
    type Target = Vec<DiffOp>;

    fn deref(&self) -> &Self::Target {
        &self.diff
    }
}

pub trait Differ {
    fn diff(&self, old: &Frame, new: &Frame) -> Diff;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyersDiffer;

impl Differ for MyersDiffer {
    fn diff(&self, old: &Frame, new: &Frame) -> Diff {
        similar::capture_diff(Algorithm::Myers, old, 0..old.len(), new, 0..new.len())
            .into_iter()
            .collect()
    }
}

fn shifted_index(old_index: usize, shift: isize) -> usize {
    if shift < 0 {
        old_index
            .checked_sub(shift.unsigned_abs())
            .expect("diff index shift moved before start of slice")
    } else {
        old_index + shift as usize
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::rc::Rc;

    use similar::DiffOp as SimilarDiffOp;

    use super::{Diff, DiffOp, Differ, MyersDiffer, shifted_index};
    use crate::frame::{Frame, RenderedLines};

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

    #[test]
    fn conversion_ignores_equal_and_preserves_every_edit_kind() {
        let diff: Diff = [
            SimilarDiffOp::Equal {
                old_index: 0,
                new_index: 0,
                len: 1,
            },
            SimilarDiffOp::Delete {
                old_index: 1,
                old_len: 2,
                new_index: 1,
            },
            SimilarDiffOp::Insert {
                old_index: 3,
                new_index: 1,
                new_len: 1,
            },
            SimilarDiffOp::Replace {
                old_index: 4,
                old_len: 1,
                new_index: 2,
                new_len: 2,
            },
        ]
        .into_iter()
        .collect();

        assert_eq!(
            diff.diff,
            [
                DiffOp::Delete {
                    old_index: 1,
                    old_len: 2,
                    new_index: 1,
                },
                DiffOp::Insert {
                    old_index: 3,
                    new_index: 1,
                    new_len: 1,
                },
                DiffOp::Replace {
                    old_index: 4,
                    old_len: 1,
                    new_index: 2,
                    new_len: 2,
                },
            ]
        );
    }

    #[test]
    fn normalization_sorts_and_accounts_for_prior_edits() {
        let normalized = Diff {
            diff: vec![
                DiffOp::Delete {
                    old_index: 4,
                    old_len: 2,
                    new_index: 0,
                },
                DiffOp::Replace {
                    old_index: 3,
                    old_len: 1,
                    new_index: 0,
                    new_len: 2,
                },
                DiffOp::Insert {
                    old_index: 0,
                    new_index: 0,
                    new_len: 1,
                },
            ],
        }
        .normalize();

        assert_eq!(
            normalized.as_slice(),
            [
                DiffOp::Insert {
                    old_index: 0,
                    new_index: 0,
                    new_len: 1,
                },
                DiffOp::Replace {
                    old_index: 4,
                    old_len: 1,
                    new_index: 0,
                    new_len: 2,
                },
                DiffOp::Delete {
                    old_index: 6,
                    old_len: 2,
                    new_index: 0,
                },
            ]
        );
        assert_eq!(normalized.into_iter().count(), 3);
    }

    #[test]
    fn deletion_shifts_later_operations_back() {
        let normalized = Diff {
            diff: vec![
                DiffOp::Delete {
                    old_index: 0,
                    old_len: 2,
                    new_index: 0,
                },
                DiffOp::Insert {
                    old_index: 2,
                    new_index: 0,
                    new_len: 1,
                },
            ],
        }
        .normalize();

        assert_eq!(normalized[1].old_index(), 0);
    }

    #[test]
    fn myers_differ_captures_changed_lines() {
        let old = frame(&["same", "old"]);
        let new = frame(&["same", "new", "extra"]);

        let normalized = MyersDiffer.diff(&old, &new).normalize();

        assert_eq!(
            normalized.as_slice(),
            [DiffOp::Replace {
                old_index: 1,
                old_len: 1,
                new_index: 1,
                new_len: 2,
            }]
        );
    }

    #[test]
    fn each_operation_exposes_its_old_index() {
        let operations = [
            DiffOp::Delete {
                old_index: 1,
                old_len: 0,
                new_index: 0,
            },
            DiffOp::Insert {
                old_index: 2,
                new_index: 0,
                new_len: 0,
            },
            DiffOp::Replace {
                old_index: 3,
                old_len: 0,
                new_index: 0,
                new_len: 0,
            },
        ];

        assert_eq!(operations.map(|operation| operation.old_index()), [1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "diff index shift moved before start of slice")]
    fn invalid_negative_shift_panics() {
        shifted_index(0, -1);
    }
}
