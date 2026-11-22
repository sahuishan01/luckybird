# Agent: lb-plugin

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Run user-authored Lua plugins inside a sandboxed `mlua` runtime. Expose a stable `lb` API table. Defer initialization until at least one plugin is configured **or** a runtime feature triggers it.

## In scope

- **Lua runtime**: `mlua` with Lua 5.4, vendored. `OnceCell<Lua>` — only created on first need.
- **Sandbox**: replace `os`, `io`, `package`, `require` with restricted versions. No file access outside `<data-dir>/plugins/<plugin-name>/`. No process exec.
- **`lb` API**: expose tables `lb.cmd`, `lb.keymap`, `lb.buffer`, `lb.lsp`, `lb.event`. Stable, documented, versioned (`lb.api_version = 1`).
- **Plugin lifecycle**: load file → run top-level → `lb.event` handlers fire on editor events.
- **Error containment**: a Lua error in plugin code logs and disables that plugin; never crashes the editor.
- **Async-from-Lua**: not in v1. Lua callbacks are synchronous. If a plugin needs async, it queues an `lb.cmd` for `lb-app` to drive.

## Out of scope

- **A package manager / plugin marketplace.** v1: users put `.lua` files in their plugin dir manually.
- **WASM plugins.** Not in v1.
- **JavaScript / Python plugins.** Never.
- **Hot reload of plugin code** — manual `:PluginReload <name>`. Don't watch files.

## Sandbox rules (these are load-bearing — see CORE.md § 6)

| Lua global | v1 policy |
|---|---|
| `os.execute`, `os.exit`, `os.remove`, `os.rename` | **removed** |
| `os.getenv` | **removed** (let plugins ask `lb.env(name)` for an allowlisted set) |
| `os.time`, `os.date`, `os.clock` | allowed |
| `io.open` | wrapped — only paths under the plugin's data dir |
| `io.popen`, `io.tmpfile` | **removed** |
| `package`, `require` | restricted to `lb.*` and the plugin's own files |
| `dofile`, `loadfile`, `load`, `loadstring` | **removed** |
| `debug.*` | **removed** |
| `arg` | **removed** |

## Public API surface (target — Rust side)

```rust
pub struct PluginRuntime { /* OnceCell<Lua> + plugin registry */ }

impl PluginRuntime {
    pub fn new() -> Self;                                  // does NOT init Lua
    pub fn ensure_started(&self) -> Result<()>;            // first call inits Lua + sandbox
    pub fn load(&self, name: &str, path: &Path) -> Result<()>;
    pub fn fire(&self, event: &Event) -> Result<()>;       // dispatch to handlers
    pub fn pending_commands(&self) -> Vec<Command>;        // queued by plugins for lb-app
}
```

## Public API surface (target — Lua side, `lb` global)

```lua
lb.api_version              -- integer; bump on breaking changes

lb.cmd.register(name, fn)
lb.cmd.run(name, ...)

lb.keymap.set(chord, command_name)

lb.buffer.current()         -- handle
lb.buffer.lines(buf, start, end)
lb.buffer.replace(buf, range, text)

lb.lsp.on_attach(fn)        -- fn(client, buf)
lb.lsp.request(client, method, params, callback)

lb.event.on(name, fn)       -- name: "buf_open", "buf_save", "config_reload", ...

lb.notify(level, msg)       -- shows in status line
```

API additions are cheap; renames or removals bump `api_version`.

## Dependencies

- `mlua` with `lua54`, `vendored` features
- `lb-core` (buffer access via Rust→Lua handles)
- `lb-config` (read `[[plugin]]` entries)

## Performance constraints

- `mlua::Lua::new()` cost: ~1 ms; that's why we defer it. Once started, idle Lua RSS overhead: ~1 MB.
- An event with no Lua handlers: **0 work** (don't even check Lua state).
- A keymap callback dispatch: **≤ 50 µs** when there is a handler.

## Files owned

```
crates/lb-plugin/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── runtime.rs      # OnceCell, ensure_started, fire
│   ├── sandbox.rs      # builds the restricted environment
│   ├── api.rs          # all lb.* table bindings
│   └── handles.rs      # buffer/client handles passed to Lua
└── tests/
    ├── sandbox.rs      # asserts os.execute is gone, etc.
    └── api.rs
```

## Coordination notes

- Every change to `api.rs` that removes or renames an `lb.*` symbol bumps `api_version` and gets a `CHANGES.md` entry tagged `lb-plugin` with a clear migration note.
- The `Command` queue interface to `lb-app` mirrors the one used by keymaps and LSP code-actions — coordinate with the agent owning `lb-app::commands`.
- Don't add network or filesystem capabilities to the sandbox without escalating. CORE.md § 6 is explicit.
