mod color;
mod error;
mod line;
mod modifiers;
mod span;
mod style;
mod text;

pub use color::Color;
pub use error::TextError;
pub use line::Line;
pub use modifiers::Modifiers;
pub use span::Span;
pub(crate) use span::validate_structural_content;
pub use style::Style;
pub use text::Text;
