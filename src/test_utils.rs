mod blocks;
mod failing_backend;
mod recording_backend;

pub(crate) use blocks::{CountingBlock, LinesBlock, NamedBlock, OtherBlock, WidthRecordingBlock};
pub(crate) use failing_backend::{
    FailOnArmedOperationBackend, FailOnSecondFlushBackend, OperationFailed,
};
pub(crate) use recording_backend::{Operation, RecordingBackend};
