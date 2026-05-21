//! Conversational terminal rendering for command-line applications.
//!
//! Brisk manages a small live conversation surface in the terminal's normal
//! buffer. Applications add transcript-like **live** blocks first and optional
//! **pinned** blocks after them for status lines, progress, or ephemeral UI.
//! Nothing is drawn until [`Terminal::render`] is called, so callers can batch
//! several mutations before one terminal update. Calling [`Terminal::finish`]
//! removes pinned UI, leaves the rendered live transcript behind, restores the
//! cursor, and flushes the backend.
//!
//! Brisk intentionally does **not** own an alternate screen, stdin, raw mode,
//! signal handling, or the event loop. Resize handling is caller-driven through
//! [`Terminal::resize`]. If another component writes to the terminal, use the
//! backend escape hatch and then call [`Terminal::force_full_redraw`] before the
//! next render so Brisk can recover its cached screen snapshot with a full
//! rewrite.
//!
//! The public API is organized around:
//!
//! - [`terminal`] for the renderer lifecycle and live/pinned block management.
//! - [`text`] for validated plain, raw, ANSI, styled, and wrapped text.
//! - [`backend`] for stdout integration, fake backends, and custom terminal I/O.
//! - [`blocks`] for pre-built renderable blocks such as [`Hr`], [`ListItem`], [`Spinner`], [`Textarea`], and [`Padding`].
//! - [`error`] for typed lifecycle and identified-block errors.
//!
//! See `examples/conversation.rs` for a runnable end-to-end conversation model.
//!
//! ```
//! # use brisk::{FakeBackend, Size, Terminal, Text};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24)))?;
//!
//! terminal.push_live(Text::from_plain("user: hello")?)?;
//! terminal.insert_live("assistant", Text::from_plain("assistant: …")?)?;
//! terminal.insert_pinned("status", Text::from_plain("status: streaming")?)?;
//! terminal.render()?;
//!
//! *terminal.live_block_mut::<Text>("assistant")? =
//!     Text::from_plain("assistant: hello back")?;
//! terminal.resize(Size::new(60, 20))?;
//! terminal.force_full_redraw()?;
//! terminal.render()?;
//!
//! terminal.finish()?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

pub mod backend;
pub mod blocks;
pub mod error;
mod render;
pub mod size;
pub mod terminal;
pub mod text;

pub use backend::{Backend, FakeBackend, Operation, StdoutBackend};
pub use blocks::{
    Hr, ListItem, Padding, Spinner, Textarea, hr, list_item, padding, spinner, textarea,
};
pub use error::TerminalError;
pub use render::Render;
pub use size::Size;
pub use terminal::Terminal;
pub use text::{Color, Line, Modifiers, Span, Style, Text, TextError};
