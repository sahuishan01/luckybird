use crossterm::event::{
    Event as CxEvent, KeyCode as CxKeyCode, KeyEvent, KeyEventKind as CxKeyKind,
    KeyModifiers as CxMods, MouseButton as CxBtn, MouseEvent as CxMouseEvent,
    MouseEventKind as CxMouseKind,
};

// ─── Public event types ──────────────────────────────────────────────────────
// lb-tui owns these types; lb-app converts them to lb-config::KeyChord for
// command dispatch. This keeps lb-tui free of a lb-config dependency.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl KeyModifiers {
    pub fn none() -> Self {
        Self { ctrl: false, alt: false, shift: false, super_key: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Moved,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent2 {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    Key(KeyEvent2),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
    FocusGained,
    FocusLost,
}

// ─── Public event reader ─────────────────────────────────────────────────────

/// Block until one terminal event is available, translate it, and return it.
///
/// Designed to be called from a **dedicated background OS thread** while the
/// main task owns the `Tui` for rendering.  Because `crossterm::event::read`
/// is a global singleton, multiple threads may not call this simultaneously.
pub fn read_event() -> std::io::Result<EditorEvent> {
    loop {
        let raw = crossterm::event::read()?;
        if let Some(ev) = translate_event(raw) {
            return Ok(ev);
        }
        // Unrecognised event variant — loop.
    }
}

// ─── Translation from crossterm ──────────────────────────────────────────────

pub(crate) fn translate_event(ev: CxEvent) -> Option<EditorEvent> {
    match ev {
        CxEvent::Key(k) => {
            // On Windows, crossterm reports Press AND Release for every key.
            // Drop everything except Press to avoid double-firing actions.
            if !matches!(k.kind, CxKeyKind::Press) {
                return None;
            }
            Some(EditorEvent::Key(translate_key(k)))
        }
        CxEvent::Mouse(m) => translate_mouse(m).map(EditorEvent::Mouse),
        CxEvent::Resize(w, h) => Some(EditorEvent::Resize(w, h)),
        CxEvent::Paste(s) => Some(EditorEvent::Paste(s)),
        CxEvent::FocusGained => Some(EditorEvent::FocusGained),
        CxEvent::FocusLost => Some(EditorEvent::FocusLost),
    }
}

fn translate_key(k: KeyEvent) -> KeyEvent2 {
    let mods = translate_mods(k.modifiers);
    let code = translate_keycode(k.code);
    KeyEvent2 { code, modifiers: mods }
}

fn translate_mods(m: CxMods) -> KeyModifiers {
    KeyModifiers {
        ctrl: m.contains(CxMods::CONTROL),
        alt: m.contains(CxMods::ALT),
        shift: m.contains(CxMods::SHIFT),
        super_key: m.contains(CxMods::SUPER),
    }
}

fn translate_keycode(code: CxKeyCode) -> KeyCode {
    match code {
        CxKeyCode::Char(c) => KeyCode::Char(c),
        CxKeyCode::Enter => KeyCode::Enter,
        CxKeyCode::Backspace => KeyCode::Backspace,
        CxKeyCode::Delete => KeyCode::Delete,
        CxKeyCode::Tab => KeyCode::Tab,
        CxKeyCode::BackTab => KeyCode::BackTab,
        CxKeyCode::Esc => KeyCode::Escape,
        CxKeyCode::Up => KeyCode::Up,
        CxKeyCode::Down => KeyCode::Down,
        CxKeyCode::Left => KeyCode::Left,
        CxKeyCode::Right => KeyCode::Right,
        CxKeyCode::Home => KeyCode::Home,
        CxKeyCode::End => KeyCode::End,
        CxKeyCode::PageUp => KeyCode::PageUp,
        CxKeyCode::PageDown => KeyCode::PageDown,
        CxKeyCode::Insert => KeyCode::Insert,
        CxKeyCode::F(n) => KeyCode::F(n),
        CxKeyCode::Null => KeyCode::Null,
        _ => KeyCode::Null,
    }
}

fn translate_mouse(m: CxMouseEvent) -> Option<MouseEvent> {
    let kind = match m.kind {
        CxMouseKind::Down(b) => MouseEventKind::Down(translate_button(b)?),
        CxMouseKind::Up(b) => MouseEventKind::Up(translate_button(b)?),
        CxMouseKind::Drag(b) => MouseEventKind::Drag(translate_button(b)?),
        CxMouseKind::Moved => MouseEventKind::Moved,
        CxMouseKind::ScrollDown => MouseEventKind::ScrollDown,
        CxMouseKind::ScrollUp => MouseEventKind::ScrollUp,
        CxMouseKind::ScrollLeft => MouseEventKind::ScrollLeft,
        CxMouseKind::ScrollRight => MouseEventKind::ScrollRight,
    };
    Some(MouseEvent {
        kind,
        column: m.column,
        row: m.row,
        modifiers: translate_mods(m.modifiers),
    })
}

fn translate_button(b: CxBtn) -> Option<MouseButton> {
    match b {
        CxBtn::Left => Some(MouseButton::Left),
        CxBtn::Right => Some(MouseButton::Right),
        CxBtn::Middle => Some(MouseButton::Middle),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_ctrl_p() {
        let cx_key = KeyEvent::new(CxKeyCode::Char('p'), CxMods::CONTROL);
        let ev = translate_key(cx_key);
        assert_eq!(ev.code, KeyCode::Char('p'));
        assert!(ev.modifiers.ctrl);
        assert!(!ev.modifiers.shift);
    }

    #[test]
    fn translate_shift_tab() {
        let cx_key = KeyEvent::new(CxKeyCode::BackTab, CxMods::SHIFT);
        let ev = translate_key(cx_key);
        assert_eq!(ev.code, KeyCode::BackTab);
        assert!(ev.modifiers.shift);
    }

    #[test]
    fn translate_f_keys() {
        let cx_key = KeyEvent::new(CxKeyCode::F(5), CxMods::empty());
        let ev = translate_key(cx_key);
        assert_eq!(ev.code, KeyCode::F(5));
    }

    #[test]
    fn translate_resize() {
        let ev = translate_event(CxEvent::Resize(120, 40));
        assert_eq!(ev, Some(EditorEvent::Resize(120, 40)));
    }
}
