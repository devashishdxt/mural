#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod backend;
mod block;
mod color_scheme;
mod differ;
mod frame;
pub mod key;
mod planner;
mod region;
mod renderer;
mod terminal;

pub use self::{
    backend::{Backend, BackendProbe, TerminaBackend},
    block::{Block, RenderContext},
    color_scheme::ColorScheme,
    terminal::{CursorPosition, Error, Terminal, TerminalSize},
};
