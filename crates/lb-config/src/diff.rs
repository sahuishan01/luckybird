use crate::schema::{Config, EditorCfg, KeyMap, LspCfg, PluginCfg, ThemeCfg};

/// Section-granularity diff between two `Config` values.
///
/// `None` in a field means that section didn't change; `Some(new)` means it did.
/// Used by `:ConfigReload` to notify subsystems of what actually changed.
#[derive(Debug, Clone, Default)]
pub struct ConfigDelta {
    pub editor: Option<EditorCfg>,
    pub lsp: Option<LspCfg>,
    pub keys: Option<KeyMap>,
    pub plugins: Option<Vec<PluginCfg>>,
    pub theme: Option<ThemeCfg>,
}

impl ConfigDelta {
    pub fn is_empty(&self) -> bool {
        self.editor.is_none()
            && self.lsp.is_none()
            && self.keys.is_none()
            && self.plugins.is_none()
            && self.theme.is_none()
    }
}

impl Config {
    /// Compute what changed between `self` (old) and `new`.
    pub fn diff(&self, new: &Config) -> ConfigDelta {
        ConfigDelta {
            editor:  (self.editor  != new.editor).then(|| new.editor.clone()),
            lsp:     (self.lsp     != new.lsp).then(|| new.lsp.clone()),
            keys:    (self.keys    != new.keys).then(|| new.keys.clone()),
            plugins: (self.plugins != new.plugins).then(|| new.plugins.clone()),
            theme:   (self.theme   != new.theme).then(|| new.theme.clone()),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_configs_produce_empty_delta() {
        let a = Config::default();
        let b = Config::default();
        assert!(a.diff(&b).is_empty());
    }

    #[test]
    fn changed_tab_size_shows_in_editor_delta() {
        let a = Config::default();
        let mut b = a.clone();
        b.editor.tab_size = 2;
        let delta = a.diff(&b);
        assert!(delta.editor.is_some());
        assert!(delta.lsp.is_none());
        assert_eq!(delta.editor.unwrap().tab_size, 2);
    }

    #[test]
    fn changed_lsp_shows_only_in_lsp_delta() {
        let a = Config::default();
        let mut b = a.clone();
        b.lsp.idle_shutdown_secs = 60;
        let delta = a.diff(&b);
        assert!(delta.editor.is_none());
        assert!(delta.lsp.is_some());
    }
}
