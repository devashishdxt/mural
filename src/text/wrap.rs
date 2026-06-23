use textwrap::{
    Options,
    core::{Fragment, Word},
    word_splitters::split_words,
    wrap_algorithms::{Penalties, wrap_optimal_fit},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl SourceRange {
    pub(crate) fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub(crate) fn text(self, source: &str) -> &str {
        &source[self.start..self.end]
    }

    pub(crate) fn display_width_by(
        self,
        source: &str,
        display_width: impl Fn(&str) -> usize + Copy,
    ) -> usize {
        display_width(self.text(source))
    }

    pub(crate) fn contains(self, other: SourceRange) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    pub(crate) fn offset(self, amount: usize) -> Self {
        Self::new(self.start + amount, self.end + amount)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrappedFragment {
    pub(crate) word_range: SourceRange,
    pub(crate) whitespace_range: SourceRange,
    pub(crate) penalty: String,
}

impl WrappedFragment {
    fn offset(&self, amount: usize) -> Self {
        Self {
            word_range: self.word_range.offset(amount),
            whitespace_range: self.whitespace_range.offset(amount),
            penalty: self.penalty.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasuredFragment {
    fragment: WrappedFragment,
    width: usize,
    whitespace_width: usize,
    penalty_width: usize,
}

impl MeasuredFragment {
    fn hard_wrap_piece(
        source: &str,
        word_range: SourceRange,
        display_width: impl Fn(&str) -> usize + Copy,
    ) -> Self {
        Self {
            fragment: WrappedFragment {
                word_range,
                whitespace_range: SourceRange::new(word_range.end, word_range.end),
                penalty: String::new(),
            },
            width: word_range.display_width_by(source, display_width),
            whitespace_width: 0,
            penalty_width: 0,
        }
    }

    fn hard_wrap_tail(
        source: &str,
        word_range: SourceRange,
        original: &MeasuredFragment,
        display_width: impl Fn(&str) -> usize + Copy,
    ) -> Self {
        Self {
            fragment: WrappedFragment {
                word_range,
                whitespace_range: original.fragment.whitespace_range,
                penalty: original.fragment.penalty.clone(),
            },
            width: word_range.display_width_by(source, display_width),
            whitespace_width: original.whitespace_width,
            penalty_width: original.penalty_width,
        }
    }

    fn needs_hard_wrap(&self, width: usize) -> bool {
        self.width > width && !self.fragment.word_range.is_empty()
    }
}

impl Fragment for MeasuredFragment {
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

/// Wraps one source line into the same word-aware fragments used by Text.
pub(crate) fn wrap_source(source: &str, width: usize) -> Vec<Vec<WrappedFragment>> {
    wrap_source_by(source, width, UnicodeWidthStr::width)
}

/// Wraps one source line using a caller-provided display-width function.
pub(crate) fn wrap_source_by(
    source: &str,
    width: usize,
    display_width: impl Fn(&str) -> usize + Copy,
) -> Vec<Vec<WrappedFragment>> {
    let fragments = measured_fragments(source, width, display_width);
    let line_widths = [width as f64];

    wrap_optimal_fit(&fragments, &line_widths, &Penalties::new())
        .expect("styled text widths fit in f64 wrapping bounds")
        .into_iter()
        .map(|line| {
            line.iter()
                .map(|fragment| fragment.fragment.clone())
                .collect()
        })
        .collect()
}

pub(crate) fn offset_wrapped_lines(
    lines: Vec<Vec<WrappedFragment>>,
    amount: usize,
) -> Vec<Vec<WrappedFragment>> {
    lines
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|fragment| fragment.offset(amount))
                .collect()
        })
        .collect()
}

fn measured_fragments(
    source: &str,
    width: usize,
    display_width: impl Fn(&str) -> usize + Copy,
) -> Vec<MeasuredFragment> {
    let options = Options::new(width).break_words(false);
    let words = split_words(
        options.word_separator.find_words(source),
        &options.word_splitter,
    );
    let mut fragments = Vec::new();
    let mut search_start = 0;

    for word in words {
        let (fragment, next_search_start) =
            measured_fragment_from_word(source, search_start, word, display_width);
        search_start = next_search_start;
        fragments.extend(split_fragment_at_grapheme_boundaries(
            source,
            fragment,
            width,
            display_width,
        ));
    }

    fragments
}

fn measured_fragment_from_word(
    source: &str,
    search_start: usize,
    word: Word<'_>,
    display_width: impl Fn(&str) -> usize + Copy,
) -> (MeasuredFragment, usize) {
    let word_start = source[search_start..]
        .find(word.word)
        .map(|offset| search_start + offset)
        .unwrap_or(search_start);
    let word_range = SourceRange::new(word_start, word_start + word.word.len());
    let whitespace_range = SourceRange::new(word_range.end, word_range.end + word.whitespace.len());

    (
        MeasuredFragment {
            fragment: WrappedFragment {
                word_range,
                whitespace_range,
                penalty: word.penalty.to_owned(),
            },
            width: word_range.display_width_by(source, display_width),
            whitespace_width: whitespace_range.display_width_by(source, display_width),
            penalty_width: display_width(word.penalty),
        },
        whitespace_range.end,
    )
}

fn split_fragment_at_grapheme_boundaries(
    source: &str,
    fragment: MeasuredFragment,
    width: usize,
    display_width: impl Fn(&str) -> usize + Copy,
) -> Vec<MeasuredFragment> {
    if !fragment.needs_hard_wrap(width) {
        return vec![fragment];
    }

    let mut fragments = Vec::new();
    let word_range = fragment.fragment.word_range;
    let mut chunk_start = word_range.start;
    let mut chunk_width = 0;

    for (offset, grapheme) in word_range.text(source).grapheme_indices(true) {
        let grapheme_start = word_range.start + offset;
        let grapheme_end = grapheme_start + grapheme.len();
        let grapheme_width = display_width(grapheme);

        if chunk_width > 0 && grapheme_width > 0 && chunk_width + grapheme_width > width {
            fragments.push(MeasuredFragment::hard_wrap_piece(
                source,
                SourceRange::new(chunk_start, grapheme_start),
                display_width,
            ));
            chunk_start = grapheme_start;
            chunk_width = 0;
        }

        chunk_width += grapheme_width;
        if grapheme_end == word_range.end {
            fragments.push(MeasuredFragment::hard_wrap_tail(
                source,
                SourceRange::new(chunk_start, word_range.end),
                &fragment,
                display_width,
            ));
        }
    }

    fragments
}
