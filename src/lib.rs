pub mod backend;
mod render;
mod size;
mod terminal;
mod text;

pub use backend::{Backend, FakeBackend, Operation, StdoutBackend};
pub use render::Render;
pub use size::Size;
pub use terminal::Terminal;
pub use text::{Color, Line, Modifiers, Span, Style, Text, TextError};
