# Agent: lb-config

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Load, validate, and serve configuration. TOML for static settings, with serde derives. Hot-reload on `:ConfigReload`. Be fast — config parsing is on the cold start path.

## In scope

- **Config file resolution**: XDG-compliant on Linux/macOS, `%APPDATA%` on Windows. Path is `<config-dir>/luckybird/config.toml`.
- **Schema**: typed via `serde` derives. Sections: `[editor]`, `[lsp]`, `[lsp.<lang>]`, `[keys]`, `[[plugin]]`, `[theme]`.
- **Defaults**: every field has a default. Missing sections are valid.
- **Validation**: clear error messages with path (e.g. `editor.tab_size: expected u8, got "wide"`).
- **Hot reload**: re-parse on demand, diff against prev, emit a `ConfigDelta` for `lb-app` to apply.
- **Theme parsing**: TOML → `Theme` (struct defined in `lb-tui`).
- **Keymap parsing**: `"ctrl-shift-p" = "command_palette"` → typed `KeyChord` + `Command` enum.

## Out of scope

- **Watching the file** for auto-reload — `lb-app` chooses if/when (likely not in v1; only manual `:ConfigReload`).
- **Plugin scripting** — `[[plugin]]` entries declare paths; `lb-plugin` runs them.
- **LSP server install policy *behavior*** — we expose `auto_install: "prompt" | "always" | "never"`; `lb-lspmgr` consumes it.

## Public API surface (target)

```rust
pub struct Config { pub editor: EditorCfg, pub lsp: LspCfg, pub keys: KeyMap, pub plugins: Vec<PluginCfg>, pub theme: Theme }

impl Config {
    pub fn load_default_path() -> Result<Self>;
    pub fn load_from(path: &Path) -> Result<Self>;
    pub fn diff(&self, new: &Config) -> ConfigDelta;
}
```

`Config` implements `Default` so a missing file yields built-in defaults.

## Dependencies

- `serde` (`derive` feature)
- `toml` (no default features beyond `parse`)
- `dirs` (XDG / AppData resolution)
- `lb-tui` only for the `Theme` struct definition — but consider re-exporting via a tiny shared types crate later if cycles emerge

**Banned**: `config` crate (the popular one — too heavy for our needs).

## Performance constraints

- Cold parse of a 5 KB config file: **≤ 2 ms**.
- Memory footprint of a parsed `Config`: **< 64 KB**.
- No I/O after `load_*`. Hot reload is explicit.

## Files owned

```
crates/lb-config/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── schema.rs       # serde structs for every section
│   ├── load.rs         # path resolution + parse
│   ├── keymap.rs       # "ctrl-shift-p" → KeyChord
│   └── diff.rs         # Config diff for hot-reload
└── tests/
    ├── defaults.rs     # missing file → defaults
    └── examples.rs     # parses every config.example.toml
```

## Coordination notes

- The `Command` enum (referenced by keymap) lives in `lb-app`. `lb-config` parses *strings*; the resolution to a typed command happens at registration time in `lb-app`. Don't import `lb-app` here.
- Adding a new config field is cheap, but every new field needs: a default, a schema doc-comment, and a smoke test in `tests/examples.rs`.
- Schema breaking changes need a `CHANGES.md` entry tagged `lb-config` + a migration note (auto-migrate, or document the rename).
