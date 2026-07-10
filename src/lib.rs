#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod backend;
mod block;
mod differ;
mod frame;
mod planner;
mod region;
mod renderer;
mod terminal;

pub use self::{
    backend::{Backend, TerminaBackend},
    block::Block,
    terminal::{CursorPosition, Error, Terminal, TerminalSize},
};
