# Lim

**Lightweight Vim alternative.** A modal editor in Rust with **zero dependencies**, built to feel like
an [AstroNvim](https://github.com/AstroNvim/AstroNvim) setup
([Neotoxic-off/neovim](https://github.com/Neotoxic-off/neovim)): same keymaps, same astrodark colors,
same statusline, tabline, picker, file tree, which-key and dashboard. One ~1 MB binary, starts in under a millisecond.

![Editing with the file tree, git signs, indent guides and reference highlighting](docs/screenshots/editor.png)

| Dashboard | Find files |
|---|---|
| ![Dashboard](docs/screenshots/dashboard.png) | ![File picker with preview](docs/screenshots/picker.png) |
| **Grep** | **which-key + splits** |
| ![Live grep](docs/screenshots/grep.png) | ![which-key menu over two splits](docs/screenshots/whichkey.png) |

---

## Contents

- [Features](#features)
- [Install](#install)
- [Usage](#usage)
- [Keymaps](#keymaps)
- [Configuration](#configuration)
- [How it is built](#how-it-is-built)
- [Limits](#limits)
- [Development](#development)

## Features

| Area | What you get |
|---|---|
| Editing | Normal, insert, visual, visual-line modes · counts · operators `d c y > < gc` with any motion or text object · `.` repeat · macros · undo/redo · unnamed register |
| Motions | `h j k l w b e 0 ^ $ { } % gg G f F t T n N * #`, half/full page scrolls |
| Text objects | `iw` `aw` `i(` `a(` `i[` `a[` `i{` `a{` `i<` `a<` `i"` `a"` `i'` `a'` `` i` `` `` a` `` |
| UI (AstroNvim look) | astrodark theme · NvChad-style statusline · buffer tabline with jump labels · winbar path · relative numbers · sign column · cursorline · rounded floats |
| Code | keyword highlighting (Rust, Lua, Python, C/C++, JS/TS, Go, shell, TOML/YAML/JSON) with multi-line comments and strings · autopairs · auto-indent · comment toggling |
| Visual helpers | indent guides · matching bracket · highlight of the word under the cursor · underlined URLs · smartcase search highlight |
| Git | gitsigns against the index (added / changed / deleted) · `]g` `[g` hunk jumps · branch in the statusline · status colors in the file tree |
| Files | snacks-style fuzzy picker with preview (files, git files, grep, buffers, recent files, lines, keymaps, commands) · neo-tree style explorer · dashboard |
| Windows | `|` / `\` splits, `<C-h/j/k/l>`, `<C-w>` family, global statusline |
| Config | one JSON file: Neovim option names, astrotheme palette, Neovim highlight groups, keymaps per mode |
| Robustness | atomic saves (temp file + rename, permissions and symlinks kept) · refuses to clobber other files · internal errors are caught, the session goes on and unsaved buffers are backed up |

## Install

Requirements: Rust 1.88+, Linux or macOS. A [Nerd Font](https://www.nerdfonts.com) for the icons
(or set `"icons": false`). A truecolor terminal is detected automatically; others get the 256-color palette.

```sh
git clone <this repo> lim && cd lim
cargo build --release
install -m 755 target/release/lim ~/.local/bin/lim
```

## Usage

```sh
lim                    # dashboard
lim src/main.rs a.lua  # open files
lim --print-config     # print the documented default config
lim --config-path      # where lim looks for config.json
lim --help | --version
```

Messages appear as notifications in the top-right corner (`cmdheight = 0`, as in AstroNvim); `:` and `/`
borrow the last line while you type.

## Keymaps

The leader is `<Space>`. Everything below matches AstroNvim's defaults unless marked *lim*.
Press `<Space>` (or `g`, `]`, `[`, `<C-w>`) and wait: which-key lists what comes next.

### General

| Keys | Action |
|---|---|
| `<Leader>w` / `<C-s>` | Save |
| `<Leader>q` | Quit window |
| `<Leader>Q` | Exit lim |
| `<C-q>` | Force quit |
| `<Leader>n` | New file |
| `<Leader>/` (normal, visual) · `gc{motion}` · `gcc` | Toggle comment |
| `<Leader>h` | Home screen (dashboard) |
| `<F7>` · `<Leader>tf` `th` `tv` | Terminal: suspends lim and opens `$SHELL`, `exit` comes back (*lim*: no embedded terminal) |

### Buffers

| Keys | Action |
|---|---|
| `<Tab>` `<S-Tab>` · `]b` `[b` | Next / previous buffer |
| `>b` `<b` | Move buffer tab right / left |
| `<Leader>c` / `<Leader>C` | Close / force close buffer |
| `<Leader>bb` · `<Leader>bd` | Pick a buffer by its tabline label, to open / close |
| `<Leader>b\|` · `<Leader>b\` | Pick a buffer into a vertical / horizontal split |
| `<Leader>bc` · `bC` · `bl` · `br` | Close others · all · to the left · to the right |
| `<Leader>bp` | Previous buffer |

### Windows

| Keys | Action |
|---|---|
| `\|` · `\` | Vertical · horizontal split |
| `<C-h>` `<C-j>` `<C-k>` `<C-l>` | Move between windows (and into the file tree) |
| `<C-w>v` `s` `c` `q` `o` `w` `h` `j` `k` `l` | Vim window commands |

### Find (picker)

| Keys | Action |
|---|---|
| `<Leader>ff` · `fF` | Files (git-aware) · all files including hidden |
| `<Leader>fg` | Git files |
| `<Leader>fw` · `fW` | Grep · grep in all files |
| `<Leader>fc` | Grep the word under the cursor |
| `<Leader>fb` · `fo` · `fl` | Buffers · recent files · lines of the buffer |
| `<Leader>fk` · `fC` | Keymaps · commands |
| `<Leader>f<CR>` | Resume the last search |

Inside the picker: type to filter, `<C-n>`/`<C-p>` (or arrows, `<C-j>`/`<C-k>`) to move, `<CR>` to open,
`<C-v>` / `<C-s>` to open in a vertical / horizontal split, `<Esc>` to close.

### File tree

`<Leader>e` toggles it, `<Leader>o` moves the focus in and out. Inside: `j` `k` move, `<CR>` / `l` / `o`
open a file or fold a directory, `h` closes a directory or jumps to its parent, `H` shows hidden files,
`R` refreshes, `q` closes, `<C-l>` / `<Esc>` back to the file.

### Git and UI toggles

| Keys | Action |
|---|---|
| `]g` `[g` · `<Leader>gg` `gG` | Next / previous git hunk |
| `<Leader>ua` | Toggle autopairs |
| `<Leader>ug` · `ul` · `ut` | Toggle sign column · statusline · tabline |
| `<Leader>un` | Cycle line numbering |
| `<Leader>uu` · `uy` · `u\|` | Toggle URL highlight · syntax highlight · indent guides |
| `<Leader>uN` · `uD` | Toggle · dismiss notifications |

### Vim

Everything usual: `i a I A o O`, `x X s S D C Y p P J ~ r`, `u <C-r>`, `v V o`, `:` commands
(`:w [file]` `:q` `:wq` `:x` `:wa` `:qa` `:e file` `:enew` `:bn` `:bp` `:bd` `:sp` `:vs` `:only` `:close` `:noh` `:<line>`,
`!` forces), `/pattern` with `\<word\>` boundaries, `q{reg}` … `q` and `@{reg}` / `@@`, `.`.
Insert mode: `<C-w>`, `<C-u>`, `<Tab>`, arrows.

## Configuration

lim reads `$LIM_CONFIG`, else `$XDG_CONFIG_HOME/lim/config.json`, else `~/.config/lim/config.json`.
The file is JSON with `//` comments and trailing commas allowed. Every section is optional; anything
invalid is reported in a notification and the default stays in place, so a broken config never stops the editor.
Start from the documented defaults:

```sh
mkdir -p ~/.config/lim && lim --print-config > ~/.config/lim/config.json
```

```jsonc
{
  "options": { "tabstop": 4, "relativenumber": false, "mapleader": "," },
  "palette": { "ui": { "base": "#11121d" }, "syntax": { "purple": "#bb9af7" } },
  "highlights": {
    "@comment": { "fg": "#565f89", "italic": false },
    "Visual": { "link": "Search" }
  },
  "keymaps": {
    "i": { "jk": "<Esc>" },
    "n": {
      "<Leader>x": { "rhs": ":x<CR>", "desc": "Save and quit" },
      "H": "^",
      "L": "$",
      "Q": false
    }
  }
}
```

### Options

Names and defaults follow AstroNvim.

| Option | Default | Meaning |
|---|---|---|
| `number` / `relativenumber` | `true` / `true` | Line numbers; both on shows the current line's absolute number |
| `signcolumn` | `true` | Two-cell column for git signs |
| `cursorline` | `true` | Highlight the cursor's line |
| `tabstop` | `2` | Width of a tab, and of `<Tab>` in insert mode |
| `shiftwidth` | `0` | Indent step for `>`, `<` and auto-indent; `0` uses `tabstop` |
| `expandtab` | `true` | Indent with spaces |
| `scrolloff` / `sidescrolloff` | `0` / `0` | Lines / columns kept around the cursor |
| `ignorecase` / `smartcase` | `true` / `true` | Search case rules |
| `hlsearch` | `true` | Highlight every match of the last search |
| `autopairs` | `true` | Close brackets and quotes as you type |
| `autoindent` | `true` | Keep (and after `{`, increase) indentation on new lines |
| `mapleader` | `" "` | Leader key, one key in notation (`","`, `"<Space>"`) |
| `timeoutlen` | `500` | Milliseconds to wait for the rest of a mapping |
| `showtabline` / `laststatus` / `winbar` | `true` | Tabline · statusline · per-window path bar |
| `cmdheight` | `0` | `0`: notifications and an on-demand command line; `1`: a permanent command line |
| `icons` | `true` | Nerd Font glyphs |
| `truecolor` | `"auto"` | `true`, `false` (256 colors) or `"auto"` |
| `highlighturl`, `indentguides`, `matchparen`, `gitsigns`, `wordhighlight`, `notifications`, `dashboard`, `syntax` | `true` | Turn each feature off |

### Palette

astrotheme's astrodark palette, overridable color by color under `palette.syntax` and `palette.ui`:

`syntax`: `red orange yellow green cyan blue purple text comment mute` ·
`ui`: `red orange yellow green cyan blue purple accent tabline winbar tool base inactive_base statusline split float title border current_line selection highlight none_text text text_active text_inactive prompt`

Overriding a palette color recolors every group derived from it, as in astrotheme.

### Highlight groups

Each group takes `fg`, `bg` (`"#RRGGBB"`), `bold`, `italic`, `underline`, or `link` to another group.

| Kind | Groups |
|---|---|
| Editor | `Normal` `NormalNC` `CursorLine` `LineNr` `CursorLineNr` `Visual` `Search` `CurSearch` `MatchParen` `LspReferenceText` `HighlightURL` `IblIndent` `Directory` |
| Syntax | `@variable` `@comment` `@string` `@number` `@keyword` `@function` `@type` `@type.builtin` `@constant` `@function.macro` `@property` |
| Statusline | `StatusLine` `StatusLineSection` `StatusLineNormal` `StatusLineInsert` `StatusLineVisual` `StatusLineCommand` `StatusLineCwd` `StatusLinePosition` `StatusLineRecording` |
| Tabline, windows | `TabLine` `TabLineSel` `TabLineFill` `TabLineIndicator` `TabLineModified` `BufferPicker` `WinSeparator` `WinBar` `WinBarNC` |
| Messages, floats | `MsgArea` `ErrorMsg` `NormalFloat` `FloatBorder` `FloatTitle` `PmenuSel` `SnacksNotifierInfo` `SnacksNotifierError` |
| Git | `GitSignsAdd` `GitSignsChange` `GitSignsDelete` |
| Plugins look-alikes | `WhichKey` `WhichKeyDesc` `WhichKeyGroup` `WhichKeySeparator` · `SnacksDashboardHeader` `SnacksDashboardKey` `SnacksDashboardDesc` `SnacksDashboardFooter` · `SnacksPickerMatch` `SnacksPickerDir` · `NeoTreeNormal` `NeoTreeRootName` `NeoTreeDirectoryIcon` `NeoTreeFileName` `NeoTreeIndentMarker` `NeoTreeFileNameOpened` `NeoTreeGitModified` `NeoTreeGitAdded` `NeoTreeGitUntracked` `NeoTreeGitDeleted` |

### Keymaps

`keymaps` holds one object per mode: `"n"` (normal), `"i"` (insert), `"v"` (visual), `"nv"` (both
normal and visual). Each entry maps a left-hand side to:

- a key sequence: `"<Leader>w": ":w<CR>"`
- an object with a which-key description: `"<Leader>w": { "rhs": ":w<CR>", "desc": "Save" }`
- `false` to disable a key: `"Q": false`

Right-hand sides run without remapping (`noremap`) and can call built-in mappings (`"<Leader>ff"`).
Key notation: plain characters, `<Leader>`, `<CR>`, `<Esc>`, `<BS>`, `<Tab>`, `<S-Tab>`, `<Space>`,
`<C-x>`, `<F1>`…`<F12>`, `<Up>` `<Down>` `<Left>` `<Right>`, `<Home>` `<End>`, `<Del>`, `<PageUp>`
`<PageDown>`, `<lt>`, `<Bar>`, `<Bslash>`.

## How it is built

No crates: the terminal is driven through a handful of libc calls (`tcsetattr`, `ioctl`, `select`, `read`,
`signal`), and everything else is the standard library.

```
src/
├── main.rs        startup, event loop, panic recovery
├── error.rs       Vim-worded errors (E32, E37, E492…)
├── term/          raw mode, window size, SIGWINCH, key decoding
├── buffer/        lines, undo history, motions, text objects, git signs, file I/O
├── editor/        modes, operators, keymaps, windows, picker, file tree, which-key, repeat/macros
├── syntax/        keyword highlighter with multi-line state, per-language tables
├── config/        JSON reader, key notation, options/palette/highlights/keymaps
└── render/        cell grid, theme, windows, bars, floats, picker, tree, dashboard
```

- **Rendering**: each frame is drawn into a grid of cells (floats are just later writes), then encoded to
  ANSI; only rows that changed since the last frame are sent, and frames are skipped while keys are queued.
- **Undo** stores the lines an edit replaced, not copies of the file; a typing run on one line costs one snapshot.
- **Highlighting** state at each line start is cached and invalidated from the first edited line.
- **Git signs** read the index once per open/save (`git show :./file`) and diff in memory when the text changes.
- **Saving** writes a temp file next to the target, syncs it, keeps the permissions, then renames it over the
  original; writing to another existing file needs `!`.
- **No crashes**: `unwrap`, `expect`, `panic!` and friends are denied by lints outside tests. If a bug still
  panics, the event loop catches it, repairs the editor state, copies unsaved buffers to the temp directory
  and tells you where, then keeps going.

## Limits

Not there (by design, to stay small): LSP, completion, treesitter (a keyword highlighter stands in), plugins,
an embedded terminal (`<F7>` suspends to your shell instead), line wrapping, mouse, folds, file operations in
the tree (create/rename/delete), sessions, persistent undo, Windows.

## Development

```sh
cargo test                     # 34 tests (incl. a random-key fuzz): motions, operators, keymaps, config, rendering at odd sizes…
cargo clippy --all-targets     # clean; unwrap/expect/panic are denied
```

Screenshots in `docs/screenshots/` are real lim frames captured in a pseudo-terminal and rasterised.
