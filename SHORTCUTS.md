# luckybird keyboard shortcuts

luckybird is a **modal** editor (vim-style). On startup you are in **NORMAL**
mode — keys are commands. Press `i` (or any other insert chord) to enter
**INSERT** mode where typing inserts text. `Escape` returns to NORMAL.

The current mode is shown on the left of the status line: `[NORMAL]` or
`[INSERT]`.

## Mode switching

| Key | From mode | Action |
|-----|-----------|--------|
| `i` | Normal | Enter Insert mode at cursor |
| `a` | Normal | Move right one char, then enter Insert mode |
| `o` | Normal | Open a new line **below**, enter Insert mode |
| `O` | Normal | Open a new line **above**, enter Insert mode |
| `Escape` | Insert | Return to Normal mode |

## Normal mode

| Key | Action |
|-----|--------|
| `h` / `j` / `k` / `l` | Move left / down / up / right |
| `w` | Move word forward |
| `b` | Move word backward |
| `0` | Move to start of line |
| `$` | Move to end of line |
| `x` | Delete character under cursor |
| `u` | Undo |
| `Ctrl-r` | Redo |
| `q` | Quit (warns on unsaved changes) |

## Insert mode

| Key | Action |
|-----|--------|
| Any printable character | Insert |
| `Enter` | Insert newline |
| `Tab` | Insert tab (expanded per `expand_tabs` config) |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character after cursor |
| `Escape` | Return to Normal mode |

## Always-on (works in any mode)

These chords use `Ctrl`/`Alt` modifiers, so they don't conflict with letter
commands and work the same in Normal and Insert.

| Key | Action |
|-----|--------|
| Arrow keys, Home, End, PageUp, PageDown | Movement (always available) |
| `Ctrl-S` | Save |
| `Ctrl-Z` | Undo |
| `Ctrl-Y` | Redo |
| `Ctrl-Q` | Quit (warns on unsaved changes; press again to discard) |
| `Ctrl-Shift-Q` | *Note: Windows reports this as Ctrl+Q — use `F10` instead* |
| `F10` | Force-quit (no confirmation) |
| `Ctrl-P` | File picker *(stub)* |
| `Ctrl-Shift-P` | Command palette *(stub)* |
| `Ctrl-D` | Select next match *(stub)* |
| `Ctrl-/` | Toggle comment *(stub)* |
| `Ctrl-A` / `Ctrl-X` / `Ctrl-C` / `Ctrl-V` | Select-all / cut / copy / paste *(stubs)* |
| `Ctrl-W` / `Ctrl-N` | Close / new buffer *(stubs)* |
| `Ctrl-Tab` / `Ctrl-Shift-Tab` | Next / previous buffer *(stubs)* |

Stubs match the chord and show `"<Cmd>: not yet implemented"` in the status
bar — the press is registered, the action is just not wired up yet.

## Quitting

- **Clean buffer:** `q` (in Normal), `Ctrl-Q`, or `F10` exits immediately.
- **Dirty buffer:** First `q` / `Ctrl-Q` warns; press again to discard and exit.
  Or save with `Ctrl-S` first. `F10` always force-quits regardless.

## Rebinding

All chords live in `~/.config/luckybird/config.toml` (Linux/macOS) or
`%APPDATA%\luckybird\config.toml` (Windows). User entries override the
built-in defaults entry-by-entry; you don't have to redeclare them all.

```toml
[keys]
# Mode-prefixed chords (`normal:` or `insert:`) only fire in that mode.
# Plain chords are global.
"normal:H"        = "move_line_start"
"normal:L"        = "move_line_end"
"normal:ctrl-c"   = "force_quit"
"insert:ctrl-j"   = "insert_newline"
"f12"             = "hover"
"ctrl-space"      = "trigger_completion"
```

### Chord syntax

- Modifiers (separated by `-`): `ctrl`, `alt`, `shift`, `super` (= `cmd` / `win`).
- Named keys: `enter`, `backspace`, `delete`, `tab`, `escape`, `space`,
  `up` / `down` / `left` / `right`, `home`, `end`, `pageup`, `pagedown`,
  `insert`, `f1`–`f24`, plus named symbols (`minus`, `slash`, `dot`, `comma`, …).
- Single character: just the character itself, e.g. `a`, `O`, `$`, `7`.

### Letter case and the `shift` modifier

- For **letter keys** the case is the chord. `i` and `I` are different chords;
  `shift-i` is **not** a valid form — write `I` directly.
- For **symbol keys** the shifted glyph is the chord: `$` (Shift+4), not `shift-4`.
- For **non-character keys** (Tab, F-keys, arrows) shift is a real modifier:
  `shift-tab`, `shift-f5`.
- For **Ctrl/Alt + letter** the chord is always lowercase: `ctrl-q` works
  whether or not the OS reports the spurious shift that Windows tags onto
  every Ctrl+letter event.

## Implemented vs stubbed

Implemented commands actually run; stubbed commands show
`"<Cmd>: not yet implemented"` in the status bar so you can verify the chord
matched.

**Implemented:** all movement, insert/delete, undo/redo, save, quit,
force_quit, hover, mode switching (normal/insert), `o`/`O` open-line.

**Stubbed:** `file_picker`, `command_palette`, `search`, `search_replace`,
`select_all`, `cut`, `copy`, `paste`, `close_buffer`, `new_buffer`,
`next_buffer`, `prev_buffer`, `toggle_comment`, `add_cursor_above`,
`add_cursor_below`, `select_next_match`, `goto_definition`,
`trigger_completion`.
