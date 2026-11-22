//! Lua 5.4 plugin runtime with sandboxed environment and `lb.*` API.
//!
//! # Lazy-load contract
//!
//! The Lua VM is **not** created at import time.  `PluginRuntime::init_if_needed`
//! is a no-op when called with an empty slice, so the VM is only started when
//! at least one plugin path is configured.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod api;
pub mod runtime;
pub mod sandbox;

pub use api::LbApi;
pub use runtime::{PluginError, PluginRuntime};
