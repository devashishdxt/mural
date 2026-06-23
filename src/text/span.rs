use unicode_width::UnicodeWidthStr;

use super::{
    Line, Style, TextError,
    ansi::{self, ParseMode},
};

/// A contiguous run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    content: String,
    style: Style,
}

impl Span {
    /// Creates a span from plain content and a style.
    pub fn new(content: impl Into<String>, style: Style) -> Result<Self, TextError> {
        let content = content.into();
        Self::validate_content(&content)?;
        Ok(Self { content, style })
    }

    /// Creates an unstyled span from plain content.
    pub fn plain(content: impl Into<String>) -> Result<Self, TextError> {
        Self::new(content, Style::new())
    }

    /// Creates a plain span from raw terminal text by lossily sanitizing unsafe content.
    ///
    /// ANSI escape sequences and unsupported control characters are stripped,
    /// and tabs are replaced with spaces.
    pub fn from_raw_lossy(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_span(ansi::parse_text(content.as_ref(), ParseMode::Raw)?)
    }

    /// Creates a styled span by parsing ANSI SGR escape sequences.
    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_span(ansi::parse_text(content.as_ref(), ParseMode::Ansi)?)
    }

    pub(crate) fn validate_content(content: &str) -> Result<(), TextError> {
        if content
            .chars()
            .any(|ch| ch == '\n' || ch == '\r' || ch == '\t' || ch == '\x1b' || ch.is_control())
        {
            return Err(TextError::StructuralContent);
        }

        Ok(())
    }

    /// Creates a span from content that has already been validated by an owning type.
    ///
    /// This keeps invariant-preserving constructors terse without relying on
    /// unchecked undefined behavior if a future caller breaks the invariant.
    pub(crate) fn from_trusted_content(content: impl Into<String>, style: Style) -> Self {
        Self::new(content, style)
            .expect("trusted span content must not contain structural terminal content")
    }

    /// Returns this span's text content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns this span's style.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Returns the Unicode display width of this span.
    pub fn display_width(&self) -> usize {
        UnicodeWidthStr::width(self.content.as_str())
    }
}

fn single_span(lines: Vec<Line>) -> Result<Span, TextError> {
    let [line] = lines.try_into().map_err(|_| TextError::MultipleLines)?;
    let mut spans = line.into_spans().into_iter();

    let Some(span) = spans.next() else {
        return Span::plain("");
    };

    if spans.next().is_some() {
        return Err(TextError::MultipleStyles);
    }

    Ok(span)
}
