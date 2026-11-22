//! Language-server manager: registry, PATH detection, managed cache,
//! download + verify, and server spawning.
//!
//! # Lazy-load contract
//!
//! `ServerRegistry::load()` only parses the embedded TOML — no disk I/O.
//! `LspManager::get_or_spawn` is the first function that touches the OS.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod download;
pub mod registry;
pub mod resolve;
pub mod spawn;

pub use download::{download_and_install, DownloadError};
pub use registry::{server_for_language, ServerMeta, ServerRegistry};
pub use resolve::{ResolveResult, Resolver};
pub use spawn::{spawn_from_path, SpawnError};

use std::collections::HashMap;
use std::path::PathBuf;

use lb_lsp::LspClient;
use tracing::{debug, warn};

// ─── LspManager ──────────────────────────────────────────────────────────────

/// Owns all active language server clients.
///
/// One `LspManager` lives for the lifetime of the editor.  It lazily spawns
/// servers when a buffer of a new language is first opened.
pub struct LspManager {
    registry: ServerRegistry,
    resolver: Resolver,
    /// Active servers keyed by server name (not language).
    active: HashMap<String, LspClient>,
    /// Workspace root used as the LSP root URI.
    workspace_root: PathBuf,
}

impl LspManager {
    /// Create a new manager.
    ///
    /// * `cache_root`      — `~/.local/share/luckybird/lsp/`
    /// * `workspace_root`  — directory of the first opened file or CWD
    pub fn new(cache_root: PathBuf, workspace_root: PathBuf) -> Self {
        Self {
            registry: registry::load(),
            resolver: Resolver::new(cache_root),
            active: HashMap::new(),
            workspace_root,
        }
    }

    /// Return the active `LspClient` for `language`, spawning it if needed.
    ///
    /// Returns `None` when:
    /// - No server is registered for `language`.
    /// - The server binary is not found (needs install).
    /// - Spawning failed.
    ///
    /// The caller is responsible for prompting the user to install the server
    /// when `None` is returned and `needs_install(language)` is `true`.
    pub async fn get_or_spawn(&mut self, language: &str) -> Option<&mut LspClient> {
        // Find the server name for this language.
        let (server_name, meta) = registry::server_for_language(&self.registry, language)?;
        let server_name = server_name.to_string();

        // Already active.
        if self.active.contains_key(&server_name) {
            return self.active.get_mut(&server_name);
        }

        // Resolve the binary.
        let meta = meta.clone();
        let result = self.resolver.resolve(&server_name, &meta);
        let binary = match result.path() {
            Some(p) => p.to_path_buf(),
            None => {
                debug!(language, server = %server_name, "server not found; skipping LSP");
                return None;
            }
        };

        // Spawn.
        let root = self.workspace_root.clone();
        match spawn::spawn_server(&binary, &meta, &root).await {
            Ok(client) => {
                self.active.insert(server_name.clone(), client);
                self.active.get_mut(&server_name)
            }
            Err(e) => {
                warn!(language, error = %e, "failed to spawn LSP server");
                None
            }
        }
    }

    /// Return `true` if a server is registered for `language` but not yet found
    /// on the system (installation needed).
    pub fn needs_install(&self, language: &str) -> bool {
        match registry::server_for_language(&self.registry, language) {
            None => false,
            Some((server_name, meta)) => {
                let result = self.resolver.resolve(server_name, meta);
                !result.is_found()
            }
        }
    }

    /// Return the server name registered for `language`, if any.
    pub fn server_name_for(&self, language: &str) -> Option<&str> {
        registry::server_for_language(&self.registry, language)
            .map(|(name, _)| name)
    }

    /// Retrieve an already-active client (does not spawn).
    pub fn get(&mut self, language: &str) -> Option<&mut LspClient> {
        let server_name = self.server_name_for(language)?.to_string();
        self.active.get_mut(&server_name)
    }

    /// Shut down all active clients gracefully.
    pub async fn shutdown_all(&mut self) {
        for (name, client) in self.active.iter_mut() {
            debug!(server = %name, "shutting down LSP server");
            let _ = client.shutdown().await;
        }
        self.active.clear();
    }

    /// Return the resolver (for install prompts / path queries).
    pub fn resolver(&self) -> &Resolver {
        &self.resolver
    }
}
