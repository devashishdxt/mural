use super::{Span, Style, TextError, validate_structural_content};

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
}
