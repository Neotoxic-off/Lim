# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

lim: a Vim/AstroNvim-style modal editor in Rust (edition 2024, MSRV 1.88) with **zero crates**, for Linux, macOS
and Windows. The terminal is driven by raw FFI: libc in `src/term/sys/unix.rs`, kernel32 in `src/term/sys/windows.rs`.
Both expose the same functions, selected with `#[cfg]` + `#[path]` in `term/mod.rs`. Never add a dependency.

## Commands

The dev machine is Windows, where `cargo` builds and tests natively. Platform code (`cfg(unix)` / `cfg(windows)`)
must also be checked on Linux in WSL. From Git Bash, prefix with `MSYS_NO_PATHCONV=1`. Use `-e`, because the WSL
login shell is fish. Keep the target dir off `/mnt` so builds stay fast:

```sh
wsl -d Ubuntu --cd /mnt/d/Code/Github/Lim -e bash -lc 'CARGO_TARGET_DIR=$HOME/.lim-target cargo clippy --all-targets && CARGO_TARGET_DIR=$HOME/.lim-target cargo test'
cargo test                      # includes the fuzzers (src/fuzz.rs) and the perf budgets (src/perf.rs)
cargo test <name>               # one test, e.g. cargo test operators_undo_and_paste
cargo clippy --all-targets      # pedantic (see [lints] in Cargo.toml); must stay at zero warnings
cargo fmt                       # rustfmt.toml: 120 columns
cargo llvm-cov --summary-only   # per-file coverage
cargo test --release perf -- --nocapture   # benchmark table (src/perf.rs): run before and after every change
LIM_FUZZ=30 cargo test --release fuzz      # a long fuzz session; LIM_FUZZ_SEED=<n> replays one failing seed
cargo test --release big_files -- --ignored --nocapture   # 100 MB text + 64 MB binary: open, jump, search, edit, write
# in WSL, run long fuzz sessions from a copy outside /mnt (the pickers read the project files, slow over /mnt)
# against Neovim (installed in WSL at ~/.local/nvim): lim must win every operation
wsl -d Ubuntu --cd /mnt/d/Code/Github/Lim -e bash -lc 'LIM_NVIM=$HOME/.local/nvim/bin/nvim CARGO_TARGET_DIR=$HOME/.lim-target cargo test --release vs_neovim -- --ignored --nocapture'
```

## Architecture

- `main.rs`: event loop. Both drawing and key handling run inside `catch_unwind`. On a panic it repairs the editor,
  backs up unsaved buffers to the temp dir and keeps going.
- `editor/` holds all state (`Editor` in `editor/mod.rs`) and its mode handlers (`normal`, `insert`, `visual`,
  `operator`, `prompt`, `command`, ...). `render/` only reads `Editor` and draws it into a cell grid. Only the rows
  that changed are sent to the terminal.
- `editor/bindings.rs` is the single table of built-in prefixed keys. The dispatcher, which-key, cheat sheet and
  keymaps picker all read from it, so add or change keys there and nowhere else.
- `buffer/`: text lines plus an undo `History` that records each replaced line range. The same ranges shift marks
  and jump-list entries, so a new kind of edit has to go through it.
- `syntax/`: a keyword highlighter that caches its per-line start state and invalidates it from the first edited
  line.
- `config/`: a hand-written JSON parser feeding `Options`. `lim --print-config` prints the documented defaults
  (also in `config.example.json`).

## Rules

- `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` and `unimplemented!` are denied outside tests (crate
  attribute in `main.rs`). Return `Error` (`src/error.rs`, Vim-worded such as E37 or E492) or handle the case.
- Clippy pedantic is on. Silence a lint only with `#[expect(clippy::…, reason = "…")]` at the item, never with
  `allow`. Encode choices as enums (`Toggle`, `SortBy`, `Align`, `{ forward: bool }`), not `char` or sign codes. Use
  `try_from` instead of `as` casts.
- Every `let` has an explicit type (`let x: T = …`), including tuples and `let … else`. The only exceptions are
  closures and anonymous iterator types, which Rust cannot name. Name types through a `use`, not a full path; for
  std errors write `io::Result<T>` / `fmt::Result`, never a bare `Error` (that name is the crate's).
- Portability: user folders come from `config::path()` and `config::state_dir()`, and resolved paths from
  `buffer::canonical` (it drops Windows' `\\?\` prefix). Never read `$HOME` or call `fs::canonicalize` directly.
  Pick platform behaviour with `cfg!(windows)` in the code, or a `#[cfg]` module when the APIs differ.
- Under `cfg(test)`, `may_touch` only allows paths in the temp dir, `may_spawn` is false, and session and recent
  file storage is disabled. Tests must use `std::env::temp_dir()`.
- Tests live in `#[cfg(test)]` modules next to the code. Editor behaviour tests go in `editor/tests.rs`, which uses
  helpers that feed key strings (`keys(&mut ed, "dw")`) and check the resulting text.
- When you add a feature, also update README.md (features table, keymaps, limits).
- Benchmark every change: run `cargo test --release perf -- --nocapture` before and after, and compare. The
  `perf` test fails on a budget overrun; a new hot path (per key, per frame, per line) gets a case there.
- lim stays faster than Neovim (`-u NONE`, no plugins) on every operation `vs_neovim` compares; run it after any
  change to editing, search, Ex commands or undo, and add a row when lim gains an operation both can do.
- No input may crash or hang lim. A new entry point (a key, a mode, a command, a parser, a file format) gets
  covered by `src/fuzz.rs`: a phrase that reaches it in `PHRASES`, or its own fuzzer. A fuzz failure prints its
  seed; fix the cause and add the input as a regression test in `editor/tests.rs`.

## Coding guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
