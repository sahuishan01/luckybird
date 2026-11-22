# Agent: lb-tui

> **Read [`CORE.md`](../../CORE.md) first.**

## Mission

Render buffers, selections, status line, command palette, and pickers to the terminal. Translate raw terminal events into editor commands. Be the only crate that talks to the terminal.

## In scope

- **Render loop**: ratatui-based, double-buffered, diff-based redraw. Aim for < 1 ms per frame on a typical buffer view.
- **Event translation**: `crossterm::event::Event` → `EditorEvent` (keypress, mouse, paste, resize). Key codes are normalized; modifier handling consistent across platforms.
- **Pickers**: file picker, buffer picker, command palette, symbol picker. All use `nucleo` for fuzzy matching. One generic `Picker<T>` widget — don't duplicate.
- **Status line + gutter + line numbers**: composable widgets.
- **Bottom prompt** (Helix-style `:` commands and search).
- **Theme application**: receives a `Theme` struct from `lb-config`, applies to styles.

## Out of scope

- The buffer itself (`lb-core`).
- Highlight computation (`lb-syntax` produces highlight ranges; we just style them).
- LSP-completion menu *content* (`lb-lsp` provides items; we render).
- File I/O, process spawning, anything async.

## Public API surface (target)

```rust
pub struct Tui { /* terminal handle, layout state */ }
pub enum EditorEvent { Key(Key), Mouse(...), Resize(u16, u16), Paste(String) }

impl Tui {
    pub fn enter() -> io::Result<Self>;       // raw mode, alt screen
    pub fn leave(self) -> io::Result<()>;     // restore on drop too
    pub fn next_event(&mut self) -> io::Result<EditorEvent>;
    pub fn render(&mut self, view: &View) -> io::Result<()>;
}

pub struct View<'a> {
    pub buffer: &'a Buffer,
    pub selection: &'a Selection,
    pub highlights: &'a [Highlight],
    pub diagnostics: &'a [Diagnostic],
    pub status: &'a StatusLine,
}
```

`View` is borrowed and constructed per frame by `lb-app`. Don't store it.

## Dependencies

- `ratatui`
- `crossterm` (matched ratatui backend version)
- `nucleo` (fuzzy matcher — Helix's, not `fuzzy-matcher`)
- `lb-core` (read-only — no mutation of buffers from here)
- nothing else

## Performance constraints

- **No allocations in the render hot path.** Pre-allocate line buffers, reuse them. Use `dhat-rs` to verify.
- Per-frame render budget: **≤ 1 ms** on a 1000-line visible viewport with highlights.
- Event translation: **≤ 100 µs**.

## Files owned

```
crates/lb-tui/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── render.rs       # render() + widget composition
│   ├── event.rs        # crossterm → EditorEvent
│   ├── picker.rs       # generic Picker<T>
│   ├── prompt.rs       # bottom prompt for : commands and /
│   ├── statusline.rs
│   └── theme.rs
└── tests/
    └── golden.rs       # snapshot-test renders against golden text
```

## Coordination notes

- This crate must stay event-driven and pull-style: `lb-app` calls `next_event` and `render`. Don't spawn tasks here.
- If a feature seems to need async (e.g. completion-menu fetch), the async lives in `lb-lsp` or `lb-app`; this crate only renders the result.
- Theme schema is shared with `lb-config`. Define the `Theme` struct in `lb-tui` and have `lb-config` deserialize into it (avoid duplicating).
