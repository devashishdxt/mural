//! Text, styling, ANSI parsing, and Unicode-aware wrapping.
//!
//! Text constructors reject structural terminal content in plain strings. Use
//! raw or ANSI constructors when input may include ANSI sequences or multiple
//! lines.

mod ansi;
mod body;
mod color;
mod error;
mod line;
mod modifiers;
mod span;
mod style;

pub use self::{
    body::Text, color::Color, error::TextError, line::Line, modifiers::Modifiers, span::Span,
    style::Style,
};
