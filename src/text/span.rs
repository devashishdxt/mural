use super::{
    Line, Style, TextError,
    ansi::{self, ParseMode},
};
use unicode_width::UnicodeWidthStr;

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

    pub fn from_raw(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_span(ansi::parse_text(content.as_ref(), ParseMode::Raw)?)
    }

    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_span(ansi::parse_text(content.as_ref(), ParseMode::Ansi)?)
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

    pub fn display_width(&self) -> usize {
        UnicodeWidthStr::width(self.content.as_str())
    }
}

fn single_span(mut lines: Vec<Line>) -> Result<Span, TextError> {
    if lines.len() != 1 {
        return Err(TextError::multiple_lines());
    }

    let mut spans = lines.remove(0).spans().to_vec();
    match spans.len() {
        0 => Ok(Span::new("", Style::new())?),
        1 => Ok(spans.remove(0)),
        _ => Err(TextError::multiple_styles()),
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
