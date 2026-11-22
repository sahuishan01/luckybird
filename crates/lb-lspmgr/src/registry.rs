use std::collections::HashMap;
use serde::Deserialize;

/// Metadata for one language server.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerMeta {
    /// Language IDs served by this server.
    pub languages: Vec<String>,
    /// Executable name used for PATH detection and the managed cache directory.
    pub binary: String,
    /// Extra arguments appended after any positional args on spawn.
    #[serde(default)]
    pub args: Vec<String>,
}

/// The server registry keyed by server name.
pub type ServerRegistry = HashMap<String, ServerMeta>;

const REGISTRY_TOML: &str = include_str!("../registry.toml");

/// Parse the embedded registry.  On malformed TOML returns an empty map
/// (graceful degradation — LSP management simply won't work).
pub fn load() -> ServerRegistry {
    toml::from_str(REGISTRY_TOML).unwrap_or_default()
}

/// Find the first server that handles `language`.
pub fn server_for_language<'r>(
    registry: &'r ServerRegistry,
    language: &str,
) -> Option<(&'r str, &'r ServerMeta)> {
    registry
        .iter()
        .find(|(_, meta)| meta.languages.iter().any(|l| l == language))
        .map(|(name, meta)| (name.as_str(), meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads() {
        let reg = load();
        assert!(reg.len() >= 28, "expected ≥28 server entries");
    }

    #[test]
    fn rust_analyzer_for_rust() {
        let reg = load();
        let (name, _) = server_for_language(&reg, "rust").expect("rust server missing");
        assert_eq!(name, "rust-analyzer");
    }

    #[test]
    fn gopls_for_go() {
        let reg = load();
        let (name, _) = server_for_language(&reg, "go").expect("go server missing");
        assert_eq!(name, "gopls");
    }
}
