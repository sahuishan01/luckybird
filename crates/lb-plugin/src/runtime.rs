use std::collections::HashMap;

use tracing::debug;

type Handler = Box<dyn Fn() + Send + Sync>;

/// An editor plugin. Implementors are linked into the binary at compile time.
pub trait Plugin {
    fn name(&self) -> &'static str;

    /// Called once when the plugin is registered.
    fn register(&self, api: &mut PluginApi);
}

/// Registration surface handed to [`Plugin::register`].
#[derive(Default)]
pub struct PluginApi {
    keymap: HashMap<String, String>,
    events: HashMap<String, Vec<Handler>>,
}

impl PluginApi {
    /// Bind `chord` to the command called `command`.
    pub fn bind_key(&mut self, chord: impl Into<String>, command: impl Into<String>) {
        self.keymap.insert(chord.into(), command.into());
    }

    /// Run `handler` whenever `event` is fired.
    pub fn on_event(&mut self, event: impl Into<String>, handler: impl Fn() + Send + Sync + 'static) {
        self.events.entry(event.into()).or_default().push(Box::new(handler));
    }
}

/// Holds registered plugins' keymaps and event handlers.
#[derive(Default)]
pub struct PluginRuntime {
    api: PluginApi,
    names: Vec<&'static str>,
}

impl PluginRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: &dyn Plugin) {
        debug!(plugin = plugin.name(), "registering plugin");
        plugin.register(&mut self.api);
        self.names.push(plugin.name());
    }

    /// Run all handlers registered for `event`.
    pub fn fire_event(&self, event: &str) {
        if let Some(handlers) = self.api.events.get(event) {
            for handler in handlers {
                handler();
            }
        }
    }

    /// Command name bound to `chord` by a plugin, if any.
    pub fn command_for(&self, chord: &str) -> Option<&str> {
        self.api.keymap.get(chord).map(String::as_str)
    }

    pub fn is_active(&self) -> bool {
        !self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Demo(Arc<AtomicUsize>);

    impl Plugin for Demo {
        fn name(&self) -> &'static str {
            "demo"
        }

        fn register(&self, api: &mut PluginApi) {
            api.bind_key("ctrl-k", "my_command");
            let hits = Arc::clone(&self.0);
            api.on_event("on_load", move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
    }

    #[test]
    fn inactive_until_a_plugin_is_registered() {
        let mut rt = PluginRuntime::new();
        assert!(!rt.is_active());
        rt.register(&Demo(Arc::default()));
        assert!(rt.is_active());
    }

    #[test]
    fn fire_event_with_no_handlers_is_noop() {
        PluginRuntime::new().fire_event("on_load");
    }

    #[test]
    fn plugin_keymap_is_stored() {
        let mut rt = PluginRuntime::new();
        rt.register(&Demo(Arc::default()));
        assert_eq!(rt.command_for("ctrl-k"), Some("my_command"));
        assert_eq!(rt.command_for("ctrl-j"), None);
    }

    #[test]
    fn event_handlers_run() {
        let hits = Arc::new(AtomicUsize::new(0));
        let mut rt = PluginRuntime::new();
        rt.register(&Demo(Arc::clone(&hits)));
        rt.fire_event("on_load");
        rt.fire_event("on_load");
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
