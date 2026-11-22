use std::path::{Path, PathBuf};

use tracing::debug;

use crate::registry::ServerMeta;

// ─── Resolution result ───────────────────────────────────────────────────────

/// The outcome of trying to locate a language server binary.
#[derive(Debug, Clone)]
pub enum ResolveResult {
    /// Found on the system PATH.
    OnPath(PathBuf),
    /// Found in the managed cache directory.
    Cached(PathBuf),
    /// Not found anywhere; user should be prompted to install.
    NotFound,
}

impl ResolveResult {
    /// Returns the path if the server was found.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::OnPath(p) | Self::Cached(p) => Some(p),
            Self::NotFound => None,
        }
    }

    pub fn is_found(&self) -> bool {
        self.path().is_some()
    }
}

// ─── Resolver ────────────────────────────────────────────────────────────────

/// Locates language server binaries via PATH or the managed cache.
pub struct Resolver {
    /// Root of managed server cache, e.g. `~/.local/share/luckybird/lsp/`.
    cache_root: PathBuf,
}

impl Resolver {
    pub fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    /// Attempt to find the binary for `meta`.
    ///
    /// Resolution order:
    /// 1. `which::which(meta.binary)` — system PATH.
    /// 2. `<cache_root>/<server_name>/<meta.binary>[.exe]` — managed cache.
    /// 3. `ResolveResult::NotFound`.
    pub fn resolve(&self, server_name: &str, meta: &ServerMeta) -> ResolveResult {
        // 1. PATH lookup.
        if let Ok(path) = which::which(&meta.binary) {
            debug!(binary = %meta.binary, path = %path.display(), "found LSP server on PATH");
            return ResolveResult::OnPath(path);
        }

        // 2. Managed cache lookup.
        let cached = self.cached_path(server_name, &meta.binary);
        if cached.exists() {
            debug!(binary = %meta.binary, path = %cached.display(), "found LSP server in cache");
            return ResolveResult::Cached(cached);
        }

        debug!(binary = %meta.binary, "LSP server not found");
        ResolveResult::NotFound
    }

    /// Return the canonical cache path for a server binary.
    pub fn cached_path(&self, server_name: &str, binary: &str) -> PathBuf {
        let exe = if cfg!(windows) && !binary.ends_with(".exe") {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        };
        self.cache_root.join(server_name).join(exe)
    }

    /// Return the directory that would hold `server_name`'s files.
    pub fn server_cache_dir(&self, server_name: &str) -> PathBuf {
        self.cache_root.join(server_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ServerMeta;

    fn meta(binary: &str) -> ServerMeta {
        ServerMeta {
            languages: vec!["rust".into()],
            binary: binary.to_string(),
            args: vec![],
        }
    }

    #[test]
    fn nonexistent_binary_returns_not_found() {
        let resolver = Resolver::new(PathBuf::from("/nonexistent/lb_cache"));
        let result = resolver.resolve("rust-analyzer", &meta("__lb_no_such_binary__"));
        assert!(matches!(result, ResolveResult::NotFound));
    }

    #[test]
    fn cached_path_structure() {
        let resolver = Resolver::new(PathBuf::from("/cache"));
        let path = resolver.cached_path("rust-analyzer", "rust-analyzer");
        assert!(path.starts_with("/cache/rust-analyzer"));
    }
}
