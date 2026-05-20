use crate::{Line, Render, Span, Style, Text, TextError};
use unicode_width::UnicodeWidthStr;

const DEFAULT_BULLET: &str = "•";
const DEFAULT_GAP: usize = 1;

/// Creates a default list item block from plain text content.
///
/// This is a convenience wrapper around [`ListItem::new`] and
/// [`Text::from_plain`](crate::Text::from_plain).
///
/// # Examples
///
/// ```
/// # use brisk::{list_item, TextError};
/// # fn main() -> Result<(), TextError> {
/// let item = list_item("hello")?;
/// assert_eq!(item.bullet_content(), "•");
/// # Ok(())
/// # }
/// ```
pub fn list_item(content: impl AsRef<str>) -> Result<ListItem<Text>, TextError> {
    Ok(ListItem::new(Text::from_plain(content)?))
}

/// A single unordered-list item with a hanging indent.
///
/// `ListItem` renders a bullet before the first content line and indents every
/// wrapped or explicit continuation line to the first text column. Its content
/// can be any [`Render`] value; the default content type is [`Text`].
///
/// # Examples
///
/// ```
/// # use brisk::{ListItem, Style, Text, TextError};
/// # fn main() -> Result<(), TextError> {
/// let item = ListItem::new(Text::from_plain("hello world")?)
///     .bullet("-")?
///     .bullet_style(Style::new().dim())
///     .gap(1);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem<T = Text> {
    content: T,
    bullet: String,
    bullet_width: usize,
    bullet_style: Style,
    gap: usize,
}

impl<T> ListItem<T> {
    /// Creates a list item from renderable content.
    pub fn new(content: T) -> Self {
        Self {
            content,
            bullet: DEFAULT_BULLET.to_owned(),
            bullet_width: UnicodeWidthStr::width(DEFAULT_BULLET),
            bullet_style: Style::new(),
            gap: DEFAULT_GAP,
        }
    }

    /// Sets the bullet content.
    ///
    /// The bullet must be non-empty, have non-zero terminal display width, and
    /// contain no structural terminal content such as newlines, tabs, ANSI
    /// escapes, or other control characters.
    pub fn bullet(mut self, bullet: impl Into<String>) -> Result<Self, TextError> {
        let bullet = bullet.into();
        self.bullet_width = validate_bullet(&bullet, self.bullet_style)?;
        self.bullet = bullet;
        Ok(self)
    }

    /// Sets the style applied to the bullet only.
    pub fn bullet_style(mut self, style: Style) -> Self {
        self.bullet_style = style;
        self
    }

    /// Sets the number of plain-space columns between the bullet and content.
    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
        self
    }

    /// Returns this list item's content.
    pub fn content(&self) -> &T {
        &self.content
    }

    /// Returns mutable access to this list item's content.
    pub fn content_mut(&mut self) -> &mut T {
        &mut self.content
    }

    /// Returns the bullet content.
    pub fn bullet_content(&self) -> &str {
        &self.bullet
    }

    /// Returns the style applied to the bullet.
    pub fn bullet_style_value(&self) -> Style {
        self.bullet_style
    }

    /// Returns the number of plain-space columns between the bullet and content.
    pub fn gap_width(&self) -> usize {
        self.gap
    }

    /// Returns the terminal display width of the bullet.
    pub fn bullet_width(&self) -> usize {
        self.bullet_width
    }

    fn prefix_width(&self) -> usize {
        self.bullet_width.saturating_add(self.gap)
    }

    fn first_prefix_line(&self, fitting_gap: usize) -> Line {
        let mut spans = vec![self.bullet_span()];
        push_spaces(&mut spans, fitting_gap);
        Line::from_spans(spans)
    }

    fn first_line(&self, content: &Line) -> Line {
        let mut spans = vec![self.bullet_span()];
        push_spaces(&mut spans, self.gap);
        spans.extend_from_slice(content.spans());
        Line::from_spans(spans)
    }

    fn continuation_line(&self, content: &Line) -> Line {
        let mut spans = Vec::new();
        push_spaces(&mut spans, self.prefix_width());
        spans.extend_from_slice(content.spans());
        Line::from_spans(spans)
    }

    fn bullet_span(&self) -> Span {
        // SAFETY: `ListItem` validates bullet content before storing it, and the
        // default bullet is static non-structural text.
        unsafe { Span::new_unchecked(self.bullet.clone(), self.bullet_style) }
    }
}

impl<T: Render> Render for ListItem<T> {
    fn render(&self, width: u16) -> Text {
        let width = usize::from(width);
        if width == 0 || width < self.bullet_width {
            return Text::empty();
        }

        let prefix_width = self.prefix_width();
        if width <= prefix_width {
            let content = self.content.render(1);
            if content.lines().is_empty() {
                Text::empty()
            } else {
                Text::from_lines(vec![self.first_prefix_line(width - self.bullet_width)])
            }
        } else {
            let content_width = width - prefix_width;
            let content = self
                .content
                .render(content_width as u16)
                .into_wrapped(content_width);
            if content.lines().is_empty() {
                return Text::empty();
            }

            Text::from_lines(
                content
                    .lines()
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        if index == 0 {
                            self.first_line(line)
                        } else {
                            self.continuation_line(line)
                        }
                    })
                    .collect(),
            )
        }
    }

    fn render_every_frame(&self) -> bool {
        self.content.render_every_frame()
    }
}

fn push_spaces(spans: &mut Vec<Span>, width: usize) {
    if width == 0 {
        return;
    }

    // SAFETY: Repeated spaces cannot contain structural terminal content.
    spans.push(unsafe { Span::new_unchecked(" ".repeat(width), Style::new()) });
}

fn validate_bullet(bullet: &str, style: Style) -> Result<usize, TextError> {
    Span::new(bullet.to_owned(), style)?;

    let width = UnicodeWidthStr::width(bullet);
    if width == 0 {
        return Err(TextError::StructuralContent);
    }

    Ok(width)
}
