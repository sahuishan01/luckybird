# Agent: lb-lspmgr

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Resolve a language ID to a runnable LSP server binary. Detect what's preinstalled on PATH; download missing servers into a managed cache; verify checksums; spawn processes; supervise lifetimes.

This is the Mason.nvim of luckybird.

## In scope

- **Registry** (`registry.toml`, embedded via `include_str!`): per-server metadata — language IDs, binary name, install method, GitHub repo + asset pattern, SHA-256 (per platform), version-check command.
- **Resolution order**: PATH (via `which`) → managed cache (`<data-dir>/lsp/<server>/`) → user prompt → install.
- **Downloader**: `ureq` + `rustls`, blocking on a worker thread, progress reported to `lb-app` via channel.
- **Extractor**: `.zip`, `.tar.gz`, `.gz`. No proprietary formats.
- **Checksum verification**: SHA-256 against the registry. Fail closed.
- **Spawner**: `tokio::process::Command`, stdio piped, returns a `Child` for `lb-lsp` to drive.
- **Idle GC**: track per-server activity, kill after `idle_shutdown_secs`.
- **Update check** (opt-in): `:LspUpdate` queries GitHub Releases API, compares versions.

## Out of scope

- **The LSP protocol itself** — `lb-lsp`.
- **Per-language server selection rules** that depend on user config (e.g. user prefers `pyright` over `pylsp`) — `lb-app` reads config and asks us for a specific server name.
- **Initial v1**: no `npm`/`pip`/`go install`-based installs. We only support direct binary downloads. NPM-distributed servers (typescript-language-server, pyright) are documented as "install via npm and we'll detect on PATH" until v2. This is a deliberate v1 simplification — see [LB-XXXX entry when this lands].

## Public API surface (target)

```rust
pub struct LspManager { /* resolved servers, running children */ }
pub struct ServerInfo { pub name: String, pub binary: PathBuf, pub version: Option<String> }

impl LspManager {
    pub fn new(data_dir: PathBuf) -> Self;
    pub fn resolve(&mut self, language: &str) -> Result<Resolution>;
    pub async fn spawn(&mut self, server: &ServerInfo, root: &Path) -> Result<Child>;
    pub async fn install(&mut self, name: &str, progress: Sender<Progress>) -> Result<ServerInfo>;
    pub fn list_running(&self) -> Vec<&RunningServer>;
    pub async fn shutdown_idle(&mut self, max_idle: Duration) -> Vec<String>;
}

pub enum Resolution {
    Found(ServerInfo),
    NotInstalled { suggestions: Vec<&'static str> },
}
```

## Dependencies

- `which` (PATH lookup)
- `ureq` with `rustls-tls` (HTTP)
- `flate2`, `tar`, `zip` (extraction)
- `sha2` (checksum)
- `tokio` (process spawning + supervision)
- `lb-config` (read user prefs: which server per language, auto-install policy)

**Banned**: `reqwest`, `native-tls`, `openssl-sys`. (CORE.md § 2.5)

## Performance constraints

- `resolve()` for a server already on PATH: **≤ 5 ms**, no allocations beyond the returned `ServerInfo`.
- `resolve()` for a server in cache: **≤ 10 ms** (one stat + one version-check exec).
- Downloads: bandwidth-bound; show progress every 50 ms or every 1 % of bytes (whichever is sparser).
- The downloader runs on a `tokio::task::spawn_blocking` worker — never on the editor thread.

## Files owned

```
crates/lb-lspmgr/
├── Cargo.toml
├── registry.toml       # the server registry — most-edited file in this crate
├── src/
│   ├── lib.rs
│   ├── registry.rs     # parse + query registry.toml
│   ├── resolve.rs      # PATH / cache lookup
│   ├── download.rs     # ureq + checksum + extract
│   ├── spawn.rs        # tokio::process glue
│   └── supervise.rs    # idle GC, restart on crash (with backoff)
└── tests/
    ├── registry.rs
    └── download_offline.rs   # uses a local HTTP fixture server
```

## Coordination notes

- `registry.toml` is high-churn (every new language adds an entry). Two agents adding entries at once will conflict trivially in git — use sorted alphabetical order to minimize this.
- New entries in `registry.toml` require: language IDs, binary name, install URL pattern, SHA-256 for at least Linux x86_64 and macOS arm64, and version-check args. No checksum = no merge.
- Coordinate with `lb-syntax` on shared HTTP/extraction code — see CORE.md § 5 (don't pre-abstract).
