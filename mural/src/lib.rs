//! High-level terminal widgets and backend-independent input types.
//!
//! `mural` provides widgets, while [`mural-core`](mural_core) owns terminal rendering and the
//! [`Block`](mural_core::Block) contract. The crates are deliberately not re-exported through one
//! another: applications that render a widget depend on and import `mural` and `mural-core`
//! separately.
//!
//! # Textarea contract
//!
//! [`Textarea`](widget::Textarea) stores sanitized multiline text. Terminal escape sequences and
//! unsupported control characters are removed, carriage returns are normalized to line feeds,
//! and tabs are preserved. Its cursor is a UTF-8 byte index, but is always clamped to an extended
//! grapheme-cluster boundary so edits cannot split a visible character.
//!
//! Rendering through [`Block`](mural_core::Block) remembers the supplied width. Widthless visual
//! navigation and default key handling subsequently use that remembered width; methods ending in
//! `_with_width` perform an explicit one-off layout without changing it. The rendered block uses a
//! fixed reverse-video software cursor because `mural-core` keeps the terminal's hardware cursor
//! hidden.
//!
//! [`KeyOutcome`](key::KeyOutcome) tells the application whether handling changed visible state,
//! reached a handled boundary without changing it, requested submission, or ignored the event.
//! Submission is application-controlled: Enter returns [`KeyOutcome::Submit`](key::KeyOutcome::Submit)
//! without clearing or otherwise mutating the textarea.
//!
//! # Example
//!
//! The example imports the widget and core block contract from their separate crates. A real
//! application can place the textarea into a `mural-core` terminal or region; no input loop is
//! required to use its editing contract.
//!
//! ```
//! use mural::{
//!     key::{KeyCode, KeyEvent, KeyOutcome},
//!     widget::Textarea,
//! };
//! use mural_core::Block;
//!
//! fn accepts_block(_: &impl Block) {}
//!
//! let mut editor = Textarea::new();
//! editor.insert("Hello").insert_char('!');
//! accepts_block(&editor);
//!
//! let outcome = editor.handle_key_event(KeyEvent::new(KeyCode::Enter));
//! let submitted = match outcome {
//!     KeyOutcome::Submit => Some(editor.take()),
//!     _ => None,
//! };
//!
//! assert_eq!(submitted.as_deref(), Some("Hello!"));
//! assert!(editor.is_empty());
//! ```

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
    reason = "layout source metadata is retained as a cohesive internal contract"
)]
pub(crate) mod layout;
pub(crate) mod rendering;
pub mod widget;
