use super::{
    Line, Span, Style, TextError,
    ansi::{self, ParseMode},
};
use crate::Render;
use textwrap::Options;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    lines: Vec<Line>,
}

impl Text {
    pub fn empty() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn from_plain(content: impl AsRef<str>) -> Result<Self, TextError> {
        let lines = content
            .as_ref()
            .split('\n')
            .map(Line::from_plain)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { lines })
    }

    pub fn from_raw(content: impl AsRef<str>) -> Result<Self, TextError> {
        Ok(Self {
            lines: ansi::parse_text(content.as_ref(), ParseMode::Raw)?,
        })
    }

    pub fn from_ansi(content: impl AsRef<str>) -> Result<Self, TextError> {
        Ok(Self {
            lines: ansi::parse_text(content.as_ref(), ParseMode::Ansi)?,
        })
    }

    pub fn from_lines(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}

impl Render for Text {
    fn render(&self, width: u16) -> Text {
        if width == 0 {
            return Text::empty();
        }

        Text::from_lines(
            self.lines
                .iter()
                .flat_map(|line| wrap_line(line, usize::from(width)))
                .collect(),
        )
    }
}

fn wrap_line(line: &Line, width: usize) -> Vec<Line> {
    if line.spans().is_empty() {
        return vec![empty_line()];
    }

    let styled_source = styled_chars(line);
    let mut next_source_index = 0;
    let wrapped_fragments = wrap_plain_content(&line.plain_content(), width);

    if wrapped_fragments.is_empty() {
        return vec![empty_line()];
    }

    wrapped_fragments
        .into_iter()
        .map(|fragment| styled_fragment_line(&styled_source, &mut next_source_index, &fragment))
        .collect()
}

fn empty_line() -> Line {
    Line::from_spans(Vec::new())
}

fn wrap_plain_content(content: &str, width: usize) -> Vec<String> {
    let options = Options::new(width).break_words(true);
    textwrap::wrap(content, options)
        .into_iter()
        .map(|fragment| fragment.into_owned())
        .collect()
}

fn styled_chars(line: &Line) -> Vec<(char, Style)> {
    line.spans()
        .iter()
        .flat_map(|span| span.content().chars().map(move |ch| (ch, span.style())))
        .collect()
}

fn styled_fragment_line(
    styled_source: &[(char, Style)],
    next_source_index: &mut usize,
    fragment: &str,
) -> Line {
    let mut spans = Vec::new();
    let mut span_content = String::new();
    let mut span_style = None;

    for target_char in fragment.chars() {
        let Some(style) = next_matching_style(styled_source, next_source_index, target_char) else {
            continue;
        };

        if span_style != Some(style) {
            flush_span(&mut spans, &mut span_content, span_style);
            span_style = Some(style);
        }

        span_content.push(target_char);
    }

    flush_span(&mut spans, &mut span_content, span_style);
    Line::from_spans(spans)
}

fn next_matching_style(
    styled_source: &[(char, Style)],
    next_source_index: &mut usize,
    target_char: char,
) -> Option<Style> {
    while let Some(&(source_char, style)) = styled_source.get(*next_source_index) {
        *next_source_index += 1;
        if source_char == target_char {
            return Some(style);
        }
    }

    None
}

fn flush_span(spans: &mut Vec<Span>, content: &mut String, style: Option<Style>) {
    if content.is_empty() {
        return;
    }

    spans.push(
        Span::new(std::mem::take(content), style.unwrap_or_default())
            .expect("wrapped span content preserves invariants"),
    );
}
