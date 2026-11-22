//! Compile-time Rust plugins.
//!
//! A plugin implements [`Plugin`] and registers keymaps and event handlers
//! through [`PluginApi`]. Plugins are linked into the editor binary (enable
//! them with Cargo features in `lb-app`), so they cost nothing at startup
//! beyond their own code.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod runtime;

pub use runtime::{Plugin, PluginApi, PluginRuntime};
