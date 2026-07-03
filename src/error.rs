/// Lifecycle errors reported by rendering and finalization operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LifecycleError {
    #[error("render is disallowed after finish has started")]
    RenderAfterFinishStarted,
    #[error("finish was called after the terminal had already finished")]
    AlreadyFinished,
}

/// Errors reported by terminal construction and rendering lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("invalid terminal size: width and height must be greater than zero")]
    InvalidTerminalSize,
    #[error("invalid cursor position: row/column must be within terminal size")]
    InvalidCursorPosition,
    #[error(transparent)]
    Lifecycle(LifecycleError),
    #[error(transparent)]
    Backend(#[from] E),
}
