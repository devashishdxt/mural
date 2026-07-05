#![deny(missing_docs)]

//! Normal-screen conversational terminal rendering.
//!
//! `mural` renders chat/log-style output in the terminal's normal screen. Your application supplies the current
//! terminal size, current cursor position, and a backend, then explicitly renders live transcript content and pinned
//! transient UI.
//!
//! Live content is the durable transcript you want users to keep in their shell after your program exits: chat
//! messages, log lines, tool output, or final answers. Pinned content is temporary UI that should appear below the
//! transcript while the program is running: spinners, progress, prompts, or status text.
//!
//! ```text
//! normal terminal scrollback
//! ┌──────────────────────────────┐
//! │ previous shell output        │
//! │                              │
//! │ live: user: hello            │  kept after finish
//! │ live: assistant: thinking... │  kept after finish
//! │ pinned: ⠋ calling tool       │  removed by finish
//! └──────────────────────────────┘
//! ```
//!
//! # Quickstart
//!
//! [`Terminal::new`] needs:
//!
//! - [`TerminalSize::width`] and [`TerminalSize::height`]: current terminal cell dimensions.
//! - [`CursorPosition::row`] and [`CursorPosition::column`]: current cursor position in zero-based visible-screen
//!   coordinates.
//! - A [`Backend`], such as [`TerminaBackend`].
//!
//! Use your terminal library to fetch the size and cursor position. With `termina`, terminal size comes from
//! `PlatformTerminal::get_dimensions`; cursor position can be read by requesting an active-position report and
//! converting the returned one-based row and column with `get_zero_based`.
//!
//! ```no_run
//! use mural::{CursorPosition, Terminal, TerminalSize, TerminaBackend};
//!
//! fn current_terminal_size() -> Result<TerminalSize, Box<dyn std::error::Error>> {
//!     // For example, with termina: PlatformTerminal::get_dimensions().
//!     todo!("read terminal dimensions")
//! }
//!
//! fn current_cursor_position() -> Result<CursorPosition, Box<dyn std::error::Error>> {
//!     // For example, with termina: request and read an active-position report.
//!     todo!("read cursor position")
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = TerminaBackend::new()?;
//! let mut terminal = Terminal::new(
//!     backend,
//!     current_terminal_size()?,
//!     current_cursor_position()?,
//! )?;
//!
//! terminal.push_live("user: hello");
//! terminal.insert_pinned("status", "thinking...");
//! terminal.render()?;
//!
//! terminal.clear_pinned();
//! terminal.push_live("assistant: hello back");
//! terminal.render()?;
//!
//! terminal.resize(current_terminal_size()?)?;
//! terminal.render()?;
//!
//! terminal.force_full_redraw();
//! terminal.render()?;
//!
//! terminal.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! `finish` leaves live content in the terminal and removes pinned content. The crate does not enter alternate screen,
//! enable raw mode, read input, query terminal size or cursor position, install signal handlers, own an event loop,
//! set scrolling margins, or reset the terminal.

mod backend;
mod block;
mod error;
mod region;
mod terminal;
#[cfg(test)]
mod test_utils;
mod types;

pub use backend::{Backend, TerminaBackend};
pub use block::Block;
pub use error::{LifecycleError, TerminalError};
pub use terminal::Terminal;
pub use types::{CursorPosition, TerminalSize};
