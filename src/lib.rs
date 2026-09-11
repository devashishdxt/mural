//! Incremental terminal rendering for conversational command-line applications.
//!
//! Mural renders application output as a sequence of [`Block`]s. Live blocks become the durable
//! transcript, while pinned blocks are shown only during regular renders and are removed by
//! [`Terminal::finish`]. Re-rendering updates only the terminal lines that changed.
//!
//! # Rendering text
//!
//! ```no_run
//! use mural::{ColorScheme, CursorPosition, Terminal, TerminalSize, TerminaBackend};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = TerminaBackend::new()?;
//!     let mut terminal = Terminal::new(
//!         backend,
//!         TerminalSize { height: 24, width: 80 },
//!         CursorPosition { row: 0, column: 0 },
//!         ColorScheme::Dark,
//!     )?;
//!
//!     terminal.push_live("Preparing response...");
//!     terminal.insert_pinned("status", String::from("Working..."));
//!     terminal.render()?;
//!
//!     *terminal.get_pinned_mut::<String>("status").unwrap() = "Done".into();
//!     terminal.render()?;
//!     terminal.finish()?;
//!     Ok(())
//! }
//! ```
//!
//! # Custom blocks
//!
//! Implement [`Block`] to render application state using the available width and color scheme:
//!
//! ```
//! use std::borrow::Cow;
//! use mural::{Backend, Block, RenderContext, Terminal};
//!
//! struct Counter(usize);
//!
//! impl Block for Counter {
//!     fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
//!         vec![Cow::Owned(format!("Count: {} (width {})", self.0, context.width()))]
//!     }
//! }
//!
//! fn increment_counter<B: Backend>(terminal: &mut Terminal<B>) {
//!     terminal.insert_pinned("counter", Counter(0));
//!     terminal.get_pinned_mut::<Counter>("counter").unwrap().0 += 1;
//! }
//! ```
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
// #![deny(missing_docs)]

mod backend;
mod block;
mod color_scheme;
mod differ;
mod frame;
mod planner;
mod region;
mod renderer;
mod style;
mod terminal;
pub mod widget;

pub use self::{
    backend::{Backend, BackendProbe, TerminaBackend},
    block::{Block, RenderContext},
    color_scheme::ColorScheme,
    style::Color,
    terminal::{CursorPosition, Error, Terminal, TerminalSize},
};
