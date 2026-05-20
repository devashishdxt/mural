use thiserror::Error;

/// Errors produced when constructing validated text values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TextError {
    /// Plain text contained newlines at the wrong level, tabs, carriage returns,
    /// ANSI escapes, or unsupported control characters.
    #[error("text contains unsupported structural content")]
    StructuralContent,
    /// A constructor that requires one line received multiple lines.
    #[error("text constructor expected a single line")]
    MultipleLines,
    /// A constructor that requires one style received multiple styles.
    #[error("text constructor expected a single style")]
    MultipleStyles,
}
