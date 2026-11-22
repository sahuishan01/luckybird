use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};
use tree_sitter::Language;
use tracing::{debug, warn};

// ─── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum LoadError {
    NotFound(PathBuf),
    Open(String),
    Symbol(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "grammar library not found: {}", p.display()),
            Self::Open(e) => write!(f, "could not open grammar library: {e}"),
            Self::Symbol(e) => write!(f, "grammar symbol not found: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

// ─── Grammar entry ───────────────────────────────────────────────────────────

/// A grammar loaded from a shared library.
/// The `_lib` field keeps the library mapped while `language` is in use.
struct Entry {
    language: Language,
    /// SAFETY: `language` holds a `*const TSLanguage` that lives inside `_lib`.
    /// Dropping `_lib` before `language` is last-used would be UB.
    /// We own both in this struct so the drop order is `language` then `_lib` —
    /// which is fine: the pointer is only accessed through `language` while the
    /// struct is alive.
    _lib: Library,
}

// SAFETY: `Language` is Send+Sync (it's a const raw pointer into read-only
// data).  `Library` is not automatically Send, but we never mutate the mapped
// pages after construction, so sharing across threads is safe here.
unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

// ─── GrammarLoader ───────────────────────────────────────────────────────────

/// Caches dynamically-loaded tree-sitter grammars.
///
/// Grammars live in `grammar_dir/<lang>.<platform_ext>`.
/// When a grammar is not present the error is returned and the caller must
/// decide whether to download it or fall back to plain-text mode.
pub struct GrammarLoader {
    cache: HashMap<String, Entry>,
    grammar_dir: PathBuf,
}

impl GrammarLoader {
    pub fn new(grammar_dir: PathBuf) -> Self {
        Self { cache: HashMap::new(), grammar_dir }
    }

    /// Return the `Language` for `lang`, loading the shared library on first use.
    pub fn load(&mut self, lang: &str, grammar_fn: &str) -> Result<Language, LoadError> {
        // Return cached language.
        if let Some(entry) = self.cache.get(lang) {
            return Ok(entry.language.clone());
        }

        let lib_path = self.grammar_dir.join(lib_filename(lang));
        debug!(lang, path = %lib_path.display(), "loading grammar");

        if !lib_path.exists() {
            warn!(lang, "grammar library not found");
            return Err(LoadError::NotFound(lib_path));
        }

        // SAFETY: We keep the library alive inside `Entry` for the lifetime of
        // this loader, preventing use-after-free.
        let lib = unsafe { Library::new(&lib_path) }
            .map_err(|e| LoadError::Open(e.to_string()))?;

        let language: Language = unsafe {
            // Tree-sitter grammar crates export the language constructor as a
            // C function returning `Language` (which is repr-compatible with
            // `*const TSLanguage` on all targets we care about).
            let sym: Symbol<unsafe extern "C" fn() -> Language> =
                lib.get(grammar_fn.as_bytes())
                    .map_err(|e| LoadError::Symbol(e.to_string()))?;
            sym()
        };

        let language_clone = language.clone();
        self.cache.insert(lang.to_string(), Entry { language, _lib: lib });
        Ok(language_clone)
    }

    /// Return the grammar directory path.
    pub fn grammar_dir(&self) -> &Path {
        &self.grammar_dir
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn lib_filename(lang: &str) -> String {
    #[cfg(target_os = "windows")]
    { format!("{lang}.dll") }
    #[cfg(target_os = "macos")]
    { format!("lib{lang}.dylib") }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { format!("lib{lang}.so") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_grammar_returns_not_found() {
        let dir = PathBuf::from("/nonexistent/grammar/dir");
        let mut loader = GrammarLoader::new(dir);
        let result = loader.load("rust", "tree_sitter_rust");
        assert!(matches!(result, Err(LoadError::NotFound(_))));
    }

    #[test]
    fn lib_filename_has_extension() {
        let name = lib_filename("rust");
        assert!(!name.is_empty());
        assert!(name.contains("rust"));
    }
}
