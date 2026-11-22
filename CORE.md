# CORE.md — Master rules for all agents

> **Every agent MUST read this file before doing any work, and re-read it if more than ~10 turns have passed since the last read.** These rules supersede the agent-specific files when in conflict.

---

## 0. Project mission

**Luckybird** is a high-performance, modeless TUI code editor written in Rust. It targets parity-of-spirit with Neovim (keyboard-driven, programmable, LSP-rich) but with a Helix/VSCode-hybrid editing model and a hard focus on **lightweightness**.

Three numbers define the project. Every change must respect them:

| Budget | Limit | Measured by |
|---|---|---|
| Stripped release binary | **≤ 8 MB** | `cargo bloat --release` |
| Cold-start to first paint | **≤ 30 ms** (warm cache) | `hyperfine 'luckybird --headless +q'` |
| Idle RSS (one empty buffer) | **≤ 30 MB** | `/proc/self/status` after 1 s idle |

If a change pushes any of these past the limit, it is rejected unless explicitly approved by the user with a new budget.

---

## 1. Coordination protocol

1. **Before starting work**: read `CHANGES.md` (last 30 entries) to understand recent context. Read your own agent file (`docs/agents/<crate>.md`).
2. **Before touching another crate**: read that crate's agent file. If your change affects its public API, coordinate via a `CHANGES.md` entry tagged with both crate names.
3. **After finishing work**: append a new entry to `CHANGES.md` with a fresh ID (`LB-<4-digit>`, monotonic). Format spec is at the top of `CHANGES.md`.
4. **Never edit historical `CHANGES.md` entries.** Append-only. If something is wrong, write a new entry that supersedes it (`Supersedes: LB-0042`).
5. **One task = one ID.** If you split a task mid-flight, allocate a new ID for the spin-off and reference the parent (`Parent: LB-0042`).

---

## 2. Architectural rules (non-negotiable)

### 2.1 Lazy loading is the law

Nothing heavy runs at startup. The cold path loads only `lb-core`, `lb-config`, `lb-tui`. Anything else (`lb-syntax`, `lb-lsp`, `lb-lspmgr`, `lb-plugin`) is initialized **on first relevant event** behind `OnceCell` / `OnceLock`.

If you find yourself adding a `mod_init()` to `lb-app::main`, stop. Move it behind a trigger.

### 2.2 Crate dependency graph (one-way)

```
lb-app  ─┬─→ lb-core
         ├─→ lb-tui      ─→ lb-core
         ├─→ lb-config
         ├─→ lb-syntax   ─→ lb-core
         ├─→ lb-lsp      ─→ lb-core
         ├─→ lb-lspmgr   ─→ lb-config
         └─→ lb-plugin   ─→ lb-core, lb-config
```

- `lb-core` depends on nothing internal.
- No back-edges. No `lb-tui` → `lb-lsp` calls; route through `lb-app`.
- Cyclic deps fail review automatically.

### 2.3 No statically linked grammars or LSP servers

Tree-sitter grammars load as `.so`/`.dll`/`.dylib` from `~/.config/luckybird/grammars/`. LSP servers are external processes resolved from PATH or downloaded into the cache dir. **Never** `include_bytes!` a grammar or server binary into the editor.

### 2.4 Async runtime: single-threaded by default

Use `tokio` with `flavor = "current_thread"` unless you have a measured reason for `multi_thread`. Spawning a multi-thread runtime costs us ~2 MB RSS minimum and is rarely worth it for an editor.

### 2.5 Forbidden dependencies

These crates are banned (alternatives noted):

| Banned | Reason | Use instead |
|---|---|---|
| `reqwest` | Pulls hyper + tokio multi-thread + native-tls | `ureq` with `rustls-tls` |
| `regex` (in hot paths) | ~600 KB | `memchr`, hand-rolled, or `regex-lite` for non-hot |
| `lazy_static` | Replaced by std | `std::sync::OnceLock` / `LazyLock` |
| `chrono` | Bloat + CVE history | `time` crate, or `jiff` |
| `serde_json` (with default features) | OK, but use `--no-default-features` + `std` | — |
| `clap` derive | Pulls proc-macro chain on every build | `lexopt`, or `clap` with `default-features = false` |

Adding a new dependency requires a `CHANGES.md` entry justifying it and noting the binary-size delta from `cargo bloat`.

### 2.6 No panics in library crates

`lb-core`, `lb-tui`, `lb-syntax`, `lb-lsp`, `lb-lspmgr`, `lb-config`, `lb-plugin` must not `panic!`, `unwrap()`, `expect()`, or `[]`-index without bounds proof in non-test code. Return `Result` or `Option`. `lb-app` may `expect()` at startup with a meaningful message.

`#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]` at the top of every library crate's `lib.rs`.

---

## 3. Coding standards

- **Edition**: Rust 2024. MSRV: stable (whatever ships when you read this).
- **Format**: `cargo fmt --all` before every commit. CI rejects unformatted code.
- **Lint**: `cargo clippy --workspace --all-targets -- -D warnings`. No `#[allow(...)]` without a comment explaining why.
- **No comments unless they explain *why***. Names should explain *what*. If you wrote a comment that paraphrases the code, delete it.
- **No trailing summaries in PR descriptions** that paraphrase the diff. PR descriptions explain motivation; the diff is the spec.
- **Tests live next to code** (`#[cfg(test)] mod tests`) for unit; integration tests in `crates/<crate>/tests/`.
- **No `unsafe`** outside of FFI boundaries (`tree-sitter` grammar loading, `mlua`). Every `unsafe` block has a `// SAFETY:` comment.
- **No `String` allocation in hot loops** (render, keypress dispatch, edit op). Use `&str`, `Cow<str>`, `SmolStr`, or pre-allocated buffers.

---

## 4. Performance discipline

### 4.1 Measure before optimizing

`cargo bench` (criterion) for hot paths. Don't optimize from intuition. The render loop and edit ops are the only paths where micro-optimization is allowed without a benchmark.

### 4.2 Allocation budget

The render hot path (called every frame) must not allocate. The edit hot path (per keystroke) may allocate at most once (the rope edit itself). Use `dhat-rs` or `heaptrack` to verify.

### 4.3 Binary-size review

After every dependency change, run `cargo bloat --release --crates -n 20` and paste the top 20 into the `CHANGES.md` entry. If any single crate exceeds 1 MB, justify it.

---

## 5. Scope discipline

- **Don't add features the plan doesn't require.** No file-tree sidebar in v1. No git integration in v1. No DAP. No tabs UI (use buffer list). No telemetry, ever.
- **Don't refactor adjacent code** when fixing a bug. Open a separate task with its own `LB-` ID.
- **Don't add abstractions for hypothetical future cases.** Three concrete uses justify a trait; one or two do not.
- **Delete dead code.** No `#[allow(dead_code)]` to silence warnings. If it's not used, remove it.

---

## 6. Security

- **Plugin sandbox is load-bearing.** Lua plugins must not have raw `os.execute`, `io.open` outside their data dir, or arbitrary `require`. If you weaken the sandbox, escalate.
- **LSP downloads must verify SHA-256** against the registry. No "trust the server" — checksum or fail.
- **No outbound network calls** other than: (a) explicit LSP-server downloads requested by user, (b) explicit grammar downloads requested by user, (c) opt-in `:LspUpdate`. No analytics, no crash reporting, no auto-update of the editor itself.

---

## 7. When you're stuck or uncertain

1. Re-read the section of the plan (`plans/plan-a-high-performance-lucky-bird.md`) covering your area.
2. Re-read your agent file in `docs/agents/`.
3. Check `CHANGES.md` for a similar past decision.
4. If still unclear: ask the user. Do **not** guess at architectural questions.

---

## 8. Definition of done (every task)

A task is done only when **all** of these are true:

- [ ] Code compiles on stable Rust (`cargo build --workspace --release`).
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] Unit + integration tests pass (`cargo test --workspace`).
- [ ] Binary, startup, and RSS budgets still hold (Section 0).
- [ ] `CHANGES.md` has a new entry with the assigned `LB-` ID.
- [ ] No new banned dependencies (Section 2.5).
- [ ] No new `unwrap`/`expect`/`panic` in library crates (Section 2.6).
