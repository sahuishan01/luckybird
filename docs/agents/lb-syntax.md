# Agent: lb-syntax

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Provide tree-sitter-based syntax highlighting and structural queries (folding, indentation, text objects). Load grammars dynamically — never statically link them.

## In scope

- **Grammar loader**: `dlopen` (via `libloading`) of `.so`/`.dll`/`.dylib` from `~/.config/luckybird/grammars/<lang>.<ext>`.
- **Grammar registry**: maps language ID → grammar filename + symbol name + queries directory. Embedded as `include_str!("grammars.toml")`.
- **Grammar download** (on-demand): `:GrammarInstall <lang>` triggers download from `tree-sitter-<lang>` releases or a curated mirror. Verify SHA-256.
- **Highlight engine**: incremental, edit-aware. Reuse the parse tree across edits via `Tree::edit`.
- **Query execution**: highlight, indent, fold, text-object queries (loaded from `<grammar-dir>/queries/`).
- **Output**: `Vec<Highlight>` (byte range + capture name) consumed by `lb-tui`.

## Out of scope

- LSP semantic tokens — that's `lb-lsp` (and `lb-app` decides priority when both exist).
- Theming the highlight names → colors — that's `lb-config` + `lb-tui`.
- Compiling grammars from source. We download prebuilt artifacts only. If a user needs a missing grammar, they file an issue or write their own loader.

## Public API surface (target)

```rust
pub struct SyntaxEngine { /* loaded grammars cache */ }
pub struct Highlight { pub range: Range<usize>, pub capture: u32 }

impl SyntaxEngine {
    pub fn new() -> Self;                                // empty, lazy
    pub fn ensure_loaded(&mut self, lang: &str) -> Result<()>;
    pub fn parse(&mut self, lang: &str, buffer: &Buffer) -> Option<TreeId>;
    pub fn edit(&mut self, tree: TreeId, edit: &InputEdit);
    pub fn highlights(&self, tree: TreeId, range: Range<usize>) -> Vec<Highlight>;
}
```

## Dependencies

- `tree-sitter` (the runtime, not language crates)
- `libloading` (dlopen)
- `lb-core` (read-only buffer access)

**Banned**: `tree-sitter-rust`, `tree-sitter-python`, etc. as crate deps. We do **not** statically link grammars. Doing so violates [`CORE.md` § 2.3].

## Performance constraints

- Initial parse of a 10 KLOC file: **< 30 ms**.
- Incremental re-parse after a single keystroke: **< 1 ms**.
- Grammar load (dlopen + first parse cache fill): **< 10 ms**, called at most once per language per session.
- No parsing happens until `lb-app` opens a buffer of that language. Lazy is mandatory.

## Files owned

```
crates/lb-syntax/
├── Cargo.toml
├── grammars.toml       # registry of supported languages → grammar artifact info
├── src/
│   ├── lib.rs
│   ├── loader.rs       # dlopen glue
│   ├── registry.rs     # parse grammars.toml, resolve paths
│   ├── highlight.rs    # query execution
│   └── download.rs     # :GrammarInstall handler
└── tests/
    └── load_smoke.rs   # uses a fixture grammar shipped under tests/fixtures
```

## Coordination notes

- The set of capture names (`@function`, `@variable`, `@string.escape`, ...) is a contract between this crate and the theme in `lb-tui` / `lb-config`. Document it in `grammars.toml`. Adding a new capture name requires a `CHANGES.md` entry tagged `lb-syntax, lb-config, lb-tui`.
- `download.rs` shares HTTP/extraction logic with `lb-lspmgr`. Factor common code into a small `lb-net` utility crate **only if** both crates need ≥3 shared functions; otherwise duplicate (see CORE.md § 5).
