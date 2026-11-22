//! Core buffer primitives for luckybird.
//!
//! No I/O, no async, no rendering. Everything here is pure data manipulation
//! that other crates build on.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod buffer;
pub mod encoding;
pub mod selection;
pub mod undo;

pub use buffer::{Buffer, Change, Edit, EditError};
pub use encoding::{utf16_to_utf8, utf8_to_utf16};
pub use selection::{
    Range, Selection,
    move_char_left, move_char_right,
    move_line_down, move_line_up,
    move_line_end, move_line_start,
    move_doc_start, move_doc_end,
    move_word_forward, move_word_backward,
};
pub use undo::UndoTree;
