use super::{
    Line, Span, Style, TextError,
    ansi::{self, ParseMode},
};
use crate::Render;
use std::borrow::Cow;
use textwrap::{
    Options,
    core::{Fragment, Word},
    word_splitters::split_words,
    wrap_algorithms::{Penalties, wrap_optimal_fit},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRange {
    start: usize,
    end: usize,
}

impl SourceRange {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    fn is_empty(self) -> bool {
        self.start == self.end
    }

    fn text(self, source: &str) -> &str {
        &source[self.start..self.end]
    }

    fn display_width(self, source: &str) -> usize {
        UnicodeWidthStr::width(self.text(source))
    }

    fn contains(self, other: SourceRange) -> bool {
        other.start >= self.start && other.end <= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StyledFragment {
    word_range: SourceRange,
    whitespace_range: SourceRange,
    penalty: String,
    width: usize,
    whitespace_width: usize,
    penalty_width: usize,
}

impl StyledFragment {
    fn hard_wrap_piece(source: &str, word_range: SourceRange) -> Self {
        Self {
            word_range,
            whitespace_range: SourceRange::new(word_range.end, word_range.end),
            penalty: String::new(),
            width: word_range.display_width(source),
            whitespace_width: 0,
            penalty_width: 0,
        }
    }

    fn hard_wrap_tail(source: &str, word_range: SourceRange, original: &StyledFragment) -> Self {
        Self {
            word_range,
            whitespace_range: original.whitespace_range,
            penalty: original.penalty.clone(),
            width: word_range.display_width(source),
            whitespace_width: original.whitespace_width,
            penalty_width: original.penalty_width,
        }
    }

    fn needs_hard_wrap(&self, width: usize) -> bool {
        self.width > width && !self.word_range.is_empty()
    }
}

impl Fragment for StyledFragment {
    fn width(&self) -> f64 {
        self.width as f64
    }

    fn whitespace_width(&self) -> f64 {
        self.whitespace_width as f64
    }

    fn penalty_width(&self) -> f64 {
        self.penalty_width as f64
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
        for grapheme in styled_graphemes
            .iter()
            .filter(|grapheme| range.contains(grapheme.range))
        {
            self.append_text(grapheme.content, grapheme.style);
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

        self.spans
            .push(unsafe { Span::new_unchecked(std::mem::take(&mut self.content), self.style) });
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
    let fragments = styled_fragments(&source, width);
    let wrapped_fragments = wrap_fragments(&fragments, width);

    wrapped_fragments
        .into_iter()
        .map(|line_fragments| line_from_fragments(line_fragments, &styled_graphemes))
        .collect()
}

fn styled_fragments(source: &str, width: usize) -> Vec<StyledFragment> {
    let options = Options::new(width).break_words(false);
    let words = split_words(
        options.word_separator.find_words(source),
        &options.word_splitter,
    );
    let mut fragments = Vec::new();
    let mut search_start = 0;

    for word in words {
        let (fragment, next_search_start) = styled_fragment_from_word(source, search_start, word);
        search_start = next_search_start;
        fragments.extend(split_fragment_at_grapheme_boundaries(
            source, fragment, width,
        ));
    }

    fragments
}

fn styled_fragment_from_word(
    source: &str,
    search_start: usize,
    word: Word<'_>,
) -> (StyledFragment, usize) {
    let word_start = source[search_start..]
        .find(word.word)
        .map(|offset| search_start + offset)
        .unwrap_or(search_start);
    let word_range = SourceRange::new(word_start, word_start + word.word.len());
    let whitespace_range = SourceRange::new(word_range.end, word_range.end + word.whitespace.len());

    (
        StyledFragment {
            word_range,
            whitespace_range,
            penalty: word.penalty.to_owned(),
            width: word_range.display_width(source),
            whitespace_width: whitespace_range.display_width(source),
            penalty_width: UnicodeWidthStr::width(word.penalty),
        },
        whitespace_range.end,
    )
}

fn split_fragment_at_grapheme_boundaries(
    source: &str,
    fragment: StyledFragment,
    width: usize,
) -> Vec<StyledFragment> {
    if !fragment.needs_hard_wrap(width) {
        return vec![fragment];
    }

    let mut fragments = Vec::new();
    let word_range = fragment.word_range;
    let mut chunk_start = word_range.start;
    let mut chunk_width = 0;

    for (offset, grapheme) in word_range.text(source).grapheme_indices(true) {
        let grapheme_start = word_range.start + offset;
        let grapheme_end = grapheme_start + grapheme.len();
        let grapheme_width = UnicodeWidthStr::width(grapheme);

        if chunk_width > 0 && grapheme_width > 0 && chunk_width + grapheme_width > width {
            fragments.push(StyledFragment::hard_wrap_piece(
                source,
                SourceRange::new(chunk_start, grapheme_start),
            ));
            chunk_start = grapheme_start;
            chunk_width = 0;
        }

        chunk_width += grapheme_width;
        if grapheme_end == word_range.end {
            fragments.push(StyledFragment::hard_wrap_tail(
                source,
                SourceRange::new(chunk_start, word_range.end),
                &fragment,
            ));
        }
    }

    fragments
}

fn wrap_fragments(fragments: &[StyledFragment], width: usize) -> Vec<&[StyledFragment]> {
    let line_widths = [width as f64];
    wrap_optimal_fit(fragments, &line_widths, &Penalties::new())
        .expect("styled text widths fit in f64 wrapping bounds")
}

fn line_from_fragments(
    fragments: &[StyledFragment],
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
    fragment: &StyledFragment,
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
