#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod event;
pub mod overlay;
pub mod render;
pub mod statusline;
pub mod term;

pub use event::{read_event, EditorEvent, KeyCode, KeyEvent2, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
pub use overlay::{OverlayModel, OverlayRow};
pub use render::{Diagnostic, DiagnosticSeverity, Highlight, View};
pub use statusline::StatusLine;
pub use term::Tui;
