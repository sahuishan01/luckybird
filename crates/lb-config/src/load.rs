use std::path::{Path, PathBuf};
use crate::schema::{Config, KeyMap};

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config I/O error: {e}"),
            Self::Parse(msg) => write!(f, "config parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(_) => None,
        }
    }
}

// ─── Path resolution ─────────────────────────────────────────────────────────

/// `~/.config/luckybird/` (XDG) or platform equivalent.
pub fn default_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("luckybird"))
}

/// `~/.config/luckybird/config.toml` (or platform equivalent).
pub fn default_config_path() -> Option<PathBuf> {
    default_config_dir().map(|p| p.join("config.toml"))
}

// ─── Loading ─────────────────────────────────────────────────────────────────

impl Config {
    /// Load config from the default platform path.
    /// Returns `Config::default()` if the file doesn't exist.
    pub fn load_default() -> Result<Self, ConfigError> {
        match default_config_path() {
            Some(path) if path.exists() => Self::load_from(&path),
            _ => Ok(Self::default()),
        }
    }

    /// Load config from a specific path.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml_str(&text)
    }

    /// Parse config from a TOML string.
    /// User-specified keys are merged over the built-in defaults.
    pub fn from_toml_str(text: &str) -> Result<Self, ConfigError> {
        let mut cfg: Self =
            toml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))?;
        // Merge user keys over built-in defaults so partial configs work.
        let user_keys = std::mem::take(&mut cfg.keys);
        let mut keys = KeyMap::builtin_defaults();
        keys.apply_overrides(&user_keys);
        cfg.keys = keys;
        Ok(cfg)
    }
}
