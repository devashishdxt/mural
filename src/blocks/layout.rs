use crate::{Line, Span, Style};

pub(super) fn empty_lines(count: usize) -> impl Iterator<Item = Line> {
    std::iter::repeat_with(|| Line::from_spans(Vec::new())).take(count)
}

pub(super) fn push_spaces(spans: &mut Vec<Span>, width: usize) {
    if width == 0 {
        return;
    }

    spans.push(Span::from_trusted_content(" ".repeat(width), Style::new()));
}
