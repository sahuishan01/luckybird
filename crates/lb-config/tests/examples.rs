use lb_config::{AutoInstall, Config, LineNumbers};

const FULL_EXAMPLE: &str = r#"
[editor]
line_numbers = "relative"
tab_size = 2
expand_tabs = false
auto_save = true
scroll_off = 8
wrap = true

[lsp]
auto_install = "always"
idle_shutdown_secs = 120

[lsp.rust]
server = "rust-analyzer"

[lsp.rust.settings]
"rust-analyzer.check.command" = "clippy"

[lsp.python]
server = "pyright"

[keys]
"ctrl-p" = "file_picker"
"ctrl-shift-p" = "command_palette"
"ctrl-q" = "quit"

[[plugin]]
path = "~/.config/luckybird/plugins/git-gutter.lua"
enabled = true

[[plugin]]
path = "~/.config/luckybird/plugins/surround.lua"

[theme]
name = "gruvbox-dark"
"#;

#[test]
fn parse_full_example() {
    let cfg = Config::from_toml_str(FULL_EXAMPLE).unwrap();

    // Editor
    assert_eq!(cfg.editor.line_numbers, LineNumbers::Relative);
    assert_eq!(cfg.editor.tab_size, 2);
    assert!(!cfg.editor.expand_tabs);
    assert!(cfg.editor.auto_save);
    assert_eq!(cfg.editor.scroll_off, 8);
    assert!(cfg.editor.wrap);

    // LSP top-level
    assert_eq!(cfg.lsp.auto_install, AutoInstall::Always);
    assert_eq!(cfg.lsp.idle_shutdown_secs, 120);

    // LSP per-language
    let rust = cfg.lsp.language.get("rust").unwrap();
    assert_eq!(rust.server.as_deref(), Some("rust-analyzer"));
    assert!(rust.settings.contains_key("rust-analyzer.check.command"));

    let python = cfg.lsp.language.get("python").unwrap();
    assert_eq!(python.server.as_deref(), Some("pyright"));

    // Plugins
    assert_eq!(cfg.plugins.len(), 2);
    assert!(cfg.plugins[0].enabled);
    assert!(cfg.plugins[0].path.contains("git-gutter"));
    assert!(cfg.plugins[1].enabled); // defaults to true

    // Theme
    assert_eq!(cfg.theme.name.as_deref(), Some("gruvbox-dark"));
}

#[test]
fn diff_detects_changes() {
    let old = Config::from_toml_str("").unwrap();
    let new = Config::from_toml_str("[editor]\ntab_size = 2").unwrap();
    let delta = old.diff(&new);
    assert!(delta.editor.is_some());
    assert!(delta.lsp.is_none());
    assert!(delta.theme.is_none());
}

#[test]
fn diff_empty_when_identical() {
    let a = Config::from_toml_str(FULL_EXAMPLE).unwrap();
    let b = Config::from_toml_str(FULL_EXAMPLE).unwrap();
    assert!(a.diff(&b).is_empty());
}
