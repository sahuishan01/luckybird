# Agent: lb-lsp

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Be the LSP **client**. Speak JSON-RPC over stdio with running language-server processes. Translate buffer events into LSP notifications, surface diagnostics/completions/hovers/definitions/code actions back to the editor.

## In scope

- **JSON-RPC framing**: Content-Length headers, message framing, request/response correlation.
- **Lifecycle**: `initialize` → `initialized` → notifications → `shutdown`/`exit`. Per-server state machine.
- **Document sync**: `didOpen`, `didChange` (incremental), `didSave`, `didClose`.
- **Requests**: hover, completion (+ resolve), signature help, definition, references, code action, rename, formatting, semantic tokens.
- **Diagnostics**: receive, store per-buffer, deliver to `lb-app`.
- **Capabilities negotiation**: only send what the server advertises.
- **Position encoding**: handle UTF-16 (the spec default) and UTF-8 / UTF-32 (servers that opt in).
- **Workspace folders**: support multi-root.

## Out of scope

- **Spawning the server process** — that's `lb-lspmgr`. This crate is given a `Child` (or `AsyncRead + AsyncWrite`) and drives the protocol.
- **Server discovery / install** — `lb-lspmgr`.
- **UI** — completions appear via `lb-tui`'s picker; we just provide the data.

## Public API surface (target)

```rust
pub struct LspClient { /* per-server: stdio, pending requests, capabilities */ }

impl LspClient {
    pub async fn new(child: Child, root: PathBuf) -> Result<Self>;
    pub async fn did_open(&mut self, uri: Url, lang: &str, text: &str) -> Result<()>;
    pub async fn did_change(&mut self, uri: Url, change: &Change) -> Result<()>;
    pub async fn hover(&mut self, uri: Url, pos: Position) -> Result<Option<Hover>>;
    pub async fn completion(&mut self, uri: Url, pos: Position) -> Result<Vec<CompletionItem>>;
    // ...
    pub fn diagnostics(&self, uri: &Url) -> &[Diagnostic];
    pub async fn shutdown(self) -> Result<()>;
}
```

## Dependencies

- `tokio` (current_thread runtime)
- `lsp-types` (the canonical types crate)
- `serde`, `serde_json` (no default-features)
- `tower-lsp`'s **client** half is acceptable if it doesn't drag the server scaffolding. Otherwise hand-roll using `lsp-types` + `tokio::io`.
- `lb-core` (to map buffer offsets to LSP positions)

## Performance constraints

- A `didChange` notification per keystroke must complete (queued, not awaited): **≤ 50 µs** of editor-thread time.
- Hover round-trip: bounded by the server, but our overhead must be **≤ 1 ms**.
- One `LspClient` per server, regardless of how many buffers attach to it.
- Idle servers (no events for `idle_shutdown_secs`, default 300 s) are killed by `lb-app`. We expose `last_activity()` so `lb-app` can decide.

## Files owned

```
crates/lb-lsp/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── client.rs       # LspClient, request/notification dispatch
│   ├── transport.rs    # framed JSON-RPC over AsyncRead/Write
│   ├── capabilities.rs # negotiation, feature gating
│   ├── encoding.rs     # UTF-8 ↔ UTF-16 ↔ buffer offsets
│   └── diagnostics.rs  # storage + delta updates
└── tests/
    └── mock_server.rs  # in-process mock for protocol tests
```

## Coordination notes

- Position encoding bugs are the #1 source of LSP issues. Any change to `encoding.rs` requires a property-test pass (use `proptest`).
- `lb-app` owns the **routing** of LSP responses (which buffer/cursor are they for?). This crate only correlates request IDs to futures.
- When `lb-lspmgr` resolves a server, it passes the spawned `Child` here — this crate does not know about PATH lookup, downloads, or the registry.
