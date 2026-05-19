use super::{Style, TextError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    content: String,
    style: Style,
}

impl Span {
    pub fn new(content: impl Into<String>, style: Style) -> Result<Self, TextError> {
        let content = content.into();
        validate_structural_content(&content)?;
        Ok(Self { content, style })
    }

    pub fn plain(content: impl Into<String>) -> Result<Self, TextError> {
        Self::new(content, Style::new())
    }

    /// Creates a span without validating content invariants.
    ///
    /// # Safety
    /// Callers must ensure the content does not contain structural terminal
    /// content such as newlines, tabs, carriage returns, ANSI escapes, or
    /// unsupported control characters unless they intentionally accept the
    /// resulting renderer behavior.
    pub unsafe fn new_unchecked(content: impl Into<String>, style: Style) -> Self {
        Self {
            content: content.into(),
            style,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn style(&self) -> Style {
        self.style
    }
}

pub(crate) fn validate_structural_content(content: &str) -> Result<(), TextError> {
    if content
        .chars()
        .any(|ch| ch == '\n' || ch == '\r' || ch == '\t' || ch == '\x1b' || ch.is_control())
    {
        return Err(TextError::structural_content());
    }

    Ok(())
}
