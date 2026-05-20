use super::{
    Span, TextError,
    ansi::{self, ParseMode},
};

/// One terminal display line made of styled spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    spans: Vec<Span>,
}

impl Line {
    /// Creates an unstyled line from plain content.
    pub fn from_plain(content: impl Into<String>) -> Result<Self, TextError> {
        let content = content.into();
        let spans = if content.is_empty() {
            Vec::new()
        } else {
            vec![Span::plain(content)?]
        };
        Ok(Self { spans })
    }

    /// Creates a line from raw text while preserving literal ANSI bytes.
    pub fn from_raw(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_line(ansi::parse_text(content.as_ref(), ParseMode::Raw)?)
    }

    /// Creates a styled line by parsing ANSI SGR escape sequences.
    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        single_line(ansi::parse_text(content.as_ref(), ParseMode::Ansi)?)
    }

    /// Creates a line from already-validated spans.
    pub fn from_spans(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    /// Returns this line's spans.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub(super) fn into_spans(self) -> Vec<Span> {
        self.spans
    }

    /// Returns this line's text content without style information.
    pub fn plain_content(&self) -> String {
        self.spans.iter().map(Span::content).collect()
    }

    /// Returns the Unicode display width of this line.
    pub fn display_width(&self) -> usize {
        self.spans.iter().map(Span::display_width).sum()
    }
}

fn single_line(lines: Vec<Line>) -> Result<Line, TextError> {
    let [line] = lines.try_into().map_err(|_| TextError::MultipleLines)?;
    Ok(line)
}
