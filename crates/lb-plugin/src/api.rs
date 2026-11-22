use mlua::{Function, Lua, Result as LuaResult};
use std::sync::{Arc, Mutex};

// ─── Plugin event registry ───────────────────────────────────────────────────

/// Stores Lua callbacks registered with `lb.event.on`.
#[derive(Default, Clone)]
pub struct EventHandlers {
    /// Event name → list of Lua function refs.
    handlers: Arc<Mutex<std::collections::HashMap<String, Vec<mlua::RegistryKey>>>>,
}

impl EventHandlers {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Command registry: name → Lua function.
#[derive(Default, Clone)]
pub struct CommandRegistry {
    commands: Arc<Mutex<std::collections::HashMap<String, mlua::RegistryKey>>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Keymap overrides set by plugins: chord string → command name.
#[derive(Default, Clone)]
pub struct PluginKeymap {
    map: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl PluginKeymap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_all(&self) -> std::collections::HashMap<String, String> {
        self.map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

// ─── Build the `lb` global table ─────────────────────────────────────────────

pub struct LbApi {
    pub events: EventHandlers,
    pub commands: CommandRegistry,
    pub keymap: PluginKeymap,
}

impl LbApi {
    pub fn new() -> Self {
        Self {
            events: EventHandlers::new(),
            commands: CommandRegistry::new(),
            keymap: PluginKeymap::new(),
        }
    }

    /// Install the `lb` global table into `lua`.
    pub fn install(&self, lua: &Lua) -> LuaResult<()> {
        let lb = lua.create_table()?;

        // ── lb.notify(msg) ────────────────────────────────────────────────
        let notify_fn = lua.create_function(|_, msg: String| {
            tracing::info!("[plugin] {msg}");
            Ok(())
        })?;
        lb.set("notify", notify_fn)?;

        // ── lb.event.on(name, fn) ─────────────────────────────────────────
        let events = self.events.clone();
        let event_tbl = lua.create_table()?;
        let on_fn = lua.create_function(move |lua_ctx, (name, func): (String, Function)| {
            let key = lua_ctx.create_registry_value(func)?;
            let mut map = events.handlers.lock().unwrap_or_else(|e| e.into_inner());
            map.entry(name).or_default().push(key);
            Ok(())
        })?;
        event_tbl.set("on", on_fn)?;
        lb.set("event", event_tbl)?;

        // ── lb.cmd.register(name, fn) ─────────────────────────────────────
        let cmds = self.commands.clone();
        let cmd_tbl = lua.create_table()?;
        let reg_fn = lua.create_function(move |lua_ctx, (name, func): (String, Function)| {
            let key = lua_ctx.create_registry_value(func)?;
            let mut map = cmds.commands.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(name, key);
            Ok(())
        })?;
        cmd_tbl.set("register", reg_fn)?;
        lb.set("cmd", cmd_tbl)?;

        // ── lb.keymap.set(chord, command_name) ────────────────────────────
        let km = self.keymap.clone();
        let km_tbl = lua.create_table()?;
        let set_fn = lua.create_function(move |_, (chord, cmd): (String, String)| {
            let mut map = km.map.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(chord, cmd);
            Ok(())
        })?;
        km_tbl.set("set", set_fn)?;
        lb.set("keymap", km_tbl)?;

        // ── lb.buffer (placeholder, extended by lb-app) ───────────────────
        let buf_tbl = lua.create_table()?;
        lb.set("buffer", buf_tbl)?;

        lua.globals().set("lb", lb)?;
        Ok(())
    }

    /// Fire a named event, calling all registered Lua handlers.
    ///
    /// Errors from individual handlers are logged but do not abort the loop.
    pub fn fire_event(&self, lua: &Lua, event: &str, _args: &[mlua::Value]) -> LuaResult<()> {
        let map = self.events.handlers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ks) = map.get(event) {
            for key in ks {
                let func: Function = match lua.registry_value(key) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!("plugin event handler error: {e}");
                        continue;
                    }
                };
                if let Err(e) = func.call::<_, ()>(()) {
                    tracing::warn!("plugin event '{event}' handler error: {e}");
                }
            }
        }
        Ok(())
    }
}

impl Default for LbApi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_installs_lb_global() {
        let lua = Lua::new();
        let api = LbApi::new();
        api.install(&lua).unwrap();
        let lb: mlua::Table = lua.globals().get("lb").unwrap();
        assert!(lb.get::<_, mlua::Value>("notify").is_ok());
    }

    #[test]
    fn notify_does_not_panic() {
        let lua = Lua::new();
        let api = LbApi::new();
        api.install(&lua).unwrap();
        lua.load(r#"lb.notify("hello from test")"#).exec().unwrap();
    }

    #[test]
    fn keymap_set_stores_entry() {
        let lua = Lua::new();
        let api = LbApi::new();
        api.install(&lua).unwrap();
        lua.load(r#"lb.keymap.set("ctrl-k", "my_command")"#).exec().unwrap();
        let km = api.keymap.get_all();
        assert_eq!(km.get("ctrl-k").map(String::as_str), Some("my_command"));
    }
}
