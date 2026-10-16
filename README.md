# Lim
Lightweight Vim alternative, in Rust with zero dependencies. It mirrors the
[Neotoxic-off/neovim](https://github.com/Neotoxic-off/neovim) AstroNvim setup:
astrodark colors, NvChad-style statusline, buffer tabline, `number` + `relativenumber`,
`signcolumn=yes`, cursorline, `nowrap`, 2-space indent, autopairs, smartcase search.

```sh
cargo build --release   # ~500 KB binary
./target/release/lim file.rs other.lua
```

Runs on Linux and macOS terminals. Truecolor when the terminal advertises it, 256 colors otherwise.
Icons need a Nerd Font.

## Keys

| Mode | Keys |
|------|------|
| Motions | `h j k l` arrows `w b e 0 ^ $ { } gg G f F t T n N` `Ctrl-d/u/f/b`, counts (`3w`) |
| Operators | `d c y > <` + motion, doubled for lines (`dd`, `>>`), `x X s S D C Y p P J ~ r` |
| Edit | `i a I A o O`, `u`, `Ctrl-r` |
| Visual | `v V`, then `d x y c s > < o` |
| Buffers | `Tab` / `Shift-Tab` next/prev, `Space b d` or `Space c` close, `Space n` new |
| Files | `Ctrl-s` / `Space w` save, `Space q` quit, `Ctrl-q` force quit, `ZZ`, `ZQ` |
| Commands | `:w [file]` `:q` `:wq` `:x` `:wa` `:qa` `:e file` `:enew` `:bn` `:bp` `:bd` `:noh` `:N` (`!` forces) |
| Search | `/pattern` then `n` / `N` |

Insert mode: `Esc`, `Ctrl-w`, `Ctrl-u`, `Tab` (spaces), bracket/quote pairing, auto-indent.

## Not included

LSP, completion, treesitter (a keyword highlighter covers Rust, Lua, Python, C/C++, JS/TS, Go,
shell and config files), plugins, file tree, fuzzy finder, dashboard, which-key, macros, `.` repeat,
text objects (`iw`, `i(`), splits.
