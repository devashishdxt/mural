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

pub use body::Text;
pub use color::Color;
pub use error::TextError;
pub use line::Line;
pub use modifiers::Modifiers;
pub use span::Span;
pub use style::Style;
