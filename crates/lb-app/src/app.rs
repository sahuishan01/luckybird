use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use lb_config::Config;
use lb_core::{
    move_char_left, move_char_right, move_doc_end, move_doc_start, move_line_down,
    move_line_end, move_line_start, move_line_up, move_word_backward, move_word_forward,
    Buffer, Edit, Selection, UndoTree,
};
use lb_lsp::ServerEvent;
use lb_lspmgr::LspManager;
use lb_plugin::PluginRuntime;
use lb_syntax::{HighlightSpan, SyntaxManager};
use lb_tui::{
    render::{DiagnosticSeverity as TuiSev, Highlight as TuiHighlight, View},
    EditorEvent, KeyCode, KeyEvent2, StatusLine, Tui,
};
use lsp_types::DiagnosticSeverity as LspSev;
use tracing::{debug, warn};

use crate::commands::{from_name, Command};

// ─── Open buffer ─────────────────────────────────────────────────────────────

pub struct OpenBuffer {
    pub buffer: Buffer,
    pub selection: Selection,
    pub undo: UndoTree,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub language: Option<String>,
    pub syntax_spans: Vec<HighlightSpan>,
    pub lsp_version: i32,
    pub scroll_offset: usize,
}

impl OpenBuffer {
    pub fn empty() -> Self {
        Self {
            buffer: Buffer::new(),
            selection: Selection::point(0),
            undo: UndoTree::new(),
            path: None,
            dirty: false,
            language: None,
            syntax_spans: vec![],
            lsp_version: 0,
            scroll_offset: 0,
        }
    }

    pub fn from_file(path: PathBuf, text: String, language: Option<String>) -> Self {
        Self {
            buffer: Buffer::from_str(&text),
            selection: Selection::point(0),
            undo: UndoTree::new(),
            path: Some(path),
            dirty: false,
            language,
            syntax_spans: vec![],
            lsp_version: 1,
            scroll_offset: 0,
        }
    }

    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "[unnamed]".to_string()),
            None => "[scratch]".to_string(),
        }
    }
}

// ─── Diagnostic cache ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DiagEntry {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub severity: TuiSev,
    pub message: String,
}

// ─── Editor mode ─────────────────────────────────────────────────────────────

/// Modal editing state. `Normal` for command keys, `Insert` for typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::Insert => "insert",
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub buffers: Vec<OpenBuffer>,
    pub active: usize,
    pub tui: Tui,
    pub syntax: SyntaxManager,
    pub lsp: LspManager,
    pub plugins: PluginRuntime,
    pub quit: bool,
    /// Transient status-bar message (overrides normal status line).
    pub status_msg: Option<String>,
    /// Diagnostics per buffer path (URI string).
    pub diagnostics: HashMap<String, Vec<DiagEntry>>,
    /// Set when the user pressed Quit on a dirty buffer; a second Quit
    /// in a row will force-quit.
    pub quit_armed: bool,
    /// Current modal editing state.
    pub mode: Mode,
}

impl App {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new(
        config: Config,
        tui: Tui,
        syntax: SyntaxManager,
        lsp: LspManager,
        plugins: PluginRuntime,
    ) -> Self {
        let buffers = vec![OpenBuffer::empty()];
        Self {
            config,
            buffers,
            active: 0,
            tui,
            syntax,
            lsp,
            plugins,
            quit: false,
            status_msg: None,
            diagnostics: HashMap::new(),
            quit_armed: false,
            mode: Mode::Normal,
        }
    }

    // ── File I/O ──────────────────────────────────────────────────────────

    pub fn open_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let (text, is_new) = match std::fs::read_to_string(&path) {
            Ok(text) => (text, false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (String::new(), true),
            Err(e) => {
                return Err(e).with_context(|| format!("reading {}", path.display()));
            }
        };
        if is_new {
            self.status_msg = Some(format!("New file {} — Ctrl-S to create it", path.display()));
        }
        let language = self.syntax.detect(
            path.to_str().unwrap_or(""),
            text.lines().next(),
        );
        debug!(path = %path.display(), ?language, "opened file");

        // Update syntax spans if grammar is available.
        let mut spans = Vec::new();
        if let Some(ref lang) = language {
            let layer = self.syntax.layer_for(lang);
            let mut layer = layer;
            spans = layer.highlight(&text);
        }

        // If the first buffer is the empty scratch buffer, replace it.
        if self.buffers.len() == 1
            && self.buffers[0].path.is_none()
            && self.buffers[0].buffer.len_bytes() == 0
        {
            let mut buf = OpenBuffer::from_file(path.clone(), text, language);
            buf.syntax_spans = spans;
            self.buffers[0] = buf;
        } else {
            let mut buf = OpenBuffer::from_file(path.clone(), text, language);
            buf.syntax_spans = spans;
            self.buffers.push(buf);
            self.active = self.buffers.len() - 1;
        }
        Ok(())
    }

    pub fn save_active(&mut self) -> anyhow::Result<()> {
        let buf = &mut self.buffers[self.active];
        let path = match &buf.path {
            Some(p) => p.clone(),
            None => {
                self.status_msg = Some("No file name — restart with: luckybird <file>".to_string());
                return Ok(());
            }
        };
        let text = buf.buffer.to_string();
        std::fs::write(&path, text.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;
        buf.dirty = false;
        self.status_msg = Some(format!("Saved {}", path.display()));
        Ok(())
    }

    // ── Editing ───────────────────────────────────────────────────────────

    fn active_buf(&mut self) -> &mut OpenBuffer {
        // Safety: active is always in bounds (maintained by all mutating ops).
        let idx = self.active;
        &mut self.buffers[idx]
    }

    fn insert_text(&mut self, text: &str) -> anyhow::Result<()> {
        let buf = self.active_buf();
        let pos = buf.selection.cursor();
        let edit = Edit::insert(pos, text.to_string());
        let change = buf.buffer.apply(&edit)
            .map_err(|e| anyhow::anyhow!("edit error: {e:?}"))?;
        buf.undo.record(change);
        let new_pos = pos + text.len();
        buf.selection = Selection::point(new_pos);
        buf.dirty = true;
        buf.lsp_version += 1;
        Ok(())
    }

    fn delete_backward(&mut self) -> anyhow::Result<()> {
        let buf = self.active_buf();
        let cursor = buf.selection.cursor();
        if cursor == 0 {
            return Ok(());
        }
        // Find previous char boundary.
        let text = buf.buffer.to_string();
        let prefix = text.get(..cursor).unwrap_or("");
        let prev_boundary = prefix
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        let edit = Edit::delete(prev_boundary, cursor);
        let change = buf.buffer.apply(&edit)
            .map_err(|e| anyhow::anyhow!("delete error: {e:?}"))?;
        buf.undo.record(change);
        buf.selection = Selection::point(prev_boundary);
        buf.dirty = true;
        buf.lsp_version += 1;
        Ok(())
    }

    fn delete_forward(&mut self) -> anyhow::Result<()> {
        let buf = self.active_buf();
        let cursor = buf.selection.cursor();
        if cursor >= buf.buffer.len_bytes() {
            return Ok(());
        }
        let text = buf.buffer.to_string();
        let rest = text.get(cursor..).unwrap_or("");
        let next_boundary = rest
            .char_indices()
            .nth(1)
            .map(|(i, _)| cursor + i)
            .unwrap_or(buf.buffer.len_bytes());
        let edit = Edit::delete(cursor, next_boundary);
        let change = buf.buffer.apply(&edit)
            .map_err(|e| anyhow::anyhow!("delete error: {e:?}"))?;
        buf.undo.record(change);
        buf.selection = Selection::point(cursor);
        buf.dirty = true;
        buf.lsp_version += 1;
        Ok(())
    }

    fn undo(&mut self) -> anyhow::Result<()> {
        let buf = self.active_buf();
        if let Some(change) = buf.undo.undo() {
            let _ = buf.buffer.revert(change);
            buf.selection = Selection::point(change.start);
            buf.dirty = true;
        }
        Ok(())
    }

    fn redo(&mut self) -> anyhow::Result<()> {
        let buf = self.active_buf();
        if let Some(change) = buf.undo.redo() {
            let edit = Edit::insert(change.start, change.new_text.clone());
            let _ = buf.buffer.apply(&edit);
            buf.selection = Selection::point(change.start + change.new_text.len());
            buf.dirty = true;
        }
        Ok(())
    }

    // ── Movement helpers ──────────────────────────────────────────────────

    fn apply_movement<F>(&mut self, f: F)
    where
        F: FnOnce(&Selection, &Buffer) -> Selection,
    {
        let idx = self.active;
        let new_sel = f(&self.buffers[idx].selection, &self.buffers[idx].buffer);
        self.buffers[idx].selection = new_sel;
        self.ensure_cursor_visible();
    }

    fn ensure_cursor_visible(&mut self) {
        let (_, height) = self.tui.size().unwrap_or((80, 24));
        let content_height = (height.saturating_sub(2)) as usize; // minus status and hint rows
        let buf = &mut self.buffers[self.active];
        let cursor_line = buf
            .buffer
            .byte_to_line(buf.selection.cursor())
            .unwrap_or(0);
        let scroll_off = self.config.editor.scroll_off as usize;

        // Scroll up if cursor is above viewport.
        if cursor_line < buf.scroll_offset + scroll_off {
            buf.scroll_offset = cursor_line.saturating_sub(scroll_off);
        }
        // Scroll down if cursor is below viewport.
        let bottom_line = buf.scroll_offset + content_height;
        if cursor_line + scroll_off >= bottom_line {
            buf.scroll_offset = (cursor_line + scroll_off + 1).saturating_sub(content_height);
        }
    }

    // ── Command execution ─────────────────────────────────────────────────

    /// Handle one event.  Returns `true` if the app should quit.
    pub fn handle_event(&mut self, ev: EditorEvent) -> anyhow::Result<bool> {
        match ev {
            EditorEvent::Key(key) => self.handle_key(key),
            EditorEvent::Resize(_, _) => {
                self.ensure_cursor_visible();
                Ok(false)
            }
            EditorEvent::Paste(text) => {
                self.insert_text(&text)?;
                Ok(false)
            }
            EditorEvent::Mouse(_) | EditorEvent::FocusGained | EditorEvent::FocusLost => {
                Ok(false)
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent2) -> anyhow::Result<bool> {
        let chord = key_to_chord_str(&key);
        let mode_chord = format!("{}:{}", self.mode.prefix(), chord);
        tracing::info!(?key, mode = ?self.mode, chord = %chord, "key event");

        // 1. Mode-prefixed binding wins (e.g. `normal:h`, `insert:escape`).
        if !chord.is_empty() {
            if let Some(cmd_name) = self.config.keys.0.get(&mode_chord).cloned() {
                tracing::info!(chord = %mode_chord, cmd = %cmd_name, "mode chord matched");
                if let Some(cmd) = from_name(&cmd_name) {
                    return self.execute(cmd);
                } else {
                    self.status_msg = Some(format!(
                        "Bound to '{cmd_name}' but that command is not implemented"
                    ));
                    return Ok(false);
                }
            }

            // 2. Global binding.
            if let Some(cmd_name) = self.config.keys.0.get(&chord).cloned() {
                tracing::info!(chord = %chord, cmd = %cmd_name, "global chord matched");
                if let Some(cmd) = from_name(&cmd_name) {
                    return self.execute(cmd);
                } else {
                    self.status_msg = Some(format!(
                        "Bound to '{cmd_name}' but that command is not implemented"
                    ));
                    return Ok(false);
                }
            }
        }

        // 3. Plugin-registered keymaps (always global for now).
        let plugin_cmd = self.plugins.command_for(&chord).and_then(from_name);
        if let Some(cmd) = plugin_cmd {
            return self.execute(cmd);
        }

        // 4. Default handling. In Normal mode unbound printable keys do
        //    nothing; in Insert mode they insert.
        if !key.modifiers.ctrl && !key.modifiers.alt {
            match key.code {
                KeyCode::Char(ch) if matches!(self.mode, Mode::Insert) => {
                    return self.execute(Command::InsertChar(ch));
                }
                KeyCode::Enter if matches!(self.mode, Mode::Insert) => {
                    return self.execute(Command::InsertNewline);
                }
                KeyCode::Tab if matches!(self.mode, Mode::Insert) => {
                    return self.execute(Command::InsertTab);
                }
                KeyCode::Backspace => return self.execute(Command::DeleteBackward),
                KeyCode::Delete => return self.execute(Command::DeleteForward),
                KeyCode::Up => return self.execute(Command::MoveUp),
                KeyCode::Down => return self.execute(Command::MoveDown),
                KeyCode::Left => return self.execute(Command::MoveLeft),
                KeyCode::Right => return self.execute(Command::MoveRight),
                KeyCode::Home => return self.execute(Command::MoveLineStart),
                KeyCode::End => return self.execute(Command::MoveLineEnd),
                KeyCode::PageUp => return self.execute(Command::PageUp),
                KeyCode::PageDown => return self.execute(Command::PageDown),
                KeyCode::Escape => {
                    self.status_msg = None;
                    return Ok(false);
                }
                _ => {}
            }
        }

        Ok(false)
    }

    pub fn execute(&mut self, cmd: Command) -> anyhow::Result<bool> {
        // Any command that isn't Quit/ForceQuit clears the "press again to
        // force-quit" arming state.
        if !matches!(cmd, Command::Quit | Command::ForceQuit) {
            self.quit_armed = false;
        }
        match cmd {
            Command::MoveLeft => {
                self.apply_movement(move_char_left);
            }
            Command::MoveRight => {
                self.apply_movement(move_char_right);
            }
            Command::MoveUp => {
                self.apply_movement(move_line_up);
            }
            Command::MoveDown => {
                self.apply_movement(move_line_down);
            }
            Command::MoveWordForward => {
                self.apply_movement(move_word_forward);
            }
            Command::MoveWordBackward => {
                self.apply_movement(move_word_backward);
            }
            Command::MoveLineStart => {
                self.apply_movement(move_line_start);
            }
            Command::MoveLineEnd => {
                self.apply_movement(move_line_end);
            }
            Command::MoveDocStart => {
                self.apply_movement(move_doc_start);
            }
            Command::MoveDocEnd => {
                self.apply_movement(move_doc_end);
            }
            Command::PageUp => {
                let (_, h) = self.tui.size().unwrap_or((80, 24));
                let page = (h.saturating_sub(2)) as usize;
                let buf = &mut self.buffers[self.active];
                let cursor = buf.selection.cursor();
                let cur_line = buf.buffer.byte_to_line(cursor).unwrap_or(0);
                let new_line = cur_line.saturating_sub(page);
                let new_pos = buf.buffer.line_to_byte(new_line).unwrap_or(0);
                buf.selection = Selection::point(new_pos);
                buf.scroll_offset = buf.scroll_offset.saturating_sub(page);
            }
            Command::PageDown => {
                let (_, h) = self.tui.size().unwrap_or((80, 24));
                let page = (h.saturating_sub(2)) as usize;
                let buf = &mut self.buffers[self.active];
                let cursor = buf.selection.cursor();
                let cur_line = buf.buffer.byte_to_line(cursor).unwrap_or(0);
                let new_line = (cur_line + page).min(buf.buffer.len_lines().saturating_sub(1));
                let new_pos = buf.buffer.line_to_byte(new_line).unwrap_or(0);
                buf.selection = Selection::point(new_pos);
                buf.scroll_offset = (buf.scroll_offset + page)
                    .min(buf.buffer.len_lines().saturating_sub(1));
            }
            Command::InsertChar(ch) => {
                let mut s = [0u8; 4];
                let enc = ch.encode_utf8(&mut s);
                self.insert_text(enc)?;
            }
            Command::InsertNewline => self.insert_text("\n")?,
            Command::InsertTab => {
                let spaces = self.config.editor.tab_size as usize;
                if self.config.editor.expand_tabs {
                    let indent = " ".repeat(spaces);
                    self.insert_text(&indent)?;
                } else {
                    self.insert_text("\t")?;
                }
            }
            Command::DeleteBackward => self.delete_backward()?,
            Command::DeleteForward => self.delete_forward()?,
            Command::Undo => self.undo()?,
            Command::Redo => self.redo()?,
            Command::Save => self.save_active()?,
            Command::Quit => {
                if self.has_dirty_buffers() && !self.quit_armed {
                    self.quit_armed = true;
                    self.status_msg = Some(
                        "Unsaved changes — press Ctrl-Q again to discard, or Ctrl-S to save"
                            .to_string(),
                    );
                } else {
                    return Ok(true);
                }
            }
            Command::ForceQuit => return Ok(true),
            Command::EnterNormalMode => {
                self.mode = Mode::Normal;
            }
            Command::EnterInsertMode => {
                self.mode = Mode::Insert;
            }
            Command::EnterInsertAfter => {
                // Move one char right (if not at line end), then enter insert.
                self.apply_movement(move_char_right);
                self.mode = Mode::Insert;
            }
            Command::OpenLineBelow => {
                self.apply_movement(move_line_end);
                self.insert_text("\n")?;
                self.mode = Mode::Insert;
            }
            Command::OpenLineAbove => {
                self.apply_movement(move_line_start);
                self.insert_text("\n")?;
                // After inserting, cursor is on the new (empty) line above.
                self.apply_movement(move_line_up);
                self.mode = Mode::Insert;
            }
            Command::Hover => {
                // LSP hover: fire and forget (result handled via status line)
                let _ = self.trigger_hover();
            }
            other => {
                // Unimplemented commands surface a status message so the user
                // gets feedback instead of silence.
                debug!(cmd = ?other, "command not yet implemented");
                self.status_msg = Some(format!("{other:?}: not yet implemented"));
            }
        }
        Ok(false)
    }

    fn trigger_hover(&mut self) -> anyhow::Result<()> {
        // In a real implementation this would be async.
        // For now just note the intent.
        debug!("hover requested");
        Ok(())
    }

    pub fn has_dirty_buffers(&self) -> bool {
        self.buffers.iter().any(|b| b.dirty)
    }

    // ── LSP event polling ─────────────────────────────────────────────────

    /// Non-blocking drain of LSP event queues.
    pub fn poll_lsp_events(&mut self) {
        // Collect all language names that have active servers.
        let languages: Vec<String> = self
            .buffers
            .iter()
            .filter_map(|b| b.language.clone())
            .collect();

        for lang in &languages {
            if let Some(client) = self.lsp.get(lang) {
                loop {
                    match client.events.try_recv() {
                        Ok(ServerEvent::Diagnostics(params)) => {
                            let uri_str = params.uri.to_string();
                            let entries: Vec<DiagEntry> = params
                                .diagnostics
                                .iter()
                                .map(|d| DiagEntry {
                                    start_line: d.range.start.line,
                                    start_char: d.range.start.character,
                                    end_line: d.range.end.line,
                                    end_char: d.range.end.character,
                                    severity: match d.severity {
                                        Some(LspSev::ERROR) => TuiSev::Error,
                                        Some(LspSev::WARNING) => TuiSev::Warning,
                                        Some(LspSev::INFORMATION) => {
                                            TuiSev::Information
                                        }
                                        _ => TuiSev::Hint,
                                    },
                                    message: d.message.clone(),
                                })
                                .collect();
                            self.diagnostics.insert(uri_str, entries);
                        }
                        Ok(ServerEvent::Exited) => {
                            warn!(lang, "LSP server exited unexpectedly");
                            break;
                        }
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }
            }
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────

    pub fn render(&mut self) -> anyhow::Result<()> {
        let active = self.active;

        // Collect owned render data to avoid conflicting borrows.
        let (buffer, selection, syntax_spans, scroll_offset, display_name, dirty, cursor) = {
            let buf = &self.buffers[active];
            (
                buf.buffer.clone(),
                buf.selection.clone(),
                buf.syntax_spans.clone(),
                buf.scroll_offset,
                buf.display_name(),
                buf.dirty,
                buf.selection.cursor(),
            )
        };

        let cur_line = buffer.byte_to_line(cursor).unwrap_or(0);
        let line_start = buffer.line_to_byte(cur_line).unwrap_or(0);
        let col = cursor.saturating_sub(line_start);

        let status_msg = self.status_msg.clone();
        let status = StatusLine {
            filename: Some(display_name),
            dirty,
            line: cur_line + 1,
            col: col + 1,
            encoding: "UTF-8",
            prompt: status_msg,
            mode: self.mode.label(),
        };

        // Convert syntax spans to lb-tui Highlight type.
        let highlights: Vec<TuiHighlight> = syntax_spans
            .iter()
            .map(|s| TuiHighlight { start: s.start, end: s.end, capture: s.capture })
            .collect();

        let view = View {
            buffer: &buffer,
            selection: &selection,
            highlights: &highlights,
            diagnostics: &[],
            status,
            scroll_offset,
        };

        self.tui.render(&view)?;
        Ok(())
    }
}

// ─── Key→chord string helper ─────────────────────────────────────────────────

/// Convert a `KeyEvent2` into the chord string used as config keymap keys,
/// e.g. `ctrl-p`, `ctrl-shift-p`, `f5`.
pub fn key_to_chord_str(key: &KeyEvent2) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if key.modifiers.ctrl { parts.push("ctrl"); }
    if key.modifiers.alt { parts.push("alt"); }
    // For Char keys, the character itself encodes shift (case for letters,
    // distinct glyph for symbols like $ vs 4), so the `shift` modifier is
    // redundant. Without this rule, Windows' spurious shift on Ctrl+letter
    // would also produce wrong chords like `ctrl-shift-q`.
    let is_char = matches!(&key.code, KeyCode::Char(_));
    if key.modifiers.shift && !is_char { parts.push("shift"); }
    if key.modifiers.super_key { parts.push("super"); }

    let key_part;
    let key_str: &str = match &key.code {
        KeyCode::Char(c) => {
            // For Ctrl/Alt+letter, normalise to lowercase so `Ctrl-Q` matches
            // `ctrl-q` regardless of whether Windows tagged it as `Q`+shift.
            // For plain typing keep the char as-is so `O` ≠ `o`.
            if c.is_alphabetic() && (key.modifiers.ctrl || key.modifiers.alt) {
                key_part = c.to_lowercase().next().unwrap_or(*c).to_string();
            } else {
                key_part = c.to_string();
            }
            &key_part
        }
        KeyCode::Enter => "enter",
        KeyCode::Backspace => "backspace",
        KeyCode::Delete => "delete",
        KeyCode::Tab => "tab",
        KeyCode::BackTab => "backtab",
        KeyCode::Escape => "escape",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Home => "home",
        KeyCode::End => "end",
        KeyCode::PageUp => "pageup",
        KeyCode::PageDown => "pagedown",
        KeyCode::Insert => "insert",
        KeyCode::F(n) => {
            key_part = format!("f{n}");
            &key_part
        }
        KeyCode::Null => return String::new(),
    };

    parts.push(key_str);
    parts.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_tui::{KeyCode as KC, KeyEvent2, KeyModifiers};

    fn mods_none() -> KeyModifiers {
        KeyModifiers { ctrl: false, alt: false, shift: false, super_key: false }
    }

    fn mods_ctrl() -> KeyModifiers {
        KeyModifiers { ctrl: true, alt: false, shift: false, super_key: false }
    }

    fn mods_ctrl_shift() -> KeyModifiers {
        KeyModifiers { ctrl: true, alt: false, shift: true, super_key: false }
    }

    #[test]
    fn chord_ctrl_p() {
        let key = KeyEvent2 { code: KC::Char('p'), modifiers: mods_ctrl() };
        assert_eq!(key_to_chord_str(&key), "ctrl-p");
    }

    #[test]
    fn ctrl_shift_letter_drops_shift() {
        // Windows reports Ctrl+letter with shift=true; the chord should still
        // match `ctrl-<letter>`, never `ctrl-shift-<letter>`.
        let key = KeyEvent2 { code: KC::Char('Q'), modifiers: mods_ctrl_shift() };
        assert_eq!(key_to_chord_str(&key), "ctrl-q");
    }

    #[test]
    fn ctrl_shift_nonletter_keeps_shift() {
        // Tab is not alphabetic — shift should be preserved for Ctrl+Shift+Tab.
        let key = KeyEvent2 { code: KC::Tab, modifiers: mods_ctrl_shift() };
        assert_eq!(key_to_chord_str(&key), "ctrl-shift-tab");
    }

    #[test]
    fn chord_f5() {
        let key = KeyEvent2 { code: KC::F(5), modifiers: mods_none() };
        assert_eq!(key_to_chord_str(&key), "f5");
    }

    #[test]
    fn chord_plain_char() {
        let key = KeyEvent2 { code: KC::Char('a'), modifiers: mods_none() };
        assert_eq!(key_to_chord_str(&key), "a");
    }
}
