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
