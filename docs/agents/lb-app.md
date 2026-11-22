# Agent: lb-app

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

The binary. Owns the event loop, command dispatch, file I/O, and the lifetime of every other subsystem. Wires everything together while keeping the cold start path empty of heavy work.

## In scope

- **`main`**: argument parsing (`lexopt`), config load, terminal init, event loop.
- **Event loop**: pulls events from `lb-tui`, dispatches commands, applies edits to `lb-core`, drives `lb-lsp` futures, redraws.
- **Command registry**: typed `Command` enum + a name → handler map. Used by keymaps, command palette, `:`-prompt, plugins.
- **File I/O**: read into `Buffer`, write from `Buffer`. Handles encoding, BOM, line endings (LF/CRLF preservation).
- **Buffer manager**: list of open buffers, active focus, switching.
- **Lazy-init triggers**: starts `SyntaxEngine` on first buffer open of a known language; starts `LspManager`-spawned clients on first relevant edit; starts `PluginRuntime` on first plugin event.
- **LSP routing**: maps incoming LSP responses to the right buffer/cursor, computes deltas for `didChange`, gates by capabilities.
- **Headless mode**: `luckybird --headless +q` (no TUI). Used for tests, scripts, benchmarks.

## Out of scope

- **Rendering details** (`lb-tui`).
- **Buffer mechanics** (`lb-core`).
- **Protocol details** (`lb-lsp`).
- **Server install / spawn** (`lb-lspmgr`).

## Public API

This crate is a binary, not a library. Everything in `src/` is private. The only public "API" is the CLI:

```
luckybird [FILE]...
luckybird --headless ARGS...
luckybird --version
luckybird --config <PATH>
```

## Dependencies

- All other internal crates: `lb-core`, `lb-tui`, `lb-syntax`, `lb-lsp`, `lb-lspmgr`, `lb-config`, `lb-plugin`.
- `tokio` (current_thread)
- `lexopt` (arg parsing — never `clap` derive)
- `tracing`, `tracing-subscriber` (file appender only — never stderr in TUI mode)

## Performance constraints

- Cold start (`luckybird --headless +q`): **≤ 30 ms**, measured by `hyperfine`.
- Cold start with a 1 KB file argument: **≤ 35 ms**.
- Per-frame CPU during idle (no input, redraw not needed): **0 % CPU**. Block on the event source. No timers, no polling.

## Files owned

```
crates/lb-app/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── app.rs          # App struct, owns subsystems
│   ├── event_loop.rs   # the central select! loop
│   ├── commands.rs     # Command enum + registry
│   ├── io.rs           # file open/save with encoding handling
│   ├── buffers.rs      # buffer list + focus
│   ├── lsp_routing.rs  # tie LspClient responses back to UI state
│   └── lazy.rs         # OnceCell trigger helpers
└── tests/
    ├── headless.rs     # cold-start budget, smoke tests
    ├── lsp_rust.rs     # opens a Rust file, asserts rust-analyzer attaches
    └── lsp_install.rs  # mocked downloader fixture
```

## Coordination notes

- This is the only crate that may `expect()` at startup — and only for things genuinely unrecoverable (no terminal, no home dir). Every `expect` has a meaningful message.
- The `Command` enum is the lingua franca between keymaps, plugins, palette, and `:` prompt. Adding a new command requires: enum variant, handler in `commands.rs`, default key (if any) in `lb-config` defaults, and palette label.
- The lazy-init helpers in `lazy.rs` are the *only* place where heavy subsystems are first touched. If you find yourself initializing `SyntaxEngine` somewhere else, you're violating CORE.md § 2.1 — route through `lazy.rs`.
- LSP idle GC runs from this crate's event loop, not from `lb-lsp` or `lb-lspmgr`. Each loop tick checks `last_activity` of clients against `idle_shutdown_secs` and asks `LspManager` to drop them.
