use super::layout::{empty_lines, push_spaces};
use crate::{Line, Render, Text};

/// Creates a padding block around renderable content.
///
/// This is a convenience wrapper around [`Padding::new`].
///
/// # Examples
///
/// ```
/// # use brisk::{padding, Text, TextError};
/// # fn main() -> Result<(), TextError> {
/// let padded = padding(Text::from_plain("hello")?).left(2).right(1);
/// assert_eq!(padded.left_width(), 2);
/// # Ok(())
/// # }
/// ```
pub fn padding<T>(content: T) -> Padding<T> {
    Padding::new(content)
}

/// A layout block that adds padding around renderable content.
///
/// `Padding` renders its content inside the width left after horizontal padding.
/// Left padding is emitted as plain leading spaces on each content row. Right
/// padding is layout-only: it reduces the width available to the content but is
/// not emitted as trailing spaces. Top and bottom padding are emitted as empty
/// lines.
///
/// # Examples
///
/// ```
/// # use brisk::{Padding, Text, TextError};
/// # fn main() -> Result<(), TextError> {
/// let padded = Padding::new(Text::from_plain("hello")?)
///     .top(1)
///     .left(2)
///     .right(1)
///     .bottom(1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Padding<T = Text> {
    content: T,
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
}

impl<T> Padding<T> {
    /// Creates a padding block from renderable content.
    pub fn new(content: T) -> Self {
        Self {
            content,
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        }
    }

    /// Sets the top padding height in lines.
    pub fn top(mut self, top: usize) -> Self {
        self.top = top;
        self
    }

    /// Sets the bottom padding height in lines.
    pub fn bottom(mut self, bottom: usize) -> Self {
        self.bottom = bottom;
        self
    }

    /// Sets the left padding width in columns.
    pub fn left(mut self, left: usize) -> Self {
        self.left = left;
        self
    }

    /// Sets the right padding width in columns.
    pub fn right(mut self, right: usize) -> Self {
        self.right = right;
        self
    }

    /// Sets the top and bottom padding height in lines.
    pub fn vertical(mut self, vertical: usize) -> Self {
        self.top = vertical;
        self.bottom = vertical;
        self
    }

    /// Sets the left and right padding width in columns.
    pub fn horizontal(mut self, horizontal: usize) -> Self {
        self.left = horizontal;
        self.right = horizontal;
        self
    }

    /// Sets all padding sides to the same amount.
    pub fn all(mut self, padding: usize) -> Self {
        self.top = padding;
        self.bottom = padding;
        self.left = padding;
        self.right = padding;
        self
    }

    /// Returns this padding block's content.
    pub fn content(&self) -> &T {
        &self.content
    }

    /// Returns mutable access to this padding block's content.
    pub fn content_mut(&mut self) -> &mut T {
        &mut self.content
    }

    /// Returns the top padding height in lines.
    pub fn top_height(&self) -> usize {
        self.top
    }

    /// Returns the bottom padding height in lines.
    pub fn bottom_height(&self) -> usize {
        self.bottom
    }

    /// Returns the left padding width in columns.
    pub fn left_width(&self) -> usize {
        self.left
    }

    /// Returns the right padding width in columns.
    pub fn right_width(&self) -> usize {
        self.right
    }

    fn inner_width(&self, width: usize) -> usize {
        width.saturating_sub(self.left.saturating_add(self.right))
    }
}

impl<T: Render> Render for Padding<T> {
    fn render(&self, width: u16) -> Text {
        let width = usize::from(width);
        if width == 0 {
            return Text::empty();
        }

        let inner_width = self.inner_width(width);
        let content = self
            .content
            .render(inner_width as u16)
            .into_wrapped(inner_width);
        let content_lines = content.into_lines();
        let mut lines = Vec::with_capacity(self.top + content_lines.len() + self.bottom);

        lines.extend(empty_lines(self.top));
        lines.extend(
            content_lines
                .into_iter()
                .map(|line| self.pad_content_line(line, width)),
        );
        lines.extend(empty_lines(self.bottom));

        Text::from_lines(lines)
    }

    fn render_every_frame(&self) -> bool {
        self.content.render_every_frame()
    }
}

impl<T> Padding<T> {
    fn pad_content_line(&self, line: Line, width: usize) -> Line {
        let visible_left = self.left.min(width);
        if visible_left == 0 {
            return line;
        }

        let content_spans = line.into_spans();
        let mut spans = Vec::with_capacity(content_spans.len() + 1);
        push_spaces(&mut spans, visible_left);
        spans.extend(content_spans);
        Line::from_spans(spans)
    }
}
