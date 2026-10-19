# Development

Rust 1.88+, no dependencies. The code layout and design are in [How lim is built](architecture.md).

```sh
cargo test                     # 125+ tests: behaviour, fuzzers on every input, perf budgets
cargo clippy --all-targets     # pedantic, zero warnings; unwrap/expect/panic are denied
cargo fmt --check              # rustfmt.toml, 120 columns
cargo test --release perf -- --nocapture   # the benchmark table: run it before and after a change
LIM_FUZZ=30 cargo test --release fuzz      # a long fuzz session (LIM_FUZZ_SEED=<n> replays a failure)
cargo test --release big_files -- --ignored --nocapture   # 100 MB text + 64 MB binary: open, jump, search, edit, write
LIM_NVIM=nvim cargo test --release vs_neovim -- --ignored --nocapture   # lim against Neovim
cargo test gallery -- --ignored   # redraws docs/gallery
```

Before sending a change: tests and clippy pass on both Windows and Linux, the perf table did not regress, a
new key, command or parser is reachable by the fuzzers, and the matching page in `docs/` is updated.

## Releases

Every push to `master` runs `.github/workflows/release.yml`: `cargo fmt --check`, clippy and the tests on Linux,
macOS and Windows, then a GitHub release `vMAJOR.MINOR.N`, where `N` is the number of commits on master. The tag
points at a commit that writes that version into `Cargo.toml` (master keeps its own), so the binaries built from it
know their version. Each release carries `lim-x86_64-linux`, `lim-aarch64-macos`, `lim-x86_64-macos` and
`lim-x86_64-windows.exe`, the names `src/update.rs` asks for. Bump major or minor in `Cargo.toml` on master by
hand. `cargo test https -- --ignored` checks the in-process HTTPS against GitHub.

## No crash, no hang

`src/fuzz.rs`: fuzzers feed random input to every entry point and check that nothing
panics, hangs (a key over 3 s) or breaks the editor's invariants (cursor inside the text, valid buffers and
windows). Random keys in every mode and screen size, mixed with phrases that reach every picker, the settings,
`:g`, `:norm`, regexes, macros, visual block and so on, over Rust, Markdown, HTML, JSON, wide and combining
characters and a 10 000-char line; random Ex command lines; random regexes (checked against a plain scan); random
and corrupted config files; key notation; text helpers; Markdown and highlighting in ten languages. The long
sessions found and fixed real bugs: a crash in line ranges (`:co-x` on non-ASCII text), a stale selection after
`gv`, completion rescanning the file on every key, live grep re-reading every file on every key.

## Performance budgets

`src/perf.rs` times the hot paths on a 10 000-line file: startup, opening, scrolling,
typing with a frame per key, regex search, `:%s`, `:g`, `:g … norm`, undo, the Markdown preview and the lines
picker. Each has a budget of about 3-5x its debug time, so `cargo test` fails on a real slowdown.

## Big files

`big_files`: a 100 MB, 3-million-line Rust file and a 64 MB binary (the hex limit), opened,
browsed, edited and written back, with the bytes on disk checked. It found `G` scanning the whole file for
highlighting (1.2 s, now 1.3 ms). Linux in WSL, release build:

| Operation | 100 MB text | 64 MB binary (hex) |
|---|---|---|
| Open + first frame | 176 ms | 487 ms |
| `G` + frame | 2.7 ms | 2.1 ms |
| Search to the last line | 872 ms | |
| Type at the top / edit a byte, + frame | 2.0 ms | 2.8 ms |
| Undo + redo | 2.0 ms | |
| `:detect` · `:strings` | | 301 ms · 309 ms |
| `:xor` one line | | 983 ms |
| `:w` | 253 ms | 380 ms |

## Against Neovim

`vs_neovim`: the same operations on the same 10 000-line file, timed inside each editor, best
of 5, Neovim 0.12 with `-u NONE` (no config, no plugins: its fastest). lim must win every row:

| Operation | lim | Neovim | lim is |
|---|---|---|---|
| Open the file | 0.28 ms | 0.57 ms | 2.0x faster |
| `:%s/\v(\w+): (\w+)/\2 \1/g` | 6.4 ms | 13.9 ms | 2.2x |
| `:g/TODO/d` | 0.43 ms | 1.34 ms | 3.1x |
| `:g/item/norm Ax` | 3.8 ms | 29.1 ms | 7.7x |
| Regex search, `n` x50 | 0.28 ms | 0.77 ms | 2.8x |
| Type 500 chars | 0.05 ms | 0.07 ms | 1.3x |
| Undo + redo 300 steps | 0.82 ms | 45.2 ms | 55x |
| `:sort` | 0.73 ms | 10.4 ms | 14x |

(Linux in WSL, release build.)

Screenshots are real lim frames captured in a pseudo-terminal and rasterised.
