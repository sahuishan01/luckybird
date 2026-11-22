//! LSP client: JSON-RPC transport over child process stdio.
//!
//! # Lazy-load contract
//!
//! Nothing in this crate touches disk or spawns processes at import time.
//! `LspClient::spawn` is the only entry point that creates OS resources.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod client;
pub mod transport;

pub use client::{LspClient, LspError};

use lsp_types::PublishDiagnosticsParams;

// ─── Server events ───────────────────────────────────────────────────────────

/// Events sent from the server to the editor (server-initiated notifications).
#[derive(Debug)]
pub enum ServerEvent {
    /// `textDocument/publishDiagnostics` notification.
    Diagnostics(PublishDiagnosticsParams),
    /// Server process exited unexpectedly.
    Exited,
}
