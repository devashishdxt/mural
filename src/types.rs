/// Physical terminal dimensions supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    /// Number of terminal columns available for rendering.
    pub width: usize,

    /// Number of visible terminal rows available for viewport calculations.
    pub height: usize,
}

/// Zero-based cursor position supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorPosition {
    /// Zero-based visible row containing the cursor at construction time.
    pub row: usize,

    /// Zero-based column containing the cursor at construction time.
    pub column: usize,
}
