use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

use crate::Render;

use super::{
    Line, Span, Style, TextError,
    ansi::{self, ParseMode},
    wrap::{self, SourceRange, WrappedFragment},
};

/// Multi-line styled text rendered by terminal blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    lines: Vec<Line>,
}

impl Text {
    /// Creates text with no lines.
    pub fn empty() -> Self {
        Self { lines: Vec::new() }
    }

    /// Creates plain text from newline-separated content.
    ///
    /// Each line rejects tabs, carriage returns, ANSI escapes, and unsupported
    /// control characters.
    pub fn from_plain(content: impl AsRef<str>) -> Result<Self, TextError> {
        let lines = content
            .as_ref()
            .split('\n')
            .map(Line::from_plain)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { lines })
    }

    /// Creates plain text from raw terminal text by lossily sanitizing unsafe content.
    ///
    /// ANSI escape sequences and unsupported control characters are stripped,
    /// tabs are replaced with spaces, and newline combinations are normalized.
    pub fn from_raw_lossy(content: impl AsRef<str>) -> Result<Self, TextError> {
        Ok(Self {
            lines: ansi::parse_text(content.as_ref(), ParseMode::Raw)?,
        })
    }

    /// Creates styled text by parsing ANSI SGR escape sequences.
    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        Ok(Self {
            lines: ansi::parse_text(content.as_ref(), ParseMode::Ansi)?,
        })
    }

    /// Creates text from already-validated lines.
    pub fn from_lines(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    /// Returns this text's lines.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub(crate) fn into_lines(self) -> Vec<Line> {
        self.lines
    }

    /// Returns the maximum display width of any line.
    pub fn display_width(&self) -> usize {
        self.lines
            .iter()
            .map(Line::display_width)
            .max()
            .unwrap_or(0)
    }

    /// Returns the number of display lines.
    pub fn display_height(&self) -> usize {
        self.lines.len()
    }

    /// Wraps text to `width` columns, borrowing when no wrapping is needed.
    pub fn wrap(&self, width: usize) -> Cow<'_, Text> {
        if width == 0 {
            return if self.lines.is_empty() {
                Cow::Borrowed(self)
            } else {
                Cow::Owned(Text::empty())
            };
        }

        if self.fits_within(width) {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(self.wrapped(width))
        }
    }

    /// Wraps text to `width` columns, consuming the original value when possible.
    pub fn into_wrapped(self, width: usize) -> Text {
        if width == 0 {
            return Text::empty();
        }

        if self.fits_within(width) {
            self
        } else {
            self.wrapped(width)
        }
    }

    fn fits_within(&self, width: usize) -> bool {
        self.lines.iter().all(|line| line.display_width() <= width)
    }

    fn wrapped(&self, width: usize) -> Text {
        Text::from_lines(
            self.lines
                .iter()
                .flat_map(|line| wrap_line(line, width))
                .collect(),
        )
    }
}

impl Render for Text {
    fn render(&self, width: u16) -> Text {
        self.wrap(usize::from(width)).into_owned()
    }
}

#[derive(Debug)]
struct StyledGrapheme<'a> {
    range: SourceRange,
    content: &'a str,
    style: Style,
}

#[derive(Default)]
struct StyledLineBuilder {
    spans: Vec<Span>,
    content: String,
    style: Style,
}

impl StyledLineBuilder {
    fn append_range(&mut self, range: SourceRange, styled_graphemes: &[StyledGrapheme<'_>]) {
        for grapheme in styled_graphemes {
            if grapheme.range.end <= range.start {
                continue;
            }
            if grapheme.range.start >= range.end {
                break;
            }
            if range.contains(grapheme.range) {
                self.append_text(grapheme.content, grapheme.style);
            }
        }
    }

    fn append_text(&mut self, text: &str, style: Style) {
        if !self.content.is_empty() && self.style != style {
            self.flush_span();
        }
        self.style = style;
        self.content.push_str(text);
    }

    fn finish(mut self) -> Line {
        self.flush_span();
        Line::from_spans(self.spans)
    }

    fn flush_span(&mut self) {
        if self.content.is_empty() {
            return;
        }

        self.spans.push(Span::from_trusted_content(
            std::mem::take(&mut self.content),
            self.style,
        ));
    }
}

fn empty_line() -> Line {
    Line::from_spans(Vec::new())
}

fn wrap_line(line: &Line, width: usize) -> Vec<Line> {
    if line.spans().is_empty() {
        return vec![empty_line()];
    }

    let source = line.plain_content();
    let styled_graphemes = styled_graphemes(line);
    wrap::wrap_source(&source, width)
        .into_iter()
        .map(|line_fragments| line_from_fragments(&line_fragments, &styled_graphemes))
        .collect()
}

fn line_from_fragments(
    fragments: &[WrappedFragment],
    styled_graphemes: &[StyledGrapheme<'_>],
) -> Line {
    let Some((last, leading)) = fragments.split_last() else {
        return empty_line();
    };

    let mut line = StyledLineBuilder::default();
    for fragment in leading {
        line.append_range(fragment.word_range, styled_graphemes);
        line.append_range(fragment.whitespace_range, styled_graphemes);
    }

    line.append_range(last.word_range, styled_graphemes);
    if !last.penalty.is_empty() {
        line.append_text(
            &last.penalty,
            trailing_fragment_style(last, styled_graphemes),
        );
    }

    line.finish()
}

fn trailing_fragment_style(
    fragment: &WrappedFragment,
    styled_graphemes: &[StyledGrapheme<'_>],
) -> Style {
    styled_graphemes
        .iter()
        .rev()
        .find(|grapheme| fragment.word_range.contains(grapheme.range))
        .map(|grapheme| grapheme.style)
        .unwrap_or_default()
}

fn styled_graphemes(line: &Line) -> Vec<StyledGrapheme<'_>> {
    let mut graphemes = Vec::new();
    let mut source_offset = 0;

    for span in line.spans() {
        for (span_offset, grapheme) in span.content().grapheme_indices(true) {
            let start = source_offset + span_offset;
            graphemes.push(StyledGrapheme {
                range: SourceRange::new(start, start + grapheme.len()),
                content: grapheme,
                style: span.style(),
            });
        }
        source_offset += span.content().len();
    }

    graphemes
}
