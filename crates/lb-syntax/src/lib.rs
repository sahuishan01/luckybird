//! Tree-sitter integration: grammar loader, language detection, highlight engine.
//!
//! # Lazy-load contract
//!
//! Nothing in this crate touches disk until `SyntaxLayer::load` is called.
//! `SyntaxRegistry::new()` only parses the embedded `grammars.toml`; no
//! file-system access occurs until the first buffer is opened.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod highlight;
pub mod loader;
pub mod registry;

pub use highlight::{HighlightError, Highlighter};
pub use loader::{GrammarLoader, LoadError};
pub use registry::{GrammarMeta, Registry};

use std::path::PathBuf;
use tracing::debug;

// ─── Public span type ────────────────────────────────────────────────────────

/// A single syntax-highlight span: byte range + capture index.
///
/// The capture index is into `Highlighter::capture_names`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    /// Index into the capture-name table of the query that produced this span.
    pub capture: u32,
}

// ─── SyntaxLayer ─────────────────────────────────────────────────────────────

/// The highlight state for one open buffer.
///
/// Created by `SyntaxManager::layer_for`, which looks up the language, loads
/// the grammar if present, compiles the highlight query, and returns a ready
/// `SyntaxLayer`.  When the grammar file is absent the layer silently degrades
/// to returning empty spans.
pub struct SyntaxLayer {
    inner: Option<Highlighter>,
    pub language: String,
}

impl SyntaxLayer {
    fn plain(language: String) -> Self {
        Self { inner: None, language }
    }

    fn with_highlighter(language: String, h: Highlighter) -> Self {
        Self { inner: Some(h), language }
    }

    /// Highlight `text`.  Returns `&[]` when grammar/query is unavailable.
    pub fn highlight(&mut self, text: &str) -> Vec<HighlightSpan> {
        match self.inner.as_mut() {
            Some(h) => h.highlight(text),
            None => Vec::new(),
        }
    }

    /// Capture name (e.g. `keyword`, `string.special`) for a span's capture index.
    pub fn capture_name(&self, index: u32) -> Option<&str> {
        self.inner.as_ref().and_then(|h| h.capture_name(index))
    }

    /// `true` when a grammar + query are loaded (full highlighting active).
    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }
}

// ─── SyntaxManager ───────────────────────────────────────────────────────────

/// Owns the grammar loader and the embedded language registry.
///
/// One `SyntaxManager` lives for the lifetime of the editor process.
pub struct SyntaxManager {
    registry: Registry,
    loader: GrammarLoader,
    query_dir: PathBuf,
}

impl SyntaxManager {
    /// Create a manager.
    ///
    /// * `grammar_dir` — directory containing per-language `.so`/`.dll` files.
    /// * `query_dir`   — directory containing per-language highlight queries
    ///   at `<query_dir>/<lang>/highlights.scm`.
    pub fn new(grammar_dir: PathBuf, query_dir: PathBuf) -> Self {
        Self {
            registry: registry::load(),
            loader: GrammarLoader::new(grammar_dir),
            query_dir,
        }
    }

    /// Detect the language for `path` using extension/shebang.
    pub fn detect(&self, path: &str, first_line: Option<&str>) -> Option<String> {
        registry::detect_by_path(&self.registry, path)
            .or_else(|| {
                first_line.and_then(|l| registry::detect_by_shebang(&self.registry, l))
            })
    }

    /// Create a `SyntaxLayer` for `language`.
    ///
    /// Loads the grammar and compiles the highlight query on first call per
    /// language. Falls back to a no-op layer when the grammar is absent.
    pub fn layer_for(&mut self, language: &str) -> SyntaxLayer {
        let meta = match self.registry.get(language) {
            Some(m) => m.clone(),
            None => return SyntaxLayer::plain(language.to_string()),
        };

        let lang = match self.loader.load(language, &meta.grammar_fn) {
            Ok(l) => l,
            Err(e) => {
                debug!(language, error = %e, "grammar not available; falling back to plain text");
                return SyntaxLayer::plain(language.to_string());
            }
        };

        // Load highlight query from disk (optional).
        let query_src = self.load_query(language);

        match Highlighter::new(lang, query_src.as_deref()) {
            Ok(h) => SyntaxLayer::with_highlighter(language.to_string(), h),
            Err(e) => {
                debug!(language, error = %e, "highlight query error; falling back to plain text");
                SyntaxLayer::plain(language.to_string())
            }
        }
    }

    /// Forget a loaded grammar so a newly installed build is picked up.
    pub fn forget_grammar(&mut self, language: &str) {
        self.loader.forget(language);
    }

    fn load_query(&self, language: &str) -> Option<String> {
        let path = self.query_dir.join(language).join("highlights.scm");
        std::fs::read_to_string(&path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn manager_detect_rs() {
        let mgr = SyntaxManager::new(PathBuf::from("/noop"), PathBuf::from("/noop"));
        assert_eq!(mgr.detect("src/main.rs", None), Some("rust".into()));
    }

    #[test]
    fn manager_layer_degrades_gracefully_when_no_grammar() {
        let mut mgr = SyntaxManager::new(PathBuf::from("/noop"), PathBuf::from("/noop"));
        let layer = mgr.layer_for("rust");
        // Grammar dir doesn't exist → plain fallback.
        assert!(!layer.is_active());
        // Highlight returns empty vec, no panic.
        let mut layer = layer;
        assert!(layer.highlight("fn main() {}").is_empty());
    }
}
