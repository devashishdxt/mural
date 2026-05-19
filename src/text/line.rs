use super::{
    Span, Style, TextError,
    ansi::{self, ParseMode},
    validate_structural_content,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    spans: Vec<Span>,
}

impl Line {
    pub fn from_plain(content: impl Into<String>) -> Result<Self, TextError> {
        let content = content.into();
        validate_structural_content(&content)?;
        let spans = if content.is_empty() {
            Vec::new()
        } else {
            vec![Span::new(content, Style::new())?]
        };
        Ok(Self { spans })
    }

    pub fn from_raw(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_line(ansi::parse_text(content.as_ref(), ParseMode::Raw)?)
    }

    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_line(ansi::parse_text(content.as_ref(), ParseMode::Ansi)?)
    }

    pub fn from_spans(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    /// Creates a line without validating span content invariants.
    ///
    /// # Safety
    /// Callers must ensure spans do not contain structural content unless they
    /// intentionally accept the resulting renderer behavior.
    pub unsafe fn new_unchecked(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn plain_content(&self) -> String {
        self.spans.iter().map(Span::content).collect()
    }

    pub fn display_width(&self) -> usize {
        self.spans.iter().map(Span::display_width).sum()
    }

    pub fn display_height(&self) -> usize {
        1
    }
}

fn single_line(mut lines: Vec<Line>) -> Result<Line, TextError> {
    if lines.len() != 1 {
        return Err(TextError::multiple_lines());
    }
    Ok(lines.remove(0))
}
