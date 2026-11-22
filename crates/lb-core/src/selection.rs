use smallvec::SmallVec;
use crate::buffer::Buffer;

// ─── Range ───────────────────────────────────────────────────────────────────

/// A cursor or selection expressed as byte offsets.
///
/// `anchor` is where the selection started; `head` is where the cursor sits.
/// When `anchor == head` the range represents a bare cursor with no selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub anchor: usize,
    pub head: usize,
}

impl Range {
    /// A zero-width cursor at `pos`.
    pub fn point(pos: usize) -> Self {
        Self { anchor: pos, head: pos }
    }

    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// Inclusive lower bound of the selected bytes.
    pub fn start(self) -> usize {
        self.anchor.min(self.head)
    }

    /// Exclusive upper bound of the selected bytes.
    pub fn end(self) -> usize {
        self.anchor.max(self.head)
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }

    pub fn is_reversed(self) -> bool {
        self.head < self.anchor
    }
}

impl From<Range> for std::ops::Range<usize> {
    fn from(r: Range) -> Self {
        r.start()..r.end()
    }
}

// ─── Selection ───────────────────────────────────────────────────────────────

static EMPTY_RANGE: Range = Range { anchor: 0, head: 0 };

/// A (possibly multi-cursor) selection.
///
/// Invariant: `ranges` is never empty; `primary < ranges.len()`.
/// Both invariants are upheld by all constructors and mutating methods.
#[derive(Debug, Clone)]
pub struct Selection {
    ranges: SmallVec<[Range; 1]>,
    primary: usize,
}

impl Selection {
    /// A single zero-width cursor.
    pub fn point(pos: usize) -> Self {
        Self {
            ranges: SmallVec::from_elem(Range::point(pos), 1),
            primary: 0,
        }
    }

    /// A single range selection.
    pub fn single(anchor: usize, head: usize) -> Self {
        Self {
            ranges: SmallVec::from_elem(Range::new(anchor, head), 1),
            primary: 0,
        }
    }

    pub fn primary(&self) -> &Range {
        self.ranges.get(self.primary).unwrap_or(&EMPTY_RANGE)
    }

    pub fn primary_mut(&mut self) -> Option<&mut Range> {
        let primary = self.primary;
        self.ranges.get_mut(primary)
    }

    pub fn ranges(&self) -> &[Range] {
        &self.ranges
    }

    /// Byte position of the primary cursor head.
    pub fn cursor(&self) -> usize {
        self.primary().head
    }

    /// Add a new range and make it primary.
    pub fn add_range(&mut self, range: Range) {
        self.ranges.push(range);
        self.primary = self.ranges.len() - 1;
    }

    /// Set which range is primary (panics in debug if idx is out of range).
    pub fn set_primary(&mut self, idx: usize) {
        debug_assert!(idx < self.ranges.len());
        self.primary = idx.min(self.ranges.len().saturating_sub(1));
    }

    /// Produce a new `Selection` by applying `f` to every range.
    pub fn map<F>(&self, f: F) -> Self
    where
        F: Fn(&Range) -> Range,
    {
        Self {
            ranges: self.ranges.iter().map(f).collect(),
            primary: self.primary,
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::point(0)
    }
}

// ─── Movement primitives ─────────────────────────────────────────────────────
//
// Each function takes an immutable (buf, sel) pair and returns the new
// Selection. Multi-cursor: every range is transformed independently.

pub fn move_char_left(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| Range::point(prev_char_boundary(buf, r.head)))
}

pub fn move_char_right(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| Range::point(next_char_boundary(buf, r.head)))
}

pub fn move_line_start(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| {
        let line = buf.byte_to_line(r.head).unwrap_or(0);
        Range::point(buf.line_to_byte(line).unwrap_or(0))
    })
}

pub fn move_line_end(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| Range::point(line_end_byte(buf, buf.byte_to_line(r.head).unwrap_or(0))))
}

pub fn move_doc_start(_sel: &Selection, _buf: &Buffer) -> Selection {
    Selection::point(0)
}

pub fn move_doc_end(_sel: &Selection, buf: &Buffer) -> Selection {
    Selection::point(buf.len_bytes())
}

pub fn move_line_up(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| {
        let cur_line = buf.byte_to_line(r.head).unwrap_or(0);
        if cur_line == 0 {
            return Range::point(0);
        }
        let col = r.head.saturating_sub(buf.line_to_byte(cur_line).unwrap_or(0));
        let prev_line = cur_line - 1;
        let prev_start = buf.line_to_byte(prev_line).unwrap_or(0);
        let prev_content_end = line_end_byte(buf, prev_line);
        let prev_len = prev_content_end.saturating_sub(prev_start);
        Range::point(prev_start + col.min(prev_len))
    })
}

pub fn move_line_down(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| {
        let cur_line = buf.byte_to_line(r.head).unwrap_or(0);
        let next_line = cur_line + 1;
        if next_line >= buf.len_lines() {
            return Range::point(buf.len_bytes());
        }
        let col = r.head.saturating_sub(buf.line_to_byte(cur_line).unwrap_or(0));
        let next_start = buf.line_to_byte(next_line).unwrap_or(0);
        let next_content_end = line_end_byte(buf, next_line);
        let next_len = next_content_end.saturating_sub(next_start);
        Range::point(next_start + col.min(next_len))
    })
}

pub fn move_word_forward(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| Range::point(skip_word_forward(buf, r.head)))
}

pub fn move_word_backward(sel: &Selection, buf: &Buffer) -> Selection {
    sel.map(|r| Range::point(skip_word_backward(buf, r.head)))
}

// ─── Internal helpers ────────────────────────────────────────────────────────

fn prev_char_boundary(buf: &Buffer, byte: usize) -> usize {
    if byte == 0 {
        return 0;
    }
    let rope = buf.rope();
    match rope.try_byte_to_char(byte) {
        Ok(char_idx) if char_idx > 0 => rope.char_to_byte(char_idx - 1),
        _ => 0,
    }
}

fn next_char_boundary(buf: &Buffer, byte: usize) -> usize {
    let rope = buf.rope();
    let len = rope.len_bytes();
    if byte >= len {
        return len;
    }
    match rope.try_byte_to_char(byte) {
        Ok(char_idx) if char_idx < rope.len_chars() => rope.char_to_byte(char_idx + 1),
        _ => len,
    }
}

/// End byte of line content, excluding the trailing `\n` / `\r\n`.
fn line_end_byte(buf: &Buffer, line_idx: usize) -> usize {
    let line_start = buf.line_to_byte(line_idx).unwrap_or(0);
    let raw_text = buf.line(line_idx).map(|s| s.to_string()).unwrap_or_default();
    let trimmed = raw_text.trim_end_matches(['\n', '\r']);
    line_start + trimmed.len()
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn skip_word_forward(buf: &Buffer, byte: usize) -> usize {
    let rope = buf.rope();
    let len = rope.len_chars();
    let start_char = match rope.try_byte_to_char(byte) {
        Ok(c) => c,
        Err(_) => return buf.len_bytes(),
    };
    let mut idx = start_char;
    // Advance past any current word chars.
    while idx < len && is_word_char(rope.char(idx)) {
        idx += 1;
    }
    // Advance past any non-word chars to land on the next word.
    while idx < len && !is_word_char(rope.char(idx)) {
        idx += 1;
    }
    rope.char_to_byte(idx)
}

fn skip_word_backward(buf: &Buffer, byte: usize) -> usize {
    let rope = buf.rope();
    let char_end = match rope.try_byte_to_char(byte) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    if char_end == 0 {
        return 0;
    }
    let mut idx = char_end;
    // Step back past any non-word chars.
    while idx > 0 && !is_word_char(rope.char(idx - 1)) {
        idx -= 1;
    }
    // Step back past word chars to find the start of the word.
    while idx > 0 && is_word_char(rope.char(idx - 1)) {
        idx -= 1;
    }
    rope.char_to_byte(idx)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> Buffer {
        Buffer::from_str(s)
    }

    #[test]
    fn point_selection() {
        let s = Selection::point(4);
        assert_eq!(s.cursor(), 4);
        assert!(s.primary().is_empty());
    }

    #[test]
    fn move_right_left() {
        let b = buf("hello");
        let s = Selection::point(0);
        let s2 = move_char_right(&s, &b);
        assert_eq!(s2.cursor(), 1);
        let s3 = move_char_left(&s2, &b);
        assert_eq!(s3.cursor(), 0);
    }

    #[test]
    fn move_right_clamped() {
        let b = buf("hi");
        let s = Selection::point(2);
        assert_eq!(move_char_right(&s, &b).cursor(), 2);
    }

    #[test]
    fn line_up_down() {
        let b = buf("abc\ndef\nghi");
        let s = Selection::point(4); // start of "def"
        let up = move_line_up(&s, &b);
        assert_eq!(up.cursor(), 0); // start of "abc"
        let down = move_line_down(&s, &b);
        assert_eq!(down.cursor(), 8); // start of "ghi"
    }

    #[test]
    fn word_forward_backward() {
        let b = buf("hello world");
        let s = Selection::point(0);
        let s2 = move_word_forward(&s, &b);
        assert_eq!(s2.cursor(), 6); // 'w' in "world"
        let s3 = move_word_backward(&s2, &b);
        assert_eq!(s3.cursor(), 0);
    }

    #[test]
    fn line_end_no_newline() {
        let b = buf("abc\nde");
        let s = Selection::point(0);
        let end = move_line_end(&s, &b);
        assert_eq!(end.cursor(), 3); // after 'c', before '\n'
    }

    #[test]
    fn doc_start_end() {
        let b = buf("hello\nworld");
        let s = Selection::point(3);
        assert_eq!(move_doc_start(&s, &b).cursor(), 0);
        assert_eq!(move_doc_end(&s, &b).cursor(), 11);
    }
}
