/// Lifecycle errors reported by rendering and finalization operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LifecycleError {
    /// A render was requested after finishing had started.
    #[error("render is disallowed after finish has started")]
    RenderAfterFinishStarted,

    /// Finish was requested after the terminal had already completed cleanup.
    #[error("finish was called after the terminal had already finished")]
    AlreadyFinished,
}

/// Errors reported by terminal construction and rendering lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// The supplied terminal size had a zero width or height.
    #[error("invalid terminal size: width and height must be greater than zero")]
    InvalidTerminalSize,

    /// The supplied cursor position was outside the supplied terminal size.
    #[error("invalid cursor position: row/column must be within terminal size")]
    InvalidCursorPosition,

    /// The operation was invalid for the terminal lifecycle state.
    #[error(transparent)]
    Lifecycle(LifecycleError),

    /// The backend failed while committing semantic terminal operations.
    #[error(transparent)]
    Backend(#[from] E),
}
