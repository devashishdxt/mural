use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TerminalError {
    #[error("terminal has already finished")]
    Finished,
    #[error("block id `{id}` already exists")]
    DuplicateBlockId { id: String },
    #[error("block id `{id}` was not found")]
    MissingBlockId { id: String },
    #[error("block id `{id}` is a pinned block, not a live block")]
    ExpectedLiveBlock { id: String },
    #[error("block id `{id}` is a live block, not a pinned block")]
    ExpectedPinnedBlock { id: String },
    #[error("block id `{id}` has type `{actual}`, not `{expected}`")]
    WrongBlockType {
        id: String,
        expected: &'static str,
        actual: &'static str,
    },
}
