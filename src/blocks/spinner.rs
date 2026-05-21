use super::{layout::push_spaces, validation::validate_non_empty_display_text};
use crate::{Line, Render, Span, Style, Text, TextError};
use std::cell::Cell;
use unicode_width::UnicodeWidthStr;

const DEFAULT_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const DEFAULT_SUCCESS_MARKER: &str = "✓";
const DEFAULT_FAILURE_MARKER: &str = "✗";
const DEFAULT_GAP: usize = 1;

/// Creates a default spinner block from plain text content.
///
/// This is a convenience wrapper around [`Spinner::new`] and
/// [`Text::from_plain`](crate::Text::from_plain).
///
/// # Examples
///
/// ```
/// # use brisk::{spinner, TextError};
/// # fn main() -> Result<(), TextError> {
/// let item = spinner("loading")?;
/// assert!(item.is_running());
/// # Ok(())
/// # }
/// ```
pub fn spinner(content: impl AsRef<str>) -> Result<Spinner<Text>, TextError> {
    Ok(Spinner::new(Text::from_plain(content)?))
}

/// A running spinner or terminal success/failure marker with a hanging indent.
///
/// `Spinner` renders a spinner frame before the first content line and indents
/// every wrapped or explicit continuation line to the first text column. While
/// running, it advances by one frame every time it is rendered and opts into
/// every-frame rendering. After [`Spinner::succeed`] or [`Spinner::fail`], it
/// renders a fixed marker and no longer requests every-frame rendering.
///
/// Its content can be any [`Render`] value; the default content type is [`Text`].
///
/// # Examples
///
/// ```
/// # use brisk::{Spinner, Style, Text, TextError};
/// # fn main() -> Result<(), TextError> {
/// let item = Spinner::new(Text::from_plain("loading")?)
///     .spinner_style(Style::new().dim())
///     .gap(1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Spinner<T = Text> {
    content: T,
    frames: Vec<String>,
    frame_width: usize,
    spinner_style: Style,
    success_marker: String,
    success_style: Style,
    failure_marker: String,
    failure_style: Style,
    gap: usize,
    state: SpinnerState,
    next_frame: Cell<usize>,
}

impl<T: PartialEq> PartialEq for Spinner<T> {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
            && self.frames == other.frames
            && self.frame_width == other.frame_width
            && self.spinner_style == other.spinner_style
            && self.success_marker == other.success_marker
            && self.success_style == other.success_style
            && self.failure_marker == other.failure_marker
            && self.failure_style == other.failure_style
            && self.gap == other.gap
            && self.state == other.state
            && self.next_frame.get() == other.next_frame.get()
    }
}

impl<T: Eq> Eq for Spinner<T> {}

impl<T> Spinner<T> {
    /// Creates a running spinner from renderable content.
    pub fn new(content: T) -> Self {
        Self {
            content,
            frames: DEFAULT_FRAMES
                .iter()
                .map(|frame| (*frame).to_owned())
                .collect(),
            frame_width: UnicodeWidthStr::width(DEFAULT_FRAMES[0]),
            spinner_style: Style::new(),
            success_marker: DEFAULT_SUCCESS_MARKER.to_owned(),
            success_style: Style::new(),
            failure_marker: DEFAULT_FAILURE_MARKER.to_owned(),
            failure_style: Style::new(),
            gap: DEFAULT_GAP,
            state: SpinnerState::Running,
            next_frame: Cell::new(0),
        }
    }

    /// Sets the running spinner frames.
    ///
    /// Frames must be non-empty, have non-zero terminal display width, and
    /// contain no structural terminal content. Frames may have different
    /// display widths; the spinner reserves the widest marker column so content
    /// stays aligned.
    pub fn frames<I, S>(mut self, frames: I) -> Result<Self, TextError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let frames = validate_frames(frames)?;

        let next_frame = self.next_frame.get() % frames.len();
        self.frames = frames;
        self.recalculate_frame_width();
        self.next_frame.set(next_frame);
        Ok(self)
    }

    /// Sets the style applied to running spinner frames only.
    pub fn spinner_style(mut self, style: Style) -> Self {
        self.spinner_style = style;
        self
    }

    /// Sets the success marker rendered after [`Spinner::succeed`].
    ///
    /// The marker must be valid non-structural terminal content with non-zero
    /// display width.
    pub fn success_marker(mut self, marker: impl Into<String>) -> Result<Self, TextError> {
        let marker = marker.into();
        validate_symbol(&marker)?;
        self.success_marker = marker;
        self.recalculate_frame_width();
        Ok(self)
    }

    /// Sets the style applied to the success marker only.
    pub fn success_style(mut self, style: Style) -> Self {
        self.success_style = style;
        self
    }

    /// Sets the failure marker rendered after [`Spinner::fail`].
    ///
    /// The marker must be valid non-structural terminal content with non-zero
    /// display width.
    pub fn failure_marker(mut self, marker: impl Into<String>) -> Result<Self, TextError> {
        let marker = marker.into();
        validate_symbol(&marker)?;
        self.failure_marker = marker;
        self.recalculate_frame_width();
        Ok(self)
    }

    /// Sets the style applied to the failure marker only.
    pub fn failure_style(mut self, style: Style) -> Self {
        self.failure_style = style;
        self
    }

    /// Sets the number of plain-space columns between the marker and content.
    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
        self
    }

    /// Marks the spinner as succeeded and stops animation.
    pub fn succeed(&mut self) -> &mut Self {
        self.state = SpinnerState::Success;
        self
    }

    /// Marks the spinner as failed and stops animation.
    pub fn fail(&mut self) -> &mut Self {
        self.state = SpinnerState::Failure;
        self
    }

    /// Returns the spinner to the running state without resetting the next frame.
    pub fn reset(&mut self) -> &mut Self {
        self.state = SpinnerState::Running;
        self
    }

    /// Returns this spinner's content.
    pub fn content(&self) -> &T {
        &self.content
    }

    /// Returns mutable access to this spinner's content.
    pub fn content_mut(&mut self) -> &mut T {
        &mut self.content
    }

    /// Returns the running spinner frame contents.
    pub fn frame_contents(&self) -> &[String] {
        &self.frames
    }

    /// Returns the index of the running spinner frame that will render next.
    pub fn current_frame_index(&self) -> usize {
        self.next_frame.get()
    }

    /// Returns the style applied to running spinner frames.
    pub fn spinner_style_value(&self) -> Style {
        self.spinner_style
    }

    /// Returns the success marker content.
    pub fn success_marker_content(&self) -> &str {
        &self.success_marker
    }

    /// Returns the style applied to the success marker.
    pub fn success_style_value(&self) -> Style {
        self.success_style
    }

    /// Returns the failure marker content.
    pub fn failure_marker_content(&self) -> &str {
        &self.failure_marker
    }

    /// Returns the style applied to the failure marker.
    pub fn failure_style_value(&self) -> Style {
        self.failure_style
    }

    /// Returns the number of plain-space columns between the marker and content.
    pub fn gap_width(&self) -> usize {
        self.gap
    }

    /// Returns the reserved terminal display width for the marker column.
    ///
    /// This is the maximum width of all running frames plus success and failure
    /// markers.
    pub fn frame_width(&self) -> usize {
        self.frame_width
    }

    /// Reports whether this spinner is currently running.
    pub fn is_running(&self) -> bool {
        self.state == SpinnerState::Running
    }

    /// Reports whether this spinner has succeeded.
    pub fn is_success(&self) -> bool {
        self.state == SpinnerState::Success
    }

    /// Reports whether this spinner has failed.
    pub fn is_failure(&self) -> bool {
        self.state == SpinnerState::Failure
    }

    fn prefix_width(&self) -> usize {
        self.frame_width.saturating_add(self.gap)
    }

    fn recalculate_frame_width(&mut self) {
        self.frame_width =
            max_marker_width(&self.frames, &self.success_marker, &self.failure_marker);
    }

    fn next_marker(&self) -> Marker {
        let (content, style) = match self.state {
            SpinnerState::Running => {
                let index = self.next_frame.get();
                self.next_frame.set((index + 1) % self.frames.len());
                (self.frames[index].clone(), self.spinner_style)
            }
            SpinnerState::Success => (self.success_marker.clone(), self.success_style),
            SpinnerState::Failure => (self.failure_marker.clone(), self.failure_style),
        };

        let width = UnicodeWidthStr::width(content.as_str());
        // `Spinner` validates marker content before storing it, and default
        // markers are static non-structural text.
        Marker {
            span: Span::from_trusted_content(content, style),
            width,
        }
    }

    fn push_marker_prefix(&self, spans: &mut Vec<Span>, marker: Marker, gap: usize) {
        let alignment_width = self.frame_width.saturating_sub(marker.width);
        spans.push(marker.span);
        push_spaces(spans, alignment_width);
        push_spaces(spans, gap);
    }

    fn first_prefix_line(&self, marker: Marker, fitting_gap: usize) -> Line {
        let mut spans = Vec::with_capacity(3);
        self.push_marker_prefix(&mut spans, marker, fitting_gap);
        Line::from_spans(spans)
    }

    fn first_line(&self, marker: Marker, content: Line) -> Line {
        let content_spans = content.into_spans();
        let mut spans = Vec::with_capacity(content_spans.len() + 3);
        self.push_marker_prefix(&mut spans, marker, self.gap);
        spans.extend(content_spans);
        Line::from_spans(spans)
    }

    fn continuation_line(&self, content: Line) -> Line {
        let content_spans = content.into_spans();
        let mut spans = Vec::with_capacity(content_spans.len() + 1);
        push_spaces(&mut spans, self.prefix_width());
        spans.extend(content_spans);
        Line::from_spans(spans)
    }
}

impl<T: Render> Render for Spinner<T> {
    fn render(&self, width: u16) -> Text {
        let marker = self.next_marker();
        let width = usize::from(width);
        if width == 0 || width < self.frame_width {
            return Text::empty();
        }

        let prefix_width = self.prefix_width();
        if width <= prefix_width {
            let content = self.content.render(1);
            if content.lines().is_empty() {
                Text::empty()
            } else {
                Text::from_lines(vec![
                    self.first_prefix_line(marker, width - self.frame_width),
                ])
            }
        } else {
            let content_width = width - prefix_width;
            let content = self
                .content
                .render(content_width as u16)
                .into_wrapped(content_width);
            let content_lines = content.into_lines();
            if content_lines.is_empty() {
                return Text::empty();
            }

            let mut lines = Vec::with_capacity(content_lines.len());
            for (index, line) in content_lines.into_iter().enumerate() {
                if index == 0 {
                    lines.push(self.first_line(marker.clone(), line));
                } else {
                    lines.push(self.continuation_line(line));
                }
            }

            Text::from_lines(lines)
        }
    }

    fn render_every_frame(&self) -> bool {
        self.state == SpinnerState::Running
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpinnerState {
    Running,
    Success,
    Failure,
}

#[derive(Debug, Clone)]
struct Marker {
    span: Span,
    width: usize,
}

fn validate_frames<I, S>(frames: I) -> Result<Vec<String>, TextError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let frames = frames.into_iter().map(Into::into).collect::<Vec<_>>();
    if frames.is_empty() {
        return Err(TextError::StructuralContent);
    }

    for frame in &frames {
        validate_symbol(frame)?;
    }

    Ok(frames)
}

fn validate_symbol(symbol: &str) -> Result<usize, TextError> {
    validate_non_empty_display_text(symbol)
}

fn max_marker_width(frames: &[String], success_marker: &str, failure_marker: &str) -> usize {
    frames
        .iter()
        .map(|frame| UnicodeWidthStr::width(frame.as_str()))
        .chain([
            UnicodeWidthStr::width(success_marker),
            UnicodeWidthStr::width(failure_marker),
        ])
        .max()
        .unwrap_or(0)
}
