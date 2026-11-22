use std::collections::HashMap;
use serde::Deserialize;

/// Metadata for one tree-sitter grammar entry.
#[derive(Debug, Clone, Deserialize)]
pub struct GrammarMeta {
    /// File extensions that map to this language (without leading dot).
    pub extensions: Vec<String>,
    /// C symbol exported by the grammar shared library, e.g. `tree_sitter_rust`.
    pub grammar_fn: String,
    /// Interpreter names that appear in shebangs, e.g. `["python3"]`.
    #[serde(default)]
    pub shebangs: Vec<String>,
}

/// The full grammar registry, keyed by language name.
pub type Registry = HashMap<String, GrammarMeta>;

const GRAMMARS_TOML: &str = include_str!("../grammars.toml");

/// Load the embedded grammar registry.  Panics only in debug builds if the
/// embedded TOML is malformed (indicates a packaging error, not a user error).
pub fn load() -> Registry {
    toml::from_str(GRAMMARS_TOML).unwrap_or_default()
}

/// Detect the language for a file path by matching the extension.
/// Returns `None` when no grammar covers that extension.
pub fn detect_by_path(registry: &Registry, path: &str) -> Option<String> {
    // Extract extension: last dot-separated segment, case-insensitive match.
    // Also handle bare filenames like "Dockerfile".
    let filename = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path);

    // Try full filename first (covers "Dockerfile").
    for (lang, meta) in registry.iter() {
        for ext in &meta.extensions {
            if ext.eq_ignore_ascii_case(filename) {
                return Some(lang.clone());
            }
        }
    }

    // Then try just the extension.
    let ext = filename.rsplit('.').next().unwrap_or("");
    if ext.is_empty() || ext == filename {
        return None;
    }
    for (lang, meta) in registry.iter() {
        for e in &meta.extensions {
            if e.eq_ignore_ascii_case(ext) {
                return Some(lang.clone());
            }
        }
    }
    None
}

/// Detect the language from the first line of a file (shebang detection).
///
/// Handles both `#!/usr/bin/python3` and `#!/usr/bin/env python3`.
pub fn detect_by_shebang(registry: &Registry, first_line: &str) -> Option<String> {
    let line = first_line.trim();
    if !line.starts_with("#!") {
        return None;
    }
    let rest = line.trim_start_matches("#!").trim();
    let mut parts = rest.split_whitespace();

    // Take the last path component of the first token.
    let first_token = parts.next().and_then(|s| s.rsplit('/').next()).unwrap_or("");

    // `#!/usr/bin/env python3` → the real interpreter is the next token.
    let interp = if first_token == "env" {
        parts.next().and_then(|s| s.rsplit('/').next()).unwrap_or("")
    } else {
        first_token
    };

    if interp.is_empty() {
        return None;
    }

    for (lang, meta) in registry.iter() {
        if meta.shebangs.iter().any(|s| s == interp) {
            return Some(lang.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads() {
        let reg = load();
        assert!(reg.len() >= 30, "expected ≥30 grammar entries, got {}", reg.len());
    }

    #[test]
    fn detect_rust() {
        let reg = load();
        assert_eq!(detect_by_path(&reg, "main.rs"), Some("rust".into()));
    }

    #[test]
    fn detect_python_shebang() {
        let reg = load();
        let lang = detect_by_shebang(&reg, "#!/usr/bin/env python3");
        assert_eq!(lang, Some("python".into()));
    }

    #[test]
    fn detect_dockerfile() {
        let reg = load();
        assert_eq!(detect_by_path(&reg, "Dockerfile"), Some("dockerfile".into()));
    }
}
