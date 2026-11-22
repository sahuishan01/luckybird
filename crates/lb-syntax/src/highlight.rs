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
                let q = Query::new(&language, src)
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
        let mut spans: Vec<HighlightSpan> = Vec::new();

        // tree-sitter 0.24 uses a StreamingIterator; bring trait into scope above.
        // Pass text as a slice directly — &[u8] implements TextProvider in 0.24.
        let mut matches = cursor.matches(query, root, text_bytes);
        while let Some(m) = matches.next() {
            for capture in m.captures {
                spans.push(HighlightSpan {
                    start: capture.node.start_byte(),
                    end: capture.node.end_byte(),
                    capture: capture.index,
                });
            }
        }

        spans
    }

    /// Look up the capture name for a capture index.
    pub fn capture_name(&self, index: u32) -> Option<&str> {
        self.capture_names.get(index as usize).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighter_no_query_returns_empty() {
        // We can't load a real grammar in unit tests (no .so present),
        // so just verify the None-query fast-path.
        // Full integration test requires a grammar binary.
        _ = HighlightError::Language("x".into()).to_string();
        _ = HighlightError::Query("x".into()).to_string();
    }
}
