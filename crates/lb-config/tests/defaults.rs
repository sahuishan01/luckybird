use lb_config::{AutoInstall, Config, LineNumbers};

#[test]
fn empty_toml_yields_defaults() {
    let cfg = Config::from_toml_str("").unwrap();
    assert_eq!(cfg.editor.tab_size, 4);
    assert!(cfg.editor.expand_tabs);
    assert!(!cfg.editor.auto_save);
    assert_eq!(cfg.editor.scroll_off, 5);
    assert_eq!(cfg.editor.line_numbers, LineNumbers::Absolute);
    assert_eq!(cfg.lsp.auto_install, AutoInstall::Prompt);
    assert_eq!(cfg.lsp.idle_shutdown_secs, 300);
    assert!(cfg.plugins.is_empty());
}

#[test]
fn missing_file_yields_defaults() {
    let cfg = Config::load_default().unwrap();
    assert_eq!(cfg.editor.tab_size, 4);
}

#[test]
fn builtin_keybindings_present() {
    let cfg = Config::from_toml_str("").unwrap();
    assert_eq!(cfg.keys.0.get("ctrl-p").map(String::as_str), Some("file_picker"));
    assert_eq!(cfg.keys.0.get("ctrl-shift-p").map(String::as_str), Some("command_palette"));
    assert_eq!(cfg.keys.0.get("ctrl-s").map(String::as_str), Some("save"));
    assert_eq!(cfg.keys.0.get("ctrl-z").map(String::as_str), Some("undo"));
}

#[test]
fn user_key_overrides_default() {
    let toml = r#"
[keys]
"ctrl-s" = "save_all"
"#;
    let cfg = Config::from_toml_str(toml).unwrap();
    // User override wins
    assert_eq!(cfg.keys.0.get("ctrl-s").map(String::as_str), Some("save_all"));
    // But other defaults are still there
    assert_eq!(cfg.keys.0.get("ctrl-z").map(String::as_str), Some("undo"));
}

#[test]
fn partial_editor_section() {
    let toml = r#"
[editor]
tab_size = 2
"#;
    let cfg = Config::from_toml_str(toml).unwrap();
    assert_eq!(cfg.editor.tab_size, 2);
    // Other fields kept their defaults
    assert!(cfg.editor.expand_tabs);
    assert!(!cfg.editor.auto_save);
}
