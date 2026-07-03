/// Physical terminal dimensions supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub width: usize,
    pub height: usize,
}

/// Zero-based cursor position supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorPosition {
    pub row: usize,
    pub column: usize,
}
