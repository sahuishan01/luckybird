/// Information rendered in the single-row status line at the bottom.
#[derive(Debug, Clone, Default)]
pub struct StatusLine {
    /// Display name of the current file/buffer.
    pub filename: Option<String>,
    /// Buffer has unsaved changes.
    pub dirty: bool,
    /// 1-based line number of the cursor.
    pub line: usize,
    /// 1-based column number of the cursor.
    pub col: usize,
    /// File encoding label (e.g. `"UTF-8"`).
    pub encoding: &'static str,
    /// When set, overrides the normal status with a prompt (`:` cmd or `/` search).
    pub prompt: Option<String>,
    /// Editor mode label, e.g. `"NORMAL"` or `"INSERT"`. Empty = no mode shown.
    pub mode: &'static str,
}

impl StatusLine {
    pub fn new() -> Self {
        Self { encoding: "UTF-8", ..Default::default() }
    }

    /// One-line text for the left portion: `[MODE] filename [+]  Ln N, Col N`.
    pub fn left_text(&self) -> String {
        let dirty = if self.dirty { " [+]" } else { "" };
        let name = self.filename.as_deref().unwrap_or("[No Name]");
        let mode = if self.mode.is_empty() {
            String::new()
        } else {
            format!(" [{}] ", self.mode)
        };
        format!("{mode} {name}{dirty}  Ln {}, Col {}", self.line, self.col)
    }

    /// Shortcut cheat-sheet for the bottom row, depending on the mode.
    pub fn hint_text(&self) -> &'static str {
        match self.mode {
            "INSERT" => " Esc normal mode  |  ^S save  F5 reload  ^Z undo  |  Ctrl-Q quit  F1 menu",
            _ => " i/a/o insert  hjkl move  w/b word  0/$ line  x del  u undo  ^R redo  |  ^S save  F5 reload  q quit  Ctrl-Space/F1 menu",
        }
    }

    /// One-line text for the right portion: encoding.
    pub fn right_text(&self) -> String {
        format!("{}  ", self.encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_marker() {
        let mut s = StatusLine::new();
        s.filename = Some("main.rs".into());
        s.dirty = true;
        s.line = 10;
        s.col = 5;
        assert!(s.left_text().contains("[+]"));
        assert!(s.left_text().contains("main.rs"));
        assert!(s.left_text().contains("Ln 10"));
    }

    #[test]
    fn clean_no_marker() {
        let mut s = StatusLine::new();
        s.filename = Some("main.rs".into());
        assert!(!s.left_text().contains("[+]"));
    }

    #[test]
    fn hints_follow_mode() {
        let mut s = StatusLine::new();
        s.mode = "INSERT";
        assert!(s.hint_text().contains("Esc"));
        s.mode = "NORMAL";
        assert!(s.hint_text().contains("^S save"));
    }

    #[test]
    fn no_name_fallback() {
        let s = StatusLine::new();
        assert!(s.left_text().contains("[No Name]"));
    }
}
