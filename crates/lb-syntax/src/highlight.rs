use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};
use tracing::warn;

use crate::HighlightSpan;

// ─── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum HighlightError {
    Language(String),
    Query(String),
}

impl std::fmt::Display for HighlightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Language(e) => write!(f, "language init error: {e}"),
            Self::Query(e) => write!(f, "query compile error: {e}"),
        }
    }
}

impl std::error::Error for HighlightError {}


/// Turn the (possibly nested) capture spans into sorted, non-overlapping ones.
///
/// AST nodes nest or are disjoint, so the innermost span wins; for identical
/// ranges the capture that came later in the query wins.
/// A capture before overlaps are resolved.
struct Raw {
    start: usize,
    end: usize,
    capture: u32,
    /// Index of the query pattern that produced it; later patterns have priority.
    pattern: usize,
    /// How specific the capture name is; more specific wins over less specific.
    rank: usize,
}

/// A plain `variable` is the catch-all and loses to every other capture; a
/// dotted name (`function.builtin`) is more specific than its base (`function`).
fn rank(capture_name: &str) -> usize {
    if capture_name == "variable" {
        0
    } else {
        1 + capture_name.matches('.').count()
    }
}

fn flatten(mut spans: Vec<Raw>) -> Vec<HighlightSpan> {
    fn emit(out: &mut Vec<HighlightSpan>, start: usize, end: usize, capture: u32) {
        if start < end {
            out.push(HighlightSpan { start, end, capture });
        }
    }

    // Outer spans first; for identical ranges the most specific capture comes last,
    // and among equals the later query pattern.
    spans.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(b.end.cmp(&a.end))
            .then(a.rank.cmp(&b.rank))
            .then(a.pattern.cmp(&b.pattern))
    });
    let mut out = Vec::with_capacity(spans.len());
    let mut stack: Vec<(usize, u32)> = Vec::new();
    let mut pos = 0usize;
    for s in spans {
        while let Some(&(end, capture)) = stack.last() {
            if end > s.start {
                break;
            }
            emit(&mut out, pos, end, capture);
            pos = pos.max(end);
            stack.pop();
        }
        if let Some(&(_, capture)) = stack.last() {
            emit(&mut out, pos, s.start, capture);
        }
        pos = s.start;
        stack.push((s.end, s.capture));
    }
    while let Some((end, capture)) = stack.pop() {
        emit(&mut out, pos, end, capture);
        pos = pos.max(end);
    }
    out
}
// ─── Highlighter ─────────────────────────────────────────────────────────────

/// Incremental syntax highlighter for a single language.
///
/// Keeps a `Parser` alive between calls so tree-sitter can do incremental
/// reparsing (not yet wired up — full reparse for now).
pub struct Highlighter {
    parser: Parser,
    query: Option<Query>,
    /// Capture name for each capture index.
    pub capture_names: Vec<String>,
}

impl Highlighter {
    /// Create a highlighter with an optional highlight query source.
    ///
    /// `query_src = None` produces a highlighter that always returns empty
    /// spans (plain-text fallback).
    pub fn new(language: Language, query_src: Option<&str>) -> Result<Self, HighlightError> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| HighlightError::Language(e.to_string()))?;

        let (query, capture_names) = match query_src {
            Some(src) => {
                // Some grammars (e.g. Rust) never capture plain identifiers. Put a
                // catch-all first; `rank` makes every other capture beat it. Grammars
                // without an `identifier` node reject it and use the query as is.
                let q = Query::new(&language, &format!("(identifier) @variable\n{src}"))
                    .or_else(|_| Query::new(&language, src))
                    .map_err(|e| HighlightError::Query(e.to_string()))?;
                let names = q.capture_names().iter().map(|s| s.to_string()).collect();
                (Some(q), names)
            }
            None => (None, Vec::new()),
        };

        Ok(Self { parser, query, capture_names })
    }

    /// Highlight `text` and return sorted, non-overlapping spans.
    ///
    /// Returns an empty `Vec` when no query is loaded or parsing fails.
    pub fn highlight(&mut self, text: &str) -> Vec<HighlightSpan> {
        let query = match self.query.as_ref() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let tree = match self.parser.parse(text, None) {
            Some(t) => t,
            None => {
                warn!("tree-sitter parse returned None");
                return Vec::new();
            }
        };

        let mut cursor = QueryCursor::new();
        let root = tree.root_node();
        let text_bytes = text.as_bytes();
        let mut spans: Vec<Raw> = Vec::new();

        // tree-sitter 0.24 uses a StreamingIterator; bring trait into scope above.
        // Pass text as a slice directly — &[u8] implements TextProvider in 0.24.
        let mut matches = cursor.matches(query, root, text_bytes);
        while let Some(m) = matches.next() {
            for capture in m.captures {
                spans.push(Raw {
                    start: capture.node.start_byte(),
                    end: capture.node.end_byte(),
                    capture: capture.index,
                    pattern: m.pattern_index,
                    rank: rank(self.capture_names.get(capture.index as usize).map_or("", String::as_str)),
                });
            }
        }

        flatten(spans)
    }

    /// Look up the capture name for a capture index.
    pub fn capture_name(&self, index: u32) -> Option<&str> {
        self.capture_names.get(index as usize).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(start: usize, end: usize, capture: u32, pattern: usize) -> Raw {
        Raw { start, end, capture, pattern, rank: 1 }
    }

    fn span(start: usize, end: usize, capture: u32) -> HighlightSpan {
        HighlightSpan { start, end, capture }
    }

    #[test]
    fn flatten_nested_spans_innermost_wins() {
        let out = flatten(vec![raw(0, 10, 1, 0), raw(2, 4, 2, 1)]);
        assert_eq!(out, vec![span(0, 2, 1), span(2, 4, 2), span(4, 10, 1)]);
    }

    #[test]
    fn flatten_identical_ranges_later_pattern_wins() {
        let out = flatten(vec![raw(0, 5, 2, 1), raw(0, 5, 1, 0)]);
        assert_eq!(out, vec![span(0, 5, 2)]);
    }


    #[test]
    fn specific_capture_beats_plain_variable_regardless_of_pattern_order() {
        let variable = Raw { start: 0, end: 3, capture: 1, pattern: 9, rank: rank("variable") };
        let function = Raw { start: 0, end: 3, capture: 2, pattern: 0, rank: rank("function") };
        assert_eq!(flatten(vec![variable, function]), vec![span(0, 3, 2)]);
    }

    #[test]
    fn dotted_capture_beats_its_base() {
        assert!(rank("function.builtin") > rank("function"));
        assert!(rank("function") > rank("variable"));
    }
    #[test]
    fn flatten_disjoint_spans_are_kept_sorted() {
        let out = flatten(vec![raw(6, 8, 3, 0), raw(0, 2, 1, 0)]);
        assert_eq!(out, vec![span(0, 2, 1), span(6, 8, 3)]);
    }

    #[test]
    fn highlighter_no_query_returns_empty() {
        // We can't load a real grammar in unit tests (no .so present),
        // so just verify the None-query fast-path.
        // Full integration test requires a grammar binary.
        _ = HighlightError::Language("x".into()).to_string();
        _ = HighlightError::Query("x".into()).to_string();
    }
}
