use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Top-level Config ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub editor: EditorCfg,
    pub lsp: LspCfg,
    /// Raw key→command strings; resolved to `KeyChord` on demand by callers.
    #[serde(default)]
    pub keys: KeyMap,
    #[serde(rename = "plugin", default)]
    pub plugins: Vec<PluginCfg>,
    pub theme: ThemeCfg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: EditorCfg::default(),
            lsp: LspCfg::default(),
            keys: KeyMap::builtin_defaults(),
            plugins: Vec::new(),
            theme: ThemeCfg::default(),
        }
    }
}

// ─── [editor] ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EditorCfg {
    pub line_numbers: LineNumbers,
    pub tab_size: u8,
    pub expand_tabs: bool,
    pub auto_save: bool,
    /// Lines of context kept above/below the cursor when scrolling.
    pub scroll_off: u16,
    pub wrap: bool,
}

impl Default for EditorCfg {
    fn default() -> Self {
        Self {
            line_numbers: LineNumbers::Absolute,
            tab_size: 4,
            expand_tabs: true,
            auto_save: false,
            scroll_off: 5,
            wrap: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineNumbers {
    Off,
    #[default]
    Absolute,
    Relative,
}

// ─── [lsp] ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct LspCfg {
    pub auto_install: AutoInstall,
    pub idle_shutdown_secs: u64,
    /// Per-language overrides. In TOML: `[lsp.rust]`, `[lsp.python]`, etc.
    #[serde(flatten)]
    pub language: HashMap<String, LangOverride>,
}

impl Default for LspCfg {
    fn default() -> Self {
        Self {
            auto_install: AutoInstall::Prompt,
            idle_shutdown_secs: 300,
            language: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutoInstall {
    #[default]
    Prompt,
    Always,
    Never,
}

/// Per-language LSP overrides. Keys that aren't `server` or `settings` are
/// passed through as extra `InitializeParams` settings.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct LangOverride {
    /// Override which server to use for this language (e.g. `"pyright"`).
    pub server: Option<String>,
    /// Arbitrary settings forwarded to the server's `InitializeParams`.
    pub settings: HashMap<String, toml::Value>,
}

// ─── [keys] ──────────────────────────────────────────────────────────────────

/// Raw keybinding map: chord string → command name.
/// Merged with `builtin_defaults()` at load time so partial user configs work.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyMap(pub HashMap<String, String>);

impl KeyMap {
    /// The built-in default keybindings shipped with luckybird.
    ///
    /// Chord syntax: a plain chord (`ctrl-s`) is global — fires in every mode.
    /// A mode-prefixed chord (`normal:h`, `insert:escape`) only fires in that
    /// mode and takes priority over the global binding.
    pub fn builtin_defaults() -> Self {
        let pairs: &[(&str, &str)] = &[
            // ── Global (all modes) ───────────────────────────────────────
            ("ctrl-p",         "file_picker"),
            ("ctrl-shift-p",   "command_palette"),
            ("ctrl-s",         "save"),
            ("f5",             "reload"),
            ("ctrl-z",         "undo"),
            ("ctrl-y",         "redo"),
            ("ctrl-f",         "search"),
            ("ctrl-h",         "search_replace"),
            ("ctrl-w",         "close_buffer"),
            ("ctrl-q",         "quit"),
            ("f10",            "force_quit"),
            ("ctrl-d",         "select_next_match"),
            ("ctrl-slash",     "toggle_comment"),
            ("ctrl-a",         "select_all"),
            ("ctrl-x",         "cut"),
            ("ctrl-c",         "copy"),
            ("ctrl-v",         "paste"),
            ("ctrl-n",         "new_buffer"),
            ("ctrl-tab",       "next_buffer"),
            ("ctrl-shift-tab", "prev_buffer"),

            // ── Insert mode ──────────────────────────────────────────────
            ("insert:escape",  "enter_normal_mode"),

            // ── Normal mode (vim-style) ──────────────────────────────────
            ("normal:i",       "enter_insert_mode"),
            ("normal:a",       "enter_insert_after"),
            ("normal:o",       "open_line_below"),
            ("normal:O",       "open_line_above"),
            ("normal:h",       "move_left"),
            ("normal:j",       "move_down"),
            ("normal:k",       "move_up"),
            ("normal:l",       "move_right"),
            ("normal:w",       "move_word_forward"),
            ("normal:b",       "move_word_backward"),
            ("normal:0",       "move_line_start"),
            ("normal:$",       "move_line_end"),
            ("normal:x",       "delete_forward"),
            ("normal:u",       "undo"),
            ("normal:ctrl-r",  "redo"),
            ("normal:q",       "quit"),
        ];
        Self(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    /// Overlay `overrides` on top of `self` (user entries win).
    pub fn apply_overrides(&mut self, overrides: &KeyMap) {
        for (k, v) in &overrides.0 {
            self.0.insert(k.clone(), v.clone());
        }
    }
}

// ─── [[plugin]] ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PluginCfg {
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

// ─── [theme] ─────────────────────────────────────────────────────────────────

/// Theme configuration — expanded when lb-tui is implemented.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ThemeCfg {
    /// Name of a named theme bundled with luckybird (e.g. `"gruvbox-dark"`).
    pub name: Option<String>,
}
