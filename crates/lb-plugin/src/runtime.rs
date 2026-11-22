use std::path::Path;
use std::sync::OnceLock;

use mlua::Lua;
use tracing::debug;

use crate::api::LbApi;
use crate::sandbox;

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum PluginError {
    Lua(mlua::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lua(e) => write!(f, "Lua error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}
impl std::error::Error for PluginError {}

impl From<mlua::Error> for PluginError {
    fn from(e: mlua::Error) -> Self { Self::Lua(e) }
}
impl From<std::io::Error> for PluginError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

// ─── PluginRuntime ───────────────────────────────────────────────────────────

/// Lazily-initialised Lua 5.4 runtime for editor plugins.
///
/// The runtime is only created when `init_if_needed` is first called with
/// at least one plugin path, keeping the startup budget intact.
pub struct PluginRuntime {
    /// `None` until the first plugin is loaded.
    lua: OnceLock<Lua>,
    api: LbApi,
}

impl PluginRuntime {
    pub fn new() -> Self {
        Self {
            lua: OnceLock::new(),
            api: LbApi::new(),
        }
    }

    /// Initialise the Lua state (if not already done) and load all `paths`.
    ///
    /// Safe to call multiple times; new paths are loaded each call.
    pub fn init_if_needed(&self, paths: &[&Path]) -> Result<(), PluginError> {
        if paths.is_empty() {
            return Ok(());
        }

        let lua = self.lua.get_or_init(|| {
            debug!("initialising Lua 5.4 runtime");
            Lua::new()
        });

        // Apply sandbox on first init.
        // We track this by checking if `lb` global already exists.
        let already_inited: bool = {
            let v: mlua::Result<mlua::Value> = lua.globals().get("lb");
            v.map(|v| !matches!(v, mlua::Value::Nil)).unwrap_or(false)
        };

        if !already_inited {
            sandbox::apply(lua)?;
            self.api.install(lua)?;
        }

        for path in paths {
            self.load_file(lua, path)?;
        }

        Ok(())
    }

    /// Load and execute a single plugin file.
    fn load_file(&self, lua: &Lua, path: &Path) -> Result<(), PluginError> {
        debug!(path = %path.display(), "loading plugin");
        let src = std::fs::read_to_string(path)?;
        lua.load(&src)
            .set_name(path.to_string_lossy().as_ref())
            .exec()
            .map_err(PluginError::Lua)?;
        Ok(())
    }

    /// Fire a named event in all loaded plugins.
    ///
    /// Does nothing if the runtime has not been initialised.
    pub fn fire_event(&self, event: &str) -> Result<(), PluginError> {
        let lua = match self.lua.get() {
            Some(l) => l,
            None => return Ok(()),
        };
        self.api.fire_event(lua, event, &[]).map_err(PluginError::Lua)?;
        Ok(())
    }

    /// Return `true` if the Lua runtime has been started.
    pub fn is_active(&self) -> bool {
        self.lua.get().is_some()
    }

    /// Access the plugin API (for reading keymap overrides, etc.).
    pub fn api(&self) -> &LbApi {
        &self.api
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_not_active_before_init() {
        let rt = PluginRuntime::new();
        assert!(!rt.is_active());
    }

    #[test]
    fn fire_event_when_inactive_is_noop() {
        let rt = PluginRuntime::new();
        rt.fire_event("on_load").unwrap();
    }

    #[test]
    fn init_with_empty_paths_stays_inactive() {
        let rt = PluginRuntime::new();
        rt.init_if_needed(&[]).unwrap();
        assert!(!rt.is_active());
    }
}
