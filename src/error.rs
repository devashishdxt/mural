//! Error types produced by terminal block management APIs.

use thiserror::Error;

/// Typed errors for identified block operations and post-finish mutations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TerminalError {
    /// The terminal has already been finished and no further mutation is allowed.
    #[error("terminal has already finished")]
    Finished,
    /// An identified block already exists with the requested id.
    #[error("block id `{id}` already exists")]
    DuplicateBlockId {
        /// Duplicate block id.
        id: String,
    },
    /// No identified block exists with the requested id.
    #[error("block id `{id}` was not found")]
    MissingBlockId {
        /// Missing block id.
        id: String,
    },
    /// The id names a pinned block when a live block was required.
    #[error("block id `{id}` is a pinned block, not a live block")]
    ExpectedLiveBlock {
        /// Block id found in the wrong region.
        id: String,
    },
    /// The id names a live block when a pinned block was required.
    #[error("block id `{id}` is a live block, not a pinned block")]
    ExpectedPinnedBlock {
        /// Block id found in the wrong region.
        id: String,
    },
    /// The id exists, but the caller requested the wrong concrete block type.
    #[error("block id `{id}` has type `{actual}`, not `{expected}`")]
    WrongBlockType {
        /// Block id whose concrete type did not match.
        id: String,
        /// Type requested by the caller.
        expected: &'static str,
        /// Actual type stored for the block.
        actual: &'static str,
    },
}
