//! UTF-8 byte offset ↔ UTF-16 code-unit offset conversions.
//!
//! LSP uses UTF-16 code units by default. Servers that advertise
//! `positionEncoding = "utf-8"` can use the byte offsets directly, but many
//! major servers (rust-analyzer, clangd) still default to UTF-16.

/// Convert a UTF-8 byte offset to a UTF-16 code-unit offset.
///
/// Returns `None` if `byte_offset` is out of bounds or falls in the middle
/// of a multi-byte UTF-8 sequence.
pub fn utf8_to_utf16(text: &str, byte_offset: usize) -> Option<usize> {
    if byte_offset > text.len() {
        return None;
    }
    if byte_offset < text.len() && !text.is_char_boundary(byte_offset) {
        return None;
    }
    let mut utf16 = 0usize;
    let mut byte_pos = 0usize;
    for ch in text.chars() {
        if byte_pos >= byte_offset {
            break;
        }
        utf16 += ch.len_utf16();
        byte_pos += ch.len_utf8();
    }
    Some(utf16)
}

/// Convert a UTF-16 code-unit offset to a UTF-8 byte offset.
///
/// Returns `None` if the offset falls inside a surrogate pair or beyond the
/// end of `text`.
pub fn utf16_to_utf8(text: &str, utf16_offset: usize) -> Option<usize> {
    let mut remaining = utf16_offset;
    let mut byte_pos = 0usize;
    for ch in text.chars() {
        if remaining == 0 {
            return Some(byte_pos);
        }
        let utf16_len = ch.len_utf16();
        if remaining < utf16_len {
            // Offset is inside a surrogate pair — not a valid position.
            return None;
        }
        remaining -= utf16_len;
        byte_pos += ch.len_utf8();
    }
    if remaining == 0 { Some(byte_pos) } else { None }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_roundtrip() {
        let text = "hello world";
        for i in 0..=text.len() {
            let utf16 = utf8_to_utf16(text, i).unwrap();
            assert_eq!(utf16_to_utf8(text, utf16).unwrap(), i);
        }
    }

    #[test]
    fn multibyte_emoji() {
        // "A😀B": A = 1 byte/1 UTF-16, 😀 = 4 bytes/2 UTF-16, B = 1 byte/1 UTF-16
        let text = "A\u{1F600}B";
        assert_eq!(text.len(), 6);
        assert_eq!(utf8_to_utf16(text, 0), Some(0)); // before A
        assert_eq!(utf8_to_utf16(text, 1), Some(1)); // before 😀
        assert_eq!(utf8_to_utf16(text, 5), Some(3)); // before B
        assert_eq!(utf8_to_utf16(text, 6), Some(4)); // end

        assert_eq!(utf16_to_utf8(text, 0), Some(0));
        assert_eq!(utf16_to_utf8(text, 1), Some(1));
        assert_eq!(utf16_to_utf8(text, 3), Some(5));
        assert_eq!(utf16_to_utf8(text, 4), Some(6));

        // UTF-16 offset 2 is inside the surrogate pair → None
        assert_eq!(utf16_to_utf8(text, 2), None);
    }

    #[test]
    fn out_of_bounds() {
        let text = "hi";
        assert_eq!(utf8_to_utf16(text, 99), None);
        assert_eq!(utf16_to_utf8(text, 99), None);
    }

    #[test]
    fn mid_multibyte_utf8_returns_none() {
        let text = "\u{00E9}"; // é = 2 bytes
        assert_eq!(utf8_to_utf16(text, 1), None); // middle of é
    }
}
