# CHANGES.md — Append-only change log

> Every completed task appends one entry here. Read the format spec below before writing. Read the last ~30 entries before starting any new work.

---

## Format

Each entry is a `##`-level section. The header is the ID + a one-line summary. The body has fixed fields.

```
## LB-XXXX — One-line imperative summary (≤ 70 chars)

- **Date**: YYYY-MM-DD
- **Crate(s)**: lb-core, lb-tui, ...   (comma-separated; or "workspace")
- **Agent**: <agent name or "human">
- **Parent**: LB-XXXX                  (omit if none)
- **Supersedes**: LB-XXXX              (omit if none)

### What changed
2–6 bullets. State the actual change, not the motivation.

### Why
1–3 sentences. The problem, constraint, or request that prompted this.

### Budget impact
- Binary delta: +X KB / -Y KB / no change
- Startup delta: +X ms / no change
- RSS delta: +X MB / no change
- New deps: <crate>@<version> (+Z KB)   or   none

### Follow-ups
- LB-YYYY (if spawned)   or   none
```

## ID rules

- IDs are `LB-` + a **4-digit zero-padded** monotonic integer (`LB-0001`, `LB-0002`, ...).
- The next ID is **(highest existing ID) + 1**. Scan this file top-to-bottom to find it; do not assume.
- Never reuse, never reorder. Append at the **bottom** so chronological order matches numeric order.
- Two agents racing on the same ID: the second one to commit takes the next free number, regardless of who started first.
- If a task is abandoned, still write the entry with `### What changed: abandoned — <reason>` so the ID is not skipped.

## Tagging conventions

- A task touching only one crate: list just that crate.
- A task changing a public API consumed by other crates: list **all** affected crates.
- A task that is workspace-wide (CI, tooling, root `Cargo.toml`): use `workspace`.
- A documentation-only task: prefix the summary with `docs:` (e.g. `LB-0007 — docs: clarify lazy-load rule`).

---

## Entries

## LB-0001 — Initialize project documentation skeleton

- **Date**: 2026-05-07
- **Crate(s)**: workspace
- **Agent**: human

### What changed
- Created `CORE.md` with master rules (budgets, lazy-load law, dep graph, banned deps, no-panic rule, scope discipline).
- Created `CHANGES.md` (this file) with format spec and ID rules.
- Created `docs/agents/` with one .md per planned crate: `lb-core`, `lb-tui`, `lb-syntax`, `lb-lsp`, `lb-lspmgr`, `lb-config`, `lb-plugin`, `lb-app`.

### Why
Before any code is written, every agent needs a fixed set of rules and a coordination channel. The plan in `plans/plan-a-high-performance-lucky-bird.md` defines architecture; these files define how agents interact with it and with each other.

### Budget impact
- Binary delta: no change (no code yet)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- LB-0002 will be the first code task: `Cargo.toml` workspace + `lb-core` skeleton.

## LB-0002 — Workspace skeleton + lb-core (buffer, selection, undo, encoding)

- **Date**: 2026-05-07
- **Crate(s)**: workspace, lb-core
- **Agent**: claude-opus-4-7

### What changed
- Added root `Cargo.toml` (workspace, 8 members, shared workspace deps, release + release-min profiles).
- Implemented `crates/lb-core` fully: `Buffer` (ropey rope), `Edit`/`Change`, `EditError`, `Selection` (SmallVec multi-cursor, `Range` anchor/head), all cursor movement primitives (`move_char_{left,right}`, `move_line_{up,down,start,end}`, `move_doc_{start,end}`, `move_word_{forward,backward}`), branching `UndoTree`, and UTF-8↔UTF-16 offset helpers.
- Added criterion benchmarks (`benches/edit.rs`): insert/delete/load.
- Stubbed remaining 7 crates (`lb-tui`, `lb-syntax`, `lb-lsp`, `lb-lspmgr`, `lb-config`, `lb-plugin`, `lb-app`) so the workspace compiles.
- Fixed `primary_mut` to return `Option<&mut Range>` (borrow-checker constraint).
- Added `PartialEq + Eq` to `Change` so error-path tests can use `assert_eq!`.

### Why
Establishes the workspace layout and the foundational text-editing primitives that every subsequent crate depends on.

### Budget impact
- Binary delta: no binary yet
- Startup delta: N/A
- RSS delta: N/A
- New deps: `ropey@1.6.1` (~60 KB), `smallvec@1.15.1` (~15 KB)

### Follow-ups
- LB-0003: `lb-config` — TOML schema, `Config::load`, `KeyMap` parser.

## LB-0003 — lb-config: TOML schema, loader, keymap parser, hot-reload diff

- **Date**: 2026-05-07
- **Crate(s)**: lb-config
- **Agent**: claude-sonnet-4-6

### What changed
- Implemented full `Config` schema (`schema.rs`): `EditorCfg`, `LspCfg` with per-language `LangOverride` via `#[serde(flatten)]`, `KeyMap`, `PluginCfg`, `ThemeCfg`.
- `KeyMap::builtin_defaults()` ships 17 default bindings; user config overlays them via `apply_overrides` in `Config::from_toml_str`.
- `keymap.rs`: `parse_chord("ctrl-shift-p")` → `KeyChord { modifiers, key }`. Supports all named keys, function keys F1–F24, and named special chars (`minus`, `slash`, `space`, etc.).
- `load.rs`: `Config::load_default()`, `Config::load_from(path)`, `Config::from_toml_str(text)`. XDG/AppData path via `dirs`.
- `diff.rs`: `Config::diff(&new)` → `ConfigDelta` (section-granularity, `None` = unchanged).
- 19 tests across unit + 2 integration test files (`tests/defaults.rs`, `tests/examples.rs`).

### Why
Config must be loadable before any other subsystem initializes; it sits on the cold-start path. `lb-app` (LB-0009) and `lb-lspmgr` (LB-0007) both read it on startup.

### Budget impact
- Binary delta: no binary yet
- Startup delta: N/A
- RSS delta: N/A
- New deps: `dirs@5.0.1` (~5 KB); `toml` was already in workspace

### Follow-ups
- LB-0004: `lb-tui` minimal render loop (open buffer, scroll, edit, save).

## LB-0004 — lb-tui: terminal management, event translation, render loop, status line

- **Date**: 2026-05-07
- **Crate(s)**: lb-tui
- **Agent**: claude-sonnet-4-6

### What changed
- `term.rs`: `Tui` — `enter()`, `leave()`, `next_event()` (blocking crossterm read), `render(view)`, `size()`. Drop-based cleanup with `active` guard prevents double-teardown.
- `event.rs`: `EditorEvent`, `KeyCode`, `KeyModifiers`, `MouseEvent` — own types independent of lb-config. Full translation from crossterm event types.
- `render.rs`: `View<'a>` (buffer, selection, highlights placeholder, diagnostics placeholder, status, scroll_offset). `EditorWidget` (ratatui `Widget`), `render_frame()`. Renders gutter with line numbers, text area with cursor-line + selection highlight + inverted-block cursor, status bar. Unicode-width-aware column calculation via `unicode-width`. `Highlight`/`Diagnostic` types defined; not yet applied (LB-0005).
- `statusline.rs`: `StatusLine` with `left_text()` / `right_text()` + prompt overlay field.
- 12 tests: event translation, status line formatting, render smoke via `TestBackend`, gutter width, visual column.

### Why
The TUI layer is the user-visible surface; lb-app (LB-0008) needs it to wire up the interactive edit loop and measure the startup budget end-to-end.

### Budget impact
- Binary delta: no binary yet
- New deps: `ratatui@0.29.0` (~350 KB), `crossterm@0.28.1` (~80 KB), `unicode-width@0.2.0` (~15 KB)

### Follow-ups
- LB-0005: `lb-syntax` — tree-sitter grammar loader, highlight engine, `grammars.toml` registry.

## LB-0005 — lb-syntax: tree-sitter grammar loader, highlight engine, registry

- **Date**: 2026-05-07
- **Crate(s)**: lb-syntax
- **Agent**: claude-sonnet-4-6
- **Parent**: LB-0004

### What changed
- `grammars.toml` (embedded via `include_str!`) with 30 language entries mapping extensions, C symbol names, and shebangs.
- `registry.rs`: `load()` → `Registry`; `detect_by_path()` (extension match); `detect_by_shebang()` (handles `#!/usr/bin/env python3` style).
- `loader.rs`: `GrammarLoader` — dlopen grammar `.so`/`.dll` via `libloading`, cache loaded `Language` objects; graceful `LoadError::NotFound` when file absent.
- `highlight.rs`: `Highlighter` — tree-sitter `Parser` + `Query` + `QueryCursor`; `highlight(&str) → Vec<HighlightSpan>`; uses `StreamingIterator` API.
- `lib.rs`: `SyntaxManager` — owns loader + registry + query dir; `detect()`, `layer_for()` building `SyntaxLayer` (degrades to plain-text when grammar absent).
- 9 tests: registry loading, path detection, shebang detection, graceful degradation.

### Why
Provides syntax highlighting for all buffers. Lazy-load constraint met: nothing touches disk until `SyntaxManager::layer_for` is first called.

### Budget impact
- Binary delta: +~50 KB (tree-sitter runtime, no grammars bundled)
- New deps: `tree-sitter@0.24.7`, `libloading@0.8.9`, `streaming-iterator@0.1.9`

### Follow-ups
- LB-0006: `lb-lsp` — JSON-RPC transport, LSP client.

## LB-0006 — lb-lsp: JSON-RPC transport and LSP client

- **Date**: 2026-05-07
- **Crate(s)**: lb-lsp
- **Agent**: claude-sonnet-4-6
- **Parent**: LB-0005

### What changed
- `transport.rs`: `write_message` / `read_message` — Content-Length framing over child process stdio using `tokio::io`.
- `client.rs`: `LspClient::spawn(cmd, args, root)` — spawns server, starts reader + writer tokio tasks, sends `initialize` + `initialized`. Methods: `did_open`, `did_change`, `did_close`, `hover`, `completion`, `goto_definition`, `shutdown`. Pending-request map via `oneshot` channels; 10-second timeout per request.
- `lib.rs`: `ServerEvent` enum (`Diagnostics`, `Exited`).
- Message dispatch: routes `textDocument/publishDiagnostics` to `events` channel; logs `window/logMessage`.

### Why
LSP clients need to communicate with language servers over a well-specified JSON-RPC protocol. All server communication is async via tokio channels so the UI thread is never blocked.

### Budget impact
- Binary delta: +~30 KB (lsp-types, serde_json already shared)
- New deps: `lsp-types@0.97.0`, `url@2.5.8` (transitive, now direct)

### Follow-ups
- LB-0007: `lb-lspmgr` — registry, resolver, downloader, spawner.

## LB-0007 — lb-lspmgr: server registry, PATH resolver, downloader, spawner

- **Date**: 2026-05-07
- **Crate(s)**: lb-lspmgr
- **Agent**: claude-sonnet-4-6
- **Parent**: LB-0006

### What changed
- `registry.toml` (embedded) — 30 server entries covering all major languages.
- `registry.rs`: `load()` → `ServerRegistry`; `server_for_language()`.
- `resolve.rs`: `Resolver::resolve()` — PATH lookup via `which::which`, then managed-cache lookup at `~/.local/share/luckybird/lsp/<server>/`.
- `download.rs`: `download_and_install()` — `ureq` GET, SHA-256 verify, extract (`gz`/`zip`/`tar.gz`/raw), `chmod +x` on Unix.
- `spawn.rs`: `spawn_server()` — calls `LspClient::spawn` with resolved binary.
- `lib.rs`: `LspManager` — owns all active clients; `get_or_spawn(lang)` lazily spawns; `needs_install(lang)` for TUI prompts; `shutdown_all()`.
- 8 tests: registry loading, server-for-language, resolver not-found, write-raw.

### Why
Mason-style LSP management: prefer what's on PATH, fall back to a managed per-user cache, surface install prompts to the UI.

### Budget impact
- Binary delta: +~120 KB (ureq, sha2, flate2, zip, tar)
- New deps: `which@6.0.3`, `ureq@2.12.1`, `sha2@0.10.9`, `flate2@1.1.9`, `zip@2.4.2`, `tar@0.4.45`

### Follow-ups
- LB-0008: `lb-plugin` — Lua runtime.

## LB-0008 — lb-plugin: Lua 5.4 runtime with sandbox and lb.* API

- **Date**: 2026-05-07
- **Crate(s)**: lb-plugin
- **Agent**: claude-sonnet-4-6
- **Parent**: LB-0007

### What changed
- `sandbox.rs`: removes `os.execute`, `os.exit`, `os.rename`, `os.remove`, `io.open`, `io.popen`, `loadfile`, `dofile`, partial `debug` table.
- `api.rs`: `LbApi` installs `lb` global table: `lb.notify`, `lb.event.on(name, fn)`, `lb.cmd.register(name, fn)`, `lb.keymap.set(chord, cmd)`, `lb.buffer` placeholder.
- `runtime.rs`: `PluginRuntime` — `OnceLock<Lua>` deferred init; `init_if_needed(paths)` no-ops when paths empty; `fire_event(name)` no-ops before init.
- 9 tests covering sandbox, API installation, and event/keymap registration.

### Why
Plugins extend the editor without touching core data structures. Lua 5.4 vendored via mlua means no system Lua required; the sandbox prevents filesystem/shell access from plugin code.

### Budget impact
- Binary delta: +~1.5 MB (mlua + vendored Lua 5.4 C source)
- Startup delta: zero (OnceLock deferred init)
- New deps: `mlua@0.9.9` (lua54, vendored)

### Follow-ups
- LB-0009: `lb-app` — complete binary.

## LB-0009 — lb-app: complete editor binary (event loop, commands, file I/O)

- **Date**: 2026-05-07
- **Crate(s)**: lb-app
- **Agent**: claude-sonnet-4-6
- **Parent**: LB-0008

### What changed
- `commands.rs`: `Command` enum (30 variants); `from_name(&str) → Option<Command>` for keymap dispatch.
- `app.rs`: `App` struct with `OpenBuffer` (buffer, selection, undo, syntax spans, LSP version, scroll); `handle_event` → key dispatch; `execute(Command)` — full movement, insert, delete, undo/redo, save, quit; `render()` → `lb_tui::View`; `poll_lsp_events()` non-blocking LSP drain; `key_to_chord_str` converting `KeyEvent2` to config chord strings.
- `event_loop.rs`: `spawn_event_thread()` — background OS thread reading crossterm events via `lb_tui::read_event()`, forwarding to unbounded tokio channel; `run(app, rx)` — main async loop with render-on-every-event.
- `main.rs`: lexopt CLI (`luckybird [FILE]...`, `--help`, `--version`, `--headless`); tokio `current_thread` runtime; file-based tracing log; plugin init; TUI enter/leave.
- Added `read_event()` and `KeyEvent2` to lb-tui public API.

### Why
Wires all subsystems into a working editor binary. The binary is 2.9 MB stripped on Windows — within the 8 MB budget.

### Budget impact
- Binary delta: **2.9 MB total** (release, strip=symbols, lto=fat)
- Startup delta: <30 ms cold (LSP/syntax/Lua not initialised on startup)
- New deps: `lexopt@0.3.2`, `anyhow@1.0.102`, `tracing-subscriber@0.3.23`

### Follow-ups
- LB-0010

## LB-0010 — Fix unresponsive shortcuts and unexitable editor

- **Date**: 2026-05-07
- **Crate(s)**: lb-tui, lb-config, lb-app
- **Agent**: claude-opus-4-7
- **Parent**: LB-0009

### What changed
- `lb-tui/event.rs`: `translate_event` now drops `KeyEventKind::{Repeat,Release}` events. On Windows crossterm fires Press AND Release for every keystroke, which caused every action (typing, movement, save, quit) to fire twice.
- `lb-config/schema.rs`: added `("ctrl-shift-q", "force_quit")` to `builtin_defaults()` so there is an unconditional way out.
- `lb-app/app.rs`: `Quit` on a dirty buffer now arms a `quit_armed` flag and prompts; a second consecutive `Ctrl-Q` discards and exits. Any other command clears the arming flag.
- `lb-app/app.rs`: replaced the silent `_ =>` catch-all in `execute()` with a status-bar message (`"<Cmd>: not yet implemented"`) so stub commands give visible feedback instead of looking broken.

### Why
User reported "none of the shortcuts work, and I am unable to exit the editor". Three independent bugs combined: (1) double-firing Press+Release events, (2) `Ctrl-Q` on a dirty buffer told the user to "press Ctrl-Q to force-quit" but `Ctrl-Q` was bound to `quit`, not `force_quit` — infinite loop, (3) all the chord stubs (`file_picker`, `command_palette`, `select_next_match`, `toggle_comment`, etc.) fell through the catch-all in `execute()` so the keys looked dead.

### Budget impact
- Binary delta: no change (logic only)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- LB-0011, LB-0012

## LB-0011 — Load builtin keymap defaults when no config file exists

- **Date**: 2026-05-08
- **Crate(s)**: lb-config
- **Agent**: claude-opus-4-7
- **Parent**: LB-0010

### What changed
- `Config::default()` now uses `KeyMap::builtin_defaults()` for the `keys` field instead of the empty `KeyMap::default()`.

### Why
On a fresh install with no `~/.config/luckybird/config.toml`, `Config::load_default` falls through to `Config::default()`. The previous default returned an empty keymap, so every config-bound chord (Ctrl+S, Ctrl+Q, F10, …) silently failed to dispatch — only the hardcoded fallbacks in `handle_key` (typing, arrows, Backspace) ever worked. The user observed F10 producing the correct chord string but no exit.

### Budget impact
- Binary delta: no change
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- LB-0012

## LB-0012 — Modal editing (Normal / Insert), vim-style key map

- **Date**: 2026-05-08
- **Crate(s)**: lb-tui, lb-config, lb-app
- **Agent**: claude-opus-4-7
- **Parent**: LB-0011

### What changed
- Added `Mode` enum (`Normal`, `Insert`) with `label()` and `prefix()`. Default is `Normal`. New `App::mode` field, threaded into `StatusLine.mode` so the active mode appears as `[NORMAL]` / `[INSERT]` on the left of the status bar.
- New commands: `EnterNormalMode`, `EnterInsertMode`, `EnterInsertAfter`, `OpenLineBelow`, `OpenLineAbove`. All wired into `from_name()` and `App::execute()`.
- `handle_key` now looks up `<mode>:<chord>` first, then falls back to the global `<chord>`. In Normal mode unbound printable keys are no-ops; only Insert mode inserts characters.
- `key_to_chord_str`: chord encoding rewritten so the character itself encodes case (`i` ≠ `I`) and shifted-symbol glyphs (`$`, not `shift-4`). Shift modifier is only emitted for non-Char keys (`shift-tab`, `shift-f5`). Ctrl/Alt + letter is always lowercased so `Ctrl-Q` matches `ctrl-q` even when Windows tags the event with a spurious shift.
- `KeyMap::builtin_defaults()` extended with vim-style normal-mode bindings (`h j k l w b 0 $ x u i a o O q ctrl-r`), `insert:escape → enter_normal_mode`, and a clarifying doc-comment about chord scope.

### Why
User asked to convert the editor from modeless to modal. The previous design had every printable key always inserting; there was no command-key surface aside from Ctrl/Alt chords. The chord encoding also needed a rework because the previous "drop shift for letters when ctrl/alt held" rule made `i` indistinguishable from `I` — fatal for vim-style commands. New rule: the Char value carries case/glyph; shift is only meaningful for non-Char keys.

### Budget impact
- Binary delta: negligible (~few KB of static data + match arms)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- none

---

## LB-0013 — Harden Lua sandbox and LSP download verification

- **Date**: 2026-11-22
- **Crate(s)**: lb-plugin, lb-lspmgr
- **Agent**: Claude

### What changed
- `lb-plugin`: Lua state is built with a whitelist of stdlibs (coroutine, table, string, utf8, math, os). `io`, `package`/`require`, `debug` and `collectgarbage` are never exposed; `os` is reduced to `clock/time/date/difftime`; `load` is restricted to text chunks.
- `lb-lspmgr`: `download_and_install` now requires a SHA-256, requires an https URL, rejects binary names containing path components, and caps downloads at 512 MB.

### Why
Security review: the blocklist sandbox left `package.loadlib`, `io.output` and `debug.*` reachable, and the checksum was optional, contradicting CORE.md section 6.

### Budget impact
- Binary delta: no change expected (not measured)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Registry entries need URLs and SHA-256 values before the downloader is wired up.

---

## LB-0014 — Shrink release binary from 2.9 MB to 1.2 MB

- **Date**: 2026-11-22
- **Crate(s)**: workspace, lb-syntax
- **Agent**: Claude

### What changed
- `tracing-subscriber`: dropped default features (`env-filter` pulled in `regex`).
- Release profile uses `opt-level = "z"`; `tokio` no longer enables `net`.
- `vendor/tree-sitter` is tree-sitter 0.24.7 with `regex::bytes` replaced by `regex-lite` (`#match?` predicates now only match valid UTF-8 text); wired in through `[patch.crates-io]`.

### Why
User asked to reduce binary size. `regex` was ~470 KB and only reachable through tree-sitter query predicates and the unused env-filter.

### Budget impact
- Binary delta: -1.7 MB (2,920,448 -> 1,240,576 bytes)
- Startup delta: not measured
- RSS delta: not measured
- New deps: regex-lite@0.1 (+23 KB); removed regex, regex-automata, regex-syntax, aho-corasick

### Follow-ups
- Rebase `vendor/tree-sitter` when upgrading tree-sitter.
- Remaining size: std ~226 KB, toml_edit+winnow ~100 KB, Lua ~100 KB.

---

## LB-0015 — Replace Lua plugins with compile-time Rust plugins

- **Date**: 2026-11-22
- **Crate(s)**: lb-plugin, lb-app
- **Agent**: Claude
- **Supersedes**: LB-0013 (sandbox part; the sandbox no longer exists)

### What changed
- `lb-plugin` now exposes a `Plugin` trait, `PluginApi` (`bind_key`, `on_event`) and `PluginRuntime` (`register`, `fire_event`, `command_for`). `mlua` and the Lua sandbox are removed.
- `lb-app` looks up plugin keybindings with `command_for`; `[[plugin]]` config entries are ignored with a warning. Removed the now-unused `shellexpand_tilde`.
- CORE.md section 6 and `docs/agents/lb-plugin.md` rewritten for Rust plugins.

### Why
User asked for Rust plugins instead of Lua to shrink the binary; Lua and `mlua` cost ~100 KB and a sandbox attack surface.

### Budget impact
- Binary delta: -292 KB (1,241,088 -> 948,736 bytes); total from 2.9 MB is -2.0 MB
- Startup delta: not measured
- RSS delta: not measured
- New deps: none (removed mlua, mlua-sys, vendored Lua)

### Follow-ups
- No plugin exists yet; add handlers with editor access when the first one needs it.

---

## LB-0016 — Add actions overlay with Save As and language installers

- **Date**: 2026-11-22
- **Crate(s)**: lb-app, lb-tui, lb-syntax
- **Agent**: Claude

### What changed
- `lb-app`: `Space` (Normal) / `F1` (any mode) opens an actions overlay (`overlay.rs`, rendered by `lb-tui/overlay.rs`): Save, Save as, Open, Install syntax highlighting, Install language server, Quit. Keys: `j`/`k`, Enter, digits, Esc. No `:` command line.
- `installer.rs` + `compile.rs`: grammars are built from source without the tree-sitter CLI (`git clone`, then `cl`/`gcc`/`clang` compile `src/parser.c` and the scanner into a shared library, found via PATH or vswhere/vcvars64.bat on Windows; `highlights.scm` copied); servers use `rustup`/`npm`/`go`/`cargo`. Languages are validated against the grammar registry and command lines come from fixed tables. Runs on a background thread; progress reaches the status line through a channel selected in the event loop.
- `lb-tui`: Hint row mentions Space/F1; new `overlay` module draws the card. `lb-syntax`: `loader::lib_filename` is now public.

### Why
User wanted an in-editor way to install tree-sitter grammars and language servers, and Save As.

### Budget impact
- Binary delta: +111 KB (948,224 -> 1,059,328 bytes)
- Startup delta: not measured
- RSS delta: not measured
- New deps: none

### Follow-ups
- Installs were not executed end to end (they download and build); open the menu and pick Install syntax highlighting.
- Re-highlighting open buffers after an install is not automatic.

---

## LB-0017 — Add reload command (F5)

- **Date**: 2026-11-22
- **Crate(s)**: lb-app, lb-config, lb-tui
- **Agent**: Claude
- **Parent**: LB-0016

### What changed
- New `reload` command bound to `F5` and listed in the actions menu (File > Reload file). A clean buffer is re-read from disk; a buffer with unsaved edits keeps its text and only re-runs syntax highlighting.
- Hint row mentions `F5 reload`.

### Why
A grammar installed from the menu is only picked up when the file is reopened.

### Budget impact
- Binary delta: ~+8 KB (1,059,328 -> 1,067,008 bytes, includes the C-compile installer change)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Auto-reload highlighting when a grammar install finishes.

---

## LB-0018 — Upgrade vendored tree-sitter to 0.25.10; log installer activity

- **Date**: 2026-11-22
- **Crate(s)**: workspace, lb-app
- **Agent**: Claude
- **Supersedes**: LB-0014 (the vendored tree-sitter version only)

### What changed
- `vendor/tree-sitter` is now 0.25.10 (still `regex` swapped for `regex-lite`) and the workspace pins `tree-sitter = "=0.25.10"`.
- The installer logs progress, every command it runs, and full stdout/stderr of failed commands to `luckybird.log` (`%LOCALAPPDATA%\luckybird\`).
- The ignored installer test now also checks that the Python grammar produces highlights.

### Why
Grammars built from current repos use ABI 15; tree-sitter 0.24 only accepts ABI 13-14 ("Incompatible language version 15"), so highlighting stayed off after a successful install.

### Budget impact
- Binary delta: +21 KB (1,068,544 -> 1,089,536 bytes)
- Startup delta: not measured
- RSS delta: not measured
- New deps: none

### Follow-ups
- Re-apply the regex-lite patch when upgrading tree-sitter again.

---

## LB-0019 — Install grammars while the old one is loaded

- **Date**: 2026-11-22
- **Crate(s)**: lb-app, lb-syntax
- **Agent**: Claude
- **Parent**: LB-0018

### What changed
- The grammar is compiled in a temp folder and swapped into place; if the old library is loaded (Windows `LNK1104`), it is renamed to `<lang>.old` first. `.lib`/`.exp` files no longer land in the grammars folder.
- `SyntaxManager::forget_grammar` / `GrammarLoader::forget` drop the cached library; F5 calls it so a reinstalled grammar is picked up without restarting.
- The ignored installer test reinstalls Python while it is loaded.

### Why
Log showed `LINK : fatal error LNK1104: cannot open file ...\python.dll` when reinstalling while the editor had the DLL loaded.

### Budget impact
- Binary delta: negligible
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Delete stale `*.old` grammar files on startup.

---

## LB-0020 — Draw syntax highlighting colours

- **Date**: 2026-11-22
- **Crate(s)**: lb-tui, lb-syntax, lb-app
- **Agent**: Claude
- **Parent**: LB-0018

### What changed
- `lb-tui`: `Highlight` is now `{start, end, rgb}`; `render_text` draws spans as foreground colours (selection and cursor-line backgrounds are kept).
- `lb-syntax`: `Highlighter::highlight` flattens nested captures into sorted, non-overlapping spans (innermost wins; later capture wins on equal ranges); `SyntaxLayer::capture_name` added.
- `lb-app`: each buffer keeps its highlighter and re-highlights whenever its text changes (undo/redo now bump `lsp_version` too). New `theme.rs` maps capture names to colours (keyword, string, comment, function, type, number/constant, property, escape/operator, builtin variables); spans without a colour are dropped when highlighting.

### Why
The grammar loaded and produced spans, but the renderer never used them (placeholder since LB-0005), so nothing was ever coloured.

### Budget impact
- Binary delta: not measured (small)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Theme is hard-coded; make it configurable.
- Highlighting re-parses the whole buffer after every edit (fine for normal files; make it incremental for very large ones).

---

## LB-0021 — Resolve overlapping captures by query pattern order

- **Date**: 2026-11-22
- **Crate(s)**: lb-syntax
- **Agent**: Claude
- **Parent**: LB-0020

### What changed
- Highlight captures carry their query pattern index; for identical ranges the later pattern wins when overlaps are flattened.
- The ignored real-grammar test checks that Python function definition and call names resolve to `function`.

### Why
tree-sitter returns matches ordered by position, not pattern, so the catch-all `(identifier) @variable` could beat `@function` and function names stayed uncoloured.

### Budget impact
- Binary delta: negligible
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- none

---

## LB-0022 — Give each function name its own colour

- **Date**: 2026-11-22
- **Crate(s)**: lb-app
- **Agent**: Claude
- **Parent**: LB-0020

### What changed
- `theme.rs`: `is_function_name` and `assign_name_colors` pick one of 12 palette colours per distinct function/method name (hash slot with forward probing, so up to 12 names never share a colour and the result does not depend on order). Purple, green and grey are excluded because keywords, strings and comments use them.
- `app.rs`: `color_spans` colours `function*`/`method*` captures (except builtin/macro) by name; a definition and its calls share a colour.

### Why
All function names were one blue, so `foo` and `foo2` could not be told apart at a glance.

### Budget impact
- Binary delta: negligible
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Beyond 12 distinct functions colours repeat; the palette could grow or variables could get the same treatment.

---

## LB-0023 — Per-name colours for functions, types and variables in every language

- **Date**: 2026-11-22
- **Crate(s)**: lb-app, lb-syntax
- **Agent**: Claude
- **Parent**: LB-0022
- **Supersedes**: LB-0022 (function-only colouring)

### What changed
- `theme.rs`: names are classified as Function, Type or Variable from the capture name (`function*`/`method*`, `type`/`class`/`namespace`/`module`/`constructor`/..., `variable`/`parameter`/`property`/`field`/`attribute`/`constant`); builtins, macros and special forms keep fixed colours. Each kind has its own 12-colour palette (cool for functions, warm for types, pastel for variables); every distinct name gets its own colour, with probing so up to 12 names of a kind never collide.
- `lb-syntax`: overlapping captures on the same node are ranked by specificity (plain `variable` loses to everything; `function.builtin` beats `function`), with query order only as a tiebreak. The old "later pattern wins" rule broke Go, whose query lists the `variable` catch-all last.
- `lb-syntax`: a `(identifier) @variable` catch-all is prepended to every query that accepts it (Rust never captures local variables).
- New ignored test builds Python, Rust, JavaScript, Go and C grammars and checks two types, two functions and two variables in each get different colours.

### Why
User wanted same-looking names (`foo` vs `foo2`) to be distinguishable for every kind of name in every language.

### Budget impact
- Binary delta: see release size in the next entry
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Languages other than the five tested may use other capture names (e.g. `@entity.name.function`); extend `name_kind` as needed.
- More than 12 names of one kind repeat colours.

---

## LB-0024 — Searchable actions menu, opened with Ctrl-Space

- **Date**: 2026-11-22
- **Crate(s)**: lb-app, lb-tui
- **Agent**: Claude
- **Parent**: LB-0016

### What changed
- The actions menu opens with `Ctrl-Space` (also reported as NUL by some terminals) or `F1`, in any mode; plain `Space` in Normal mode no longer opens it.
- The menu and the language pickers filter as you type (case-insensitive, each word must appear). Movement is Up/Down, Tab/Shift-Tab, Ctrl-N/P, Ctrl-J/K; `j`/`k` and the 1-9 shortcuts were removed because letters and digits are search text. `Esc` clears the search, then closes.
- The search text is shown in the overlay's input line; empty results show "no matches".

### Why
User asked for Ctrl-Space instead of Space and for searchable options.

### Budget impact
- Binary delta: ~+8 KB (1,118,208 -> 1,126,912 bytes)
- Startup delta: no change
- RSS delta: no change
- New deps: none

### Follow-ups
- Ctrl-Space was not tested on a real terminal; F1 is the fallback.
