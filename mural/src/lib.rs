//! High-level terminal widgets and their backend-independent input types.

#![warn(missing_docs)]

// This crate-private contract is consumed by the textarea state introduced in the next ticket.
#[allow(
    dead_code,
    reason = "editing is an independently delivered prerequisite"
)]
pub(crate) mod editing;
pub mod key;
#[allow(
    dead_code,
    reason = "layout is an independently delivered prerequisite"
)]
pub(crate) mod layout;
pub mod widget;
