use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TextError {
    #[error("text contains unsupported structural content")]
    StructuralContent,
    #[error("text constructor expected a single line")]
    MultipleLines,
    #[error("text constructor expected a single style")]
    MultipleStyles,
}

impl TextError {
    pub(crate) fn structural_content() -> Self {
        Self::StructuralContent
    }

    pub(crate) fn multiple_lines() -> Self {
        Self::MultipleLines
    }

    pub(crate) fn multiple_styles() -> Self {
        Self::MultipleStyles
    }
}
