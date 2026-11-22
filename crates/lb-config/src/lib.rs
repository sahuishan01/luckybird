#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod diff;
pub mod keymap;
pub mod load;
pub mod schema;

pub use diff::ConfigDelta;
pub use keymap::{parse_chord, Key, KeyChord, KeyParseError, Modifiers};
pub use load::{default_config_path, ConfigError};
pub use schema::{
    AutoInstall, Config, EditorCfg, KeyMap, LangOverride, LineNumbers, LspCfg, PluginCfg,
    ThemeCfg,
};
