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
        return vec![Line::from_spans(Vec::new())];
    }

    let source = styled_chars(line);
    let mut source_index = 0;
    let options = Options::new(width).break_words(true);
    let plain_content = line.plain_content();
    let wrapped = textwrap::wrap(&plain_content, options);

    if wrapped.is_empty() {
        return vec![Line::from_spans(Vec::new())];
    }

    wrapped
        .into_iter()
        .map(|fragment| styled_fragment_line(&source, &mut source_index, &fragment))
        .collect()
}

fn styled_chars(line: &Line) -> Vec<(char, Style)> {
    line.spans()
        .iter()
        .flat_map(|span| span.content().chars().map(move |ch| (ch, span.style())))
        .collect()
}

fn styled_fragment_line(
    source: &[(char, Style)],
    source_index: &mut usize,
    fragment: &str,
) -> Line {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style = None;

    for target in fragment.chars() {
        let Some((_, style)) = next_matching_char(source, source_index, target) else {
            continue;
        };

        if current_style != Some(style) {
            flush_span(&mut spans, &mut current_text, current_style);
            current_style = Some(style);
        }

        current_text.push(target);
    }

    flush_span(&mut spans, &mut current_text, current_style);
    Line::from_spans(spans)
}

fn next_matching_char(
    source: &[(char, Style)],
    source_index: &mut usize,
    target: char,
) -> Option<(char, Style)> {
    while let Some(&(ch, style)) = source.get(*source_index) {
        *source_index += 1;
        if ch == target {
            return Some((ch, style));
        }
    }

    None
}

fn flush_span(spans: &mut Vec<Span>, text: &mut String, style: Option<Style>) {
    if text.is_empty() {
        return;
    }

    spans.push(
        Span::new(std::mem::take(text), style.unwrap_or_default())
            .expect("wrapped span content preserves invariants"),
    );
}
