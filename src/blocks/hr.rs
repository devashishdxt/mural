use crate::{Line, Render, Span, Style, Text, TextError};
use unicode_width::UnicodeWidthChar;

const DEFAULT_CHARACTER: char = '─';

/// Creates a default horizontal rule block.
///
/// This is a convenience wrapper around [`Hr::new`].
///
/// # Examples
///
/// ```
/// # use mural::hr;
/// let rule = hr();
/// assert_eq!(rule.fill_character(), '─');
/// ```
pub fn hr() -> Hr {
    Hr::new()
}

/// A horizontal rule block that fills the render width with a repeated character.
///
/// `Hr` defaults to a plain `─` rule. Custom characters may have any non-zero
/// terminal display width; rendering repeats the character as many times as fit
/// in the available width.
///
/// # Examples
///
/// ```
/// # use mural::{Hr, Style, TextError};
/// # fn main() -> Result<(), TextError> {
/// let rule = Hr::new()
///     .character('═')?
///     .style(Style::new().dim());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hr {
    character: char,
    character_width: usize,
    style: Style,
}

impl Hr {
    /// Creates a plain horizontal rule using `─`.
    pub fn new() -> Self {
        Self {
            character: DEFAULT_CHARACTER,
            character_width: 1,
            style: Style::new(),
        }
    }

    /// Creates a plain horizontal rule using `character`.
    ///
    /// Characters with non-zero display width are accepted. Structural terminal
    /// content such as newlines, tabs, ANSI escapes, and other control
    /// characters is rejected.
    pub fn with_character(character: char) -> Result<Self, TextError> {
        Self::new().character(character)
    }

    /// Sets the repeated character for this horizontal rule.
    ///
    /// Characters with non-zero display width are accepted. Structural terminal
    /// content such as newlines, tabs, ANSI escapes, and other control
    /// characters is rejected.
    pub fn character(mut self, character: char) -> Result<Self, TextError> {
        self.character_width = validate_character(character)?;
        self.character = character;
        Ok(self)
    }

    /// Sets the style applied to the rule line.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Returns the character repeated by this horizontal rule.
    pub fn fill_character(&self) -> char {
        self.character
    }

    /// Returns the terminal display width of the repeated character.
    pub fn character_width(&self) -> usize {
        self.character_width
    }

    /// Returns the style applied to the rule line.
    pub fn rule_style(&self) -> Style {
        self.style
    }
}

impl Default for Hr {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for Hr {
    fn render(&self, width: u16) -> Text {
        let width = usize::from(width);
        if width == 0 {
            return Text::empty();
        }

        let count = width / self.character_width;
        if count == 0 {
            return Text::from_lines(vec![Line::from_spans(Vec::new())]);
        }

        let content: String = std::iter::repeat_n(self.character, count).collect();
        // `Hr` validates the repeated character before storing it, and
        // repetition cannot introduce structural terminal content.
        let span = Span::from_trusted_content(content, self.style);
        Text::from_lines(vec![Line::from_spans(vec![span])])
    }
}

fn validate_character(character: char) -> Result<usize, TextError> {
    let mut buffer = [0; 4];
    Span::validate_content(character.encode_utf8(&mut buffer))?;

    let width = UnicodeWidthChar::width(character).unwrap_or(0);
    if width == 0 {
        return Err(TextError::StructuralContent);
    }

    Ok(width)
}
