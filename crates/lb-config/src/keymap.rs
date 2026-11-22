// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Tab,
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
    /// Function key 1-24.
    F(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub modifiers: Modifiers,
    pub key: Key,
}

// ─── Parse error ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyParseError {
    Empty,
    UnknownModifier(String),
    UnknownKey(String),
}

impl std::fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty key chord"),
            Self::UnknownModifier(m) => write!(f, "unknown modifier: {m}"),
            Self::UnknownKey(k) => write!(f, "unknown key: {k}"),
        }
    }
}

impl std::error::Error for KeyParseError {}

// ─── Parser ──────────────────────────────────────────────────────────────────

/// Parse a chord string like `"ctrl-shift-p"` or `"f1"` into a `KeyChord`.
///
/// Modifiers: `ctrl`/`alt`/`shift`/`super` (case-insensitive), separated by `-`.
/// The key is always the last `-`-separated token.
/// Named keys: `enter`, `backspace`/`bs`, `delete`/`del`, `tab`, `escape`/`esc`,
///   `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`,
///   `insert`/`ins`, `space`, `minus`, `f1`–`f24`.
/// Single characters are taken as-is (lowercase recommended; shift modifier
/// determines capitalisation at the UI layer).
pub fn parse_chord(s: &str) -> Result<KeyChord, KeyParseError> {
    if s.is_empty() {
        return Err(KeyParseError::Empty);
    }

    let parts: Vec<&str> = s.split('-').collect();

    let Some((key_part, mod_parts)) = parts.split_last() else {
        return Err(KeyParseError::Empty);
    };

    let mut modifiers = Modifiers::default();
    for m in mod_parts {
        match *m {
            "ctrl" | "Ctrl" | "control" => modifiers.ctrl = true,
            "alt" | "Alt" | "meta" | "opt" => modifiers.alt = true,
            "shift" | "Shift" => modifiers.shift = true,
            "super" | "Super" | "cmd" | "win" => modifiers.super_key = true,
            other => return Err(KeyParseError::UnknownModifier(other.to_string())),
        }
    }

    let key = parse_key(key_part)?;
    Ok(KeyChord { modifiers, key })
}

fn parse_key(s: &str) -> Result<Key, KeyParseError> {
    match s.to_lowercase().as_str() {
        "enter" | "return" | "cr" => Ok(Key::Enter),
        "backspace" | "bs" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "tab" => Ok(Key::Tab),
        "escape" | "esc" => Ok(Key::Escape),
        "space" => Ok(Key::Char(' ')),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" | "page_up" | "pgup" => Ok(Key::PageUp),
        "pagedown" | "page_down" | "pgdown" => Ok(Key::PageDown),
        "insert" | "ins" => Ok(Key::Insert),
        "minus" => Ok(Key::Char('-')),
        "slash" => Ok(Key::Char('/')),
        "backslash" => Ok(Key::Char('\\')),
        "dot" | "period" => Ok(Key::Char('.')),
        "comma" => Ok(Key::Char(',')),
        "semicolon" => Ok(Key::Char(';')),
        "colon" => Ok(Key::Char(':')),
        "quote" => Ok(Key::Char('\'')),
        "doublequote" => Ok(Key::Char('"')),
        "backtick" | "grave" => Ok(Key::Char('`')),
        "lbracket" => Ok(Key::Char('[')),
        "rbracket" => Ok(Key::Char(']')),
        "lparen" => Ok(Key::Char('(')),
        "rparen" => Ok(Key::Char(')')),
        "lbrace" => Ok(Key::Char('{')),
        "rbrace" => Ok(Key::Char('}')),
        "equal" | "equals" => Ok(Key::Char('=')),
        "plus" => Ok(Key::Char('+')),
        "asterisk" | "star" => Ok(Key::Char('*')),
        "ampersand" => Ok(Key::Char('&')),
        "percent" => Ok(Key::Char('%')),
        "dollar" => Ok(Key::Char('$')),
        "hash" | "pound" => Ok(Key::Char('#')),
        "at" => Ok(Key::Char('@')),
        "exclamation" | "bang" => Ok(Key::Char('!')),
        "caret" => Ok(Key::Char('^')),
        "tilde" => Ok(Key::Char('~')),
        "underscore" => Ok(Key::Char('_')),
        "pipe" => Ok(Key::Char('|')),
        s => {
            // Function keys: f1-f24
            if let Some(rest) = s.strip_prefix('f') {
                if let Ok(n) = rest.parse::<u8>() {
                    if (1..=24).contains(&n) {
                        return Ok(Key::F(n));
                    }
                }
            }
            // Single character
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => Ok(Key::Char(ch)),
                _ => Err(KeyParseError::UnknownKey(s.to_string())),
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_char() {
        let ch = parse_chord("p").unwrap();
        assert_eq!(ch.key, Key::Char('p'));
        assert!(!ch.modifiers.ctrl);
    }

    #[test]
    fn ctrl_p() {
        let ch = parse_chord("ctrl-p").unwrap();
        assert_eq!(ch.key, Key::Char('p'));
        assert!(ch.modifiers.ctrl);
        assert!(!ch.modifiers.shift);
    }

    #[test]
    fn ctrl_shift_p() {
        let ch = parse_chord("ctrl-shift-p").unwrap();
        assert_eq!(ch.key, Key::Char('p'));
        assert!(ch.modifiers.ctrl);
        assert!(ch.modifiers.shift);
    }

    #[test]
    fn named_keys() {
        assert_eq!(parse_chord("enter").unwrap().key, Key::Enter);
        assert_eq!(parse_chord("backspace").unwrap().key, Key::Backspace);
        assert_eq!(parse_chord("escape").unwrap().key, Key::Escape);
        assert_eq!(parse_chord("tab").unwrap().key, Key::Tab);
        assert_eq!(parse_chord("up").unwrap().key, Key::Up);
        assert_eq!(parse_chord("pagedown").unwrap().key, Key::PageDown);
    }

    #[test]
    fn function_keys() {
        assert_eq!(parse_chord("f1").unwrap().key, Key::F(1));
        assert_eq!(parse_chord("f12").unwrap().key, Key::F(12));
        assert_eq!(parse_chord("ctrl-f5").unwrap().key, Key::F(5));
    }

    #[test]
    fn named_special_chars() {
        assert_eq!(parse_chord("ctrl-minus").unwrap().key, Key::Char('-'));
        assert_eq!(parse_chord("ctrl-slash").unwrap().key, Key::Char('/'));
        assert_eq!(parse_chord("space").unwrap().key, Key::Char(' '));
    }

    #[test]
    fn unknown_modifier_errors() {
        assert_eq!(
            parse_chord("hyper-x"),
            Err(KeyParseError::UnknownModifier("hyper".to_string()))
        );
    }

    #[test]
    fn empty_errors() {
        assert_eq!(parse_chord(""), Err(KeyParseError::Empty));
    }
}
