# lb-plugin agent

## Mission
Compile-time Rust plugins. A plugin implements `Plugin` and registers key bindings and event handlers through `PluginApi`. Plugins are linked into the binary (gate them with Cargo features in `lb-app`), so they cost nothing at startup beyond their own code.

## Public API
```rust
pub trait Plugin { fn name(&self) -> &'static str; fn register(&self, api: &mut PluginApi); }
impl PluginApi { fn bind_key(&mut self, chord, command); fn on_event(&mut self, event, handler); }
impl PluginRuntime { fn new() -> Self; fn register(&mut self, &dyn Plugin);
                     fn fire_event(&self, event: &str); fn command_for(&self, chord: &str) -> Option<&str>; }
```

## Rules
- No script runtime (Lua/WASM) and no runtime-loaded plugin code. Adding either needs a sandbox design and explicit user approval.
- Depends on `tracing` only. Do not add `lb-core` or `lb-config` unless a real API needs them.
- `lb-app` calls `command_for` after built-in keymaps and fires events; the `[[plugin]]` config section is ignored (a warning is logged).
- Library-crate lint rules from CORE.md apply (no `unwrap`/`expect`/`panic`/indexing).

## Not in scope
- Handlers with editor access (buffers, commands). Add when the first real plugin needs it, not before.
