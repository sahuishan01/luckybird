use ropey::Rope;

// ─── Edit ────────────────────────────────────────────────────────────────────

/// A single text replacement: remove `start..end` bytes and insert `new_text`.
/// A pure insert has `start == end`; a pure delete has `new_text` empty.
#[derive(Debug, Clone)]
pub struct Edit {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    pub new_text: String,
}

impl Edit {
    pub fn insert(at: usize, text: impl Into<String>) -> Self {
        Self { start: at, end: at, new_text: text.into() }
    }

    pub fn delete(start: usize, end: usize) -> Self {
        Self { start, end, new_text: String::new() }
    }

    pub fn replace(start: usize, end: usize, text: impl Into<String>) -> Self {
        Self { start, end, new_text: text.into() }
    }
}

// ─── Change ──────────────────────────────────────────────────────────────────

/// The recorded outcome of a successful `Buffer::apply`, used to undo/redo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Byte offset where the edit began.
    pub start: usize,
    /// Text that was removed.
    pub old_text: String,
    /// Text that was inserted.
    pub new_text: String,
}

impl Change {
    /// End byte of `new_text` in the post-edit buffer.
    pub fn new_end(&self) -> usize {
        self.start + self.new_text.len()
    }

    /// End byte of `old_text` in the pre-edit buffer.
    pub fn old_end(&self) -> usize {
        self.start + self.old_text.len()
    }
}

// ─── EditError ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    OutOfBounds,
    InvalidRange,
    NotCharBoundary,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds => write!(f, "byte offset out of buffer bounds"),
            Self::InvalidRange => write!(f, "edit start > end"),
            Self::NotCharBoundary => write!(f, "byte offset not on UTF-8 char boundary"),
        }
    }
}

impl std::error::Error for EditError {}

// ─── Buffer ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    pub fn from_str(s: &str) -> Self {
        Self { rope: Rope::from(s) }
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Number of lines including the trailing empty line after a final newline.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    pub fn line(&self, idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if idx < self.rope.len_lines() {
            Some(self.rope.line(idx))
        } else {
            None
        }
    }

    /// 0-based line index for a given byte offset.
    pub fn byte_to_line(&self, byte: usize) -> Option<usize> {
        let char_idx = self.rope.try_byte_to_char(byte).ok()?;
        Some(self.rope.char_to_line(char_idx))
    }

    /// Byte offset of the start of a given line.
    pub fn line_to_byte(&self, line: usize) -> Option<usize> {
        if line > self.rope.len_lines() {
            return None;
        }
        let char_idx = self.rope.line_to_char(line);
        Some(self.rope.char_to_byte(char_idx))
    }

    /// Apply an edit, returning the `Change` needed to undo it.
    #[must_use = "drop Change only if you don't need undo"]
    pub fn apply(&mut self, edit: &Edit) -> Result<Change, EditError> {
        if edit.start > edit.end {
            return Err(EditError::InvalidRange);
        }
        if edit.end > self.rope.len_bytes() {
            return Err(EditError::OutOfBounds);
        }
        let start_char = self.rope.try_byte_to_char(edit.start)
            .map_err(|_| EditError::NotCharBoundary)?;
        let end_char = self.rope.try_byte_to_char(edit.end)
            .map_err(|_| EditError::NotCharBoundary)?;

        let old_text: String = self.rope.slice(start_char..end_char).into();

        if start_char < end_char {
            self.rope.remove(start_char..end_char);
        }
        if !edit.new_text.is_empty() {
            self.rope.insert(start_char, &edit.new_text);
        }

        Ok(Change { start: edit.start, old_text, new_text: edit.new_text.clone() })
    }

    /// Reverse a previously recorded change (undo one step).
    pub fn revert(&mut self, change: &Change) -> Result<(), EditError> {
        let new_end = change.start
            .checked_add(change.new_text.len())
            .ok_or(EditError::OutOfBounds)?;
        if new_end > self.rope.len_bytes() {
            return Err(EditError::OutOfBounds);
        }
        let start_char = self.rope.try_byte_to_char(change.start)
            .map_err(|_| EditError::NotCharBoundary)?;
        let end_char = self.rope.try_byte_to_char(new_end)
            .map_err(|_| EditError::NotCharBoundary)?;

        if start_char < end_char {
            self.rope.remove(start_char..end_char);
        }
        if !change.old_text.is_empty() {
            self.rope.insert(start_char, &change.old_text);
        }
        Ok(())
    }

    /// Iterate over text chunks (used by syntax / LSP layers, no allocation).
    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        self.rope.chunks()
    }

    /// Full text as a `String` (allocates — avoid in hot paths).
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Read-only rope access for intra-crate movement helpers.
    pub(crate) fn rope(&self) -> &Rope {
        &self.rope
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_at_start() {
        let mut buf = Buffer::from_str("world");
        let ch = buf.apply(&Edit::insert(0, "hello ")).unwrap();
        assert_eq!(buf.to_string(), "hello world");
        buf.revert(&ch).unwrap();
        assert_eq!(buf.to_string(), "world");
    }

    #[test]
    fn delete_range() {
        let mut buf = Buffer::from_str("hello world");
        let ch = buf.apply(&Edit::delete(5, 11)).unwrap();
        assert_eq!(buf.to_string(), "hello");
        buf.revert(&ch).unwrap();
        assert_eq!(buf.to_string(), "hello world");
    }

    #[test]
    fn replace() {
        let mut buf = Buffer::from_str("foo bar");
        let _ = buf.apply(&Edit::replace(4, 7, "baz")).unwrap();
        assert_eq!(buf.to_string(), "foo baz");
    }

    #[test]
    fn out_of_bounds_returns_err() {
        let mut buf = Buffer::from_str("hi");
        assert_eq!(buf.apply(&Edit::delete(1, 99)), Err(EditError::OutOfBounds));
    }

    #[test]
    fn invalid_range_returns_err() {
        let mut buf = Buffer::from_str("hi");
        assert_eq!(buf.apply(&Edit::delete(2, 1)), Err(EditError::InvalidRange));
    }

    #[test]
    fn line_navigation() {
        let buf = Buffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.len_lines(), 3);
        assert_eq!(buf.byte_to_line(0), Some(0));
        assert_eq!(buf.byte_to_line(6), Some(1));
        assert_eq!(buf.line_to_byte(1), Some(6));
    }
}
