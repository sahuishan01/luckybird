# Agent: lb-core

> **Read [`CORE.md`](../../CORE.md) first.** That file's rules supersede this one.

## Mission

Own the in-memory representation of edited text. Provide buffer, cursor, selection, and undo primitives that the rest of the editor builds on. Be allocation-light, panic-free, and dependency-light.

## In scope

- **Buffer**: rope-backed text storage (`ropey`), line/byte/char index conversions, slicing, edit application.
- **Selection model**: a list of `Range`s (start, end, anchor). Multi-cursor is the default — single-cursor is "list of length 1".
- **Cursor movement primitives**: char-left/right, word-left/right, line-up/down, doc-start/end, find-next/prev. Movement is pure functions over `Buffer + Selection`.
- **Edit operations**: insert, delete, replace. Each returns a `Change` value the undo tree records.
- **Undo tree**: not a flat stack — a tree (Helix-style). Branches when the user types after an undo.
- **Encoding**: UTF-8 internal, with helpers for UTF-16 offsets (LSP requires both).

## Out of scope

- File I/O — that's `lb-app`'s job (it picks how to load: mmap, read, async).
- Syntax highlighting — `lb-syntax`.
- Rendering — `lb-tui`.
- Anything async. This crate is sync, single-threaded, no `tokio`.

## Public API surface (target)

```rust
pub struct Buffer { /* ropey + metadata */ }
pub struct Selection { /* Vec<Range>, primary index */ }
pub struct Range { pub anchor: usize, pub head: usize }   // byte offsets
pub struct Change { /* for undo */ }

impl Buffer {
    pub fn new() -> Self;
    pub fn from_str(s: &str) -> Self;
    pub fn len_bytes(&self) -> usize;
    pub fn len_lines(&self) -> usize;
    pub fn line(&self, idx: usize) -> Option<RopeSlice>;
    pub fn slice(&self, range: Range) -> RopeSlice;
    pub fn apply(&mut self, edit: &Edit) -> Change;
    pub fn revert(&mut self, change: &Change);
}
```

Stabilize this once two consumers (`lb-tui`, `lb-lsp`) are using it. Until then, refactor freely.

## Dependencies

- `ropey` (mandatory)
- `smallvec` (selection list — usually 1, sometimes 50)
- nothing else internal

## Performance constraints

- A single-character insert at a random offset: **< 1 µs** on a 10 MB buffer (criterion bench).
- Allocation count for `apply(insert)`: **≤ 1** (the rope's internal allocation).
- Loading a 100 MB file into a `Buffer`: **< 100 ms**.

Benchmarks live in `crates/lb-core/benches/`. CI runs them and fails on regression > 10 %.

## Files owned

```
crates/lb-core/
├── Cargo.toml
├── src/
│   ├── lib.rs          # re-exports, lints
│   ├── buffer.rs       # Buffer, Edit, Change
│   ├── selection.rs    # Range, Selection, movement primitives
│   ├── undo.rs         # undo tree
│   └── encoding.rs     # UTF-8 ↔ UTF-16 offset helpers
└── benches/
    └── edit.rs
```

## Coordination notes

- API changes here ripple everywhere. Before changing a public signature, scan for callers in `lb-tui`, `lb-syntax`, `lb-lsp`, `lb-app` and tag all of them in your `CHANGES.md` entry.
- The undo tree shape will be needed by a future `:undo-tree` command in `lb-app`. Design it serializable (serde) but don't actually serialize until that command lands.
