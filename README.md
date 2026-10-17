<p align="center">
  <img src="docs/logo.png" alt="lim: the Lernaean Hydra" width="280">
</p>

<p align="center">
  <b>A lightweight modal editor, in Rust, with zero dependencies.</b><br>
  AstroNvim's keymaps, look and workflow in one ~1 MB binary that starts in under a millisecond.
</p>

---

**The symbol.** lim's emblem is the Lernaean Hydra, the many-headed serpent of the lake (λίμνη, *limnē*)
of Lerna: one body, many heads, like one editor working many windows. It greets you, in plain ASCII, on
the start screen.

![Editing: file tree, git signs, indent guides, TODO and color highlights](docs/screenshots/editor.png)

| Start screen | Find files |
|---|---|
| ![Start screen with the hydra](docs/screenshots/dashboard.png) | ![File picker with preview](docs/screenshots/picker.png) |
| **Grep** | **which-key over two splits** |
| ![Live grep](docs/screenshots/grep.png) | ![which-key menu](docs/screenshots/whichkey.png) |
| **Cheat sheet** (`<F1>`) | **Theme switcher** (`<Leader>ft`, live preview) |
| ![Cheat sheet](docs/screenshots/cheatsheet.png) | ![Theme picker](docs/screenshots/themes.png) |
| **Completion** (words from open buffers) | **Command-line completion** (tabs aligned with the windows) |
| ![Completion menu](docs/screenshots/completion.png) | ![Command-line completion](docs/screenshots/cmdline.png) |

## Contents

- [Features](#features)
- [Install](#install)
- [Usage](#usage)
- [Keymaps](#keymaps)
- [Themes and icons](#themes-and-icons)
- [Configuration](#configuration)
- [Plugins](#plugins)
- [How it is built](#how-it-is-built)
- [Limits](#limits)
- [Development](#development)

## Features

| Area | What you get |
|---|---|
| Vim editing | Normal, insert, replace, visual, visual-line and visual-block modes · counts · operators `d c y > < gu gU g~ gc` over any motion or text object · `.` · macros · undo/redo (1000 levels) · named registers (`"a`, `"A` append, `"0`, `"1`, `"-`, `"_`, `"+`) · marks and jump list that follow edits · `:s` and line ranges |
| Motions | `h j k l w b e ge W B E gE 0 ^ $ g_ { } % gg G H M L f F t T ; , n N * #`, page and line scrolling, `<S-arrows>` (word / page), `zz zt zb` · `:` and `/` history |
| Text objects | `iw` `aw` `i(` `a(` `i[` `a[` `i{` `a{` `i<` `a<` `i"` `a"` `i'` `a'` `` i` `` `` a` `` |
| AstroNvim look | 30 colorschemes (NvChad's onedark by default) · NvChad-style statusline (rounded, slanted, arrow or flat ends) · buffer tabline with jump labels, aligned with the windows when the file tree is open · winbar path · relative numbers · sign column · cursorline · rounded floats · notifications · start screen |
| Code | keyword highlighting for 40+ languages and formats (Rust, Python, JS/TS, C/C++, C#, Java, Kotlin, Go, PHP, Ruby, Swift, Dart, Scala, Lua, SQL, HTML/XML, CSS/SCSS, Markdown, JSON/YAML/TOML, shell, PowerShell, batch, Makefile, Dockerfile, CMake, Zig, Haskell, Elixir, R, Perl, asm, Nix, Terraform...) with multi-line comments and strings, HTML tags and config keys · blink.cmp-style completion from the words of open buffers · autopairs · auto-indent · comment toggling |
| Helpers | indent guides · matching bracket · references of the word under the cursor (`]r`) · URLs (`gx`) · `#RRGGBB` shown in its color · `TODO:`/`FIX:`/`HACK:`… highlighted (`]T`) |
| Git | signs against the index · hunk jumps · branches, log, file log, status and stash pickers · `git browse` · lazygit in one key · status colors in the file tree |
| Files | fuzzy picker with preview (files, git files, grep, buffers, recent, smart, lines, marks, registers, notifications, keymaps, commands, TODOs, sessions) · neo-tree style explorer with create/rename/copy/move/delete · sessions |
| Windows | `\|` / `\` splits, `<C-h/j/k/l>`, `<C-arrows>` to resize, `<C-w>` family, zen mode, global statusline |
| System | system clipboard (`clipboard=unnamedplus`, OSC 52 fallback over SSH) · `<F7>` shell, lazygit, node, python, btm, gdu |
| Config | settings screen (`<Leader>,`) to search and change every option live and save it · one JSON file: Neovim option names, colorscheme, astrotheme palette, Neovim highlight groups, keymaps per mode, icon set |
| Plugins | a folder with a `plugin.json` that adds options, colors, keymaps and `:Commands` · a command pipes the lines through a program in any language and puts its output back or shows it |
| Robustness | atomic saves · refuses to clobber other files · internal errors are caught, the session goes on and unsaved buffers are backed up · `unwrap`/`panic!` denied by lints |

## Install

Requirements: Rust 1.88+, Linux, macOS or Windows 10+ (Windows Terminal or the console, not mintty). A [Nerd Font](https://www.nerdfonts.com) for the default icons
(or `"iconset": "unicode"` / `"ascii"`). Truecolor is detected; other terminals get the 256-color palette.

```sh
git clone <this repo> lim && cd lim
cargo build --release
install -m 755 target/release/lim ~/.local/bin/lim   # Windows: copy target\release\lim.exe onto your PATH
```

Optional, picked up when installed: `wl-copy`/`xclip`/`xsel`/`pbcopy` (clipboard; Windows uses its own),
`lazygit`, `node`, `python3`, `btm`, `gdu`, `xdg-open`/`open` (`gx`, `<Leader>go`; Windows uses `explorer`).

## Usage

```sh
lim                    # start screen
lim src/main.rs a.lua  # open files
lim ../project         # work in that folder (explorer, finder, git, sessions)
lim --print-config     # the documented default config
lim --config-path      # where lim looks for config.json
lim --help | --version
```

Messages appear as notifications in the top-right corner (`cmdheight = 0`, as in AstroNvim); `:` and `/`
borrow a line while you type, pushing the statusline up as Neovim does.

The `:` line completes as you type: command names, then file paths for `:e`, `:w`, `:sp`,
`:vs`. `<Tab>`/`<C-n>` and `<S-Tab>`/`<C-p>`
cycle through the menu, writing the choice into the line; `<CR>` runs it. Press `<F1>` (or `<Leader>fh`, `:help`) for the cheat sheet, or press
`<Space>` and wait for which-key.

## Keymaps

The leader is `<Space>`. Unless marked *lim*, everything matches AstroNvim's defaults, plus the
`<Tab>`/`<S-Tab>` buffer navigation of the reference config.

### General

| Keys | Action |
|---|---|
| `<Leader>w` · `<C-s>` | Save · force write |
| `<Leader>q` · `<Leader>Q` · `<C-q>` | Quit window · exit · force quit |
| `<Leader>n` | New file |
| `<Leader>R` | Rename file |
| `<Leader>/` (normal, visual) · `gc{motion}` · `gcc` | Toggle comment |
| `gco` · `gcO` | Add a comment line below / above |
| `<Leader>h` | Home screen |
| `<F1>` · `<Leader>fh` · `<Leader>?` | Cheat sheet (*lim*) |
| `<Leader>,` | Settings: theme, icons, every option (*lim*) |

### Buffers

| Keys | Action |
|---|---|
| `<Tab>` `<S-Tab>` · `]b` `[b` | Next / previous buffer |
| `>b` `<b` | Move buffer tab right / left |
| `<Leader>c` · `<Leader>C` | Close · force close buffer |
| `<Leader>bb` · `<Leader>bd` | Pick a buffer by its tabline label to open / close |
| `<Leader>b\|` · `<Leader>b\` | Pick a buffer into a vertical / horizontal split |
| `<Leader>bc` · `bC` · `bl` · `br` | Close others · all · to the left · to the right |
| `<Leader>bp` | Previous buffer |
| `<Leader>bse` `bsr` `bsp` `bsi` `bsm` | Sort by extension · relative path · full path · number · modification |

### Windows

| Keys | Action |
|---|---|
| `\|` · `\` | Vertical · horizontal split |
| `<C-h>` `<C-j>` `<C-k>` `<C-l>` | Move between windows (and into the file tree) |
| `<C-Up>` `<C-Down>` `<C-Left>` `<C-Right>` | Resize the split |
| `<C-w>v` `s` `c` `q` `o` `w` `h` `j` `k` `l`, `<C-w><arrows>` | Vim window commands |

### Find

| Keys | Action |
|---|---|
| `<Leader>ff` · `fF` · `fg` | Files · all files · git files |
| `<Leader>fw` · `fW` · `fc` | Grep · grep in all files · grep the word under the cursor |
| `<Leader>fb` · `fo` · `fO` · `fs` | Buffers · old files · old files in this directory · buffers/recent/files |
| `<Leader>fl` · `f'` · `fr` · `fn` | Lines · marks · registers · notifications |
| `<Leader>fk` · `fC` · `fT` | Keymaps · commands · TODO comments |
| `<Leader>ft` · `fI` · `fa` | Colorschemes (live preview) · icon sets (*lim*) · lim config files |
| `<Leader>f<CR>` | Resume the last search |

Inside a picker: type to filter, `<C-n>`/`<C-p>` (or arrows, `<C-j>`/`<C-k>`) to move, `<CR>` to open,
`<C-v>`/`<C-s>` to open in a vertical/horizontal split, `<Esc>` to close.

### Git

| Keys | Action |
|---|---|
| `]g` `[g` · `]G` `[G` | Next / previous hunk · last / first hunk |
| `<Leader>gg` | lazygit |
| `<Leader>gb` · `gc` · `gC` · `gt` · `gT` | Branches (switch) · log · log of this file · status · stash |
| `<Leader>go` | Open the file and line on the git host |

### File tree

`<Leader>e` toggles it, `<Leader>o` moves the focus in and out. Inside, as in neo-tree:

| Keys | Action |
|---|---|
| `j` `k` · `<CR>` `l` `o` · `h` | Move · open file or fold directory · close directory / go to parent |
| `a` · `A` · `r` · `d` | Add file (`name/` for a directory) · add directory · rename · delete (asks) |
| `c` · `m` · `y` · `Y` | Copy · move · copy name · copy path |
| `s` · `S` · `P` · `O` | Open in vertical / horizontal split · preview · open with the system |
| `.` · `<BS>` · `H` · `R` | Make root · root up · show hidden · refresh |
| `q` · `<C-l>` `<Esc>` · `?` | Close · back to the file · cheat sheet |

### Terminal and tools

`<F7>`, `<Leader>tf`, `<Leader>th`, `<Leader>tv` suspend lim and open your `$SHELL` (`%COMSPEC%` on Windows; `exit` comes back);
`<Leader>tl` lazygit, `<Leader>tn` node, `<Leader>tp` python, `<Leader>tt` btm, `<Leader>tu` gdu.
Files changed meanwhile are reloaded. *lim*: there is no embedded terminal.

### Sessions

`<Leader>Sl` last session · `Ss` save (named) · `SS` save for this directory · `S.` load this directory's ·
`Sf` / `SF` pick a session / directory session · `Sd` / `SD` delete one. The last session is saved on exit.

### UI toggles

| Keys | Action |
|---|---|
| `<Leader>ua` · `up` · `uc`/`uC` | Autopairs · paste mode · autocompletion |
| `<Leader>ub` | Light / dark variant of the theme |
| `<Leader>ug` · `u>` · `ul` · `ut` | Sign column · fold column · statusline · tabline |
| `<Leader>un` · `ui` | Cycle line numbering · indent width (prompt) |
| `<Leader>ur` · `uu` · `uz` · `uy` · `u\|` | References · URLs · colors · syntax · indent guides |
| `<Leader>uN` · `uD` · `uZ` | Notifications · dismiss them · zen mode |

### Vim

Everything usual, including: `R` replace mode, `<C-a>`/`<C-x>`, `gu`/`gU`/`g~`, `gJ`, `gi`, `gI`, `gv`,
`gx`, `ge`, `g_`, `H`/`M`/`L`, `zz`/`zt`/`zb`, `<C-e>`/`<C-y>`, `m{a-z}` with `'`/`` ` ``, `''`, `<C-o>`,
`"{register}`, `q{r}`/`@{r}`/`@@`, `?`, `;`/`,`, `&`. Visual block: `I`, `A`, `d`, `c`, `y`, `r`, `~`/`u`/`U`, `o`/`O`,
blockwise put. Insert mode: `<C-r>{r}`, `<C-o>`, `<C-t>`/`<C-d>`, `<C-w>`, `<C-u>`.
`<S-Left>`/`<S-Right>` move a word and `<S-Up>`/`<S-Down>` a page, in normal, visual (to grow the selection fast),
insert and after an operator. On the `:` and `/` lines, `<Up>`/`<Down>` recall older / newer entries that start with
what is typed (separate histories for commands and searches).
Marks, `''` and `<C-o>` stay on their text when lines above are added or removed (and after undo);
a letter mark goes away with its line, as in Vim.

**Completion** (blink.cmp keys as AstroNvim maps them): words from the open buffers appear while you type,
current buffer first. `<Tab>`/`<C-n>`/`<C-j>`/`<Down>` next, `<S-Tab>`/`<C-p>`/`<C-k>`/`<Up>` previous,
`<CR>` accepts the selected item (with nothing selected it is a plain newline), `<C-e>` hides,
`<C-Space>` shows the menu on demand.
Commands: `:w [file]` `:q` `:wq` `:x` `:wa` `:qa` `:e file` `:enew` `:bn` `:bp` `:bd` `:sp` `:vs` `:only`
`:close` `:noh` `:help` `:settings` (or `:setup`), `:[range]s/a/b/[g]` (literal pattern, `&` = match),
`:[range]d` `y` `>` `<` `j`, and addresses `N`, `.`, `$`, `%`, `'a`, `'<,'>`. Commands that start with an
uppercase letter come from [plugins](#plugins).

## Themes and icons

Every picture below is a real frame drawn by lim's renderer (`cargo test gallery -- --ignored` redraws them).

**Colorschemes** (30): colors come from each theme's own repository, mapped onto the astrotheme palette lim
draws with. nvchad (NvChad's base46 onedark) is the default.

|   |   |   |
|---|---|---|
| <img src="docs/gallery/nvchad.svg" alt="nvchad" width="300"> | <img src="docs/gallery/astrodark.svg" alt="astrodark" width="300"> | <img src="docs/gallery/astrolight.svg" alt="astrolight" width="300"> |
| `nvchad` | `astrodark` | `astrolight` |
| <img src="docs/gallery/astromars.svg" alt="astromars" width="300"> | <img src="docs/gallery/astrojupiter.svg" alt="astrojupiter" width="300"> | <img src="docs/gallery/catppuccin-mocha.svg" alt="catppuccin-mocha" width="300"> |
| `astromars` | `astrojupiter` | `catppuccin-mocha` |
| <img src="docs/gallery/catppuccin-macchiato.svg" alt="catppuccin-macchiato" width="300"> | <img src="docs/gallery/catppuccin-frappe.svg" alt="catppuccin-frappe" width="300"> | <img src="docs/gallery/catppuccin-latte.svg" alt="catppuccin-latte" width="300"> |
| `catppuccin-macchiato` | `catppuccin-frappe` | `catppuccin-latte` |
| <img src="docs/gallery/tokyonight-night.svg" alt="tokyonight-night" width="300"> | <img src="docs/gallery/tokyonight-storm.svg" alt="tokyonight-storm" width="300"> | <img src="docs/gallery/tokyonight-moon.svg" alt="tokyonight-moon" width="300"> |
| `tokyonight-night` | `tokyonight-storm` | `tokyonight-moon` |
| <img src="docs/gallery/tokyonight-day.svg" alt="tokyonight-day" width="300"> | <img src="docs/gallery/dracula.svg" alt="dracula" width="300"> | <img src="docs/gallery/onedark.svg" alt="onedark" width="300"> |
| `tokyonight-day` | `dracula` | `onedark` |
| <img src="docs/gallery/gruvbox.svg" alt="gruvbox" width="300"> | <img src="docs/gallery/nord.svg" alt="nord" width="300"> | <img src="docs/gallery/nordic.svg" alt="nordic" width="300"> |
| `gruvbox` | `nord` | `nordic` |
| <img src="docs/gallery/night-owl.svg" alt="night-owl" width="300"> | <img src="docs/gallery/sonokai.svg" alt="sonokai" width="300"> | <img src="docs/gallery/kanagawa.svg" alt="kanagawa" width="300"> |
| `night-owl` | `sonokai` | `kanagawa` |
| <img src="docs/gallery/rose-pine.svg" alt="rose-pine" width="300"> | <img src="docs/gallery/rose-pine-moon.svg" alt="rose-pine-moon" width="300"> | <img src="docs/gallery/rose-pine-dawn.svg" alt="rose-pine-dawn" width="300"> |
| `rose-pine` | `rose-pine-moon` | `rose-pine-dawn` |
| <img src="docs/gallery/everforest.svg" alt="everforest" width="300"> | <img src="docs/gallery/gruvbox-light.svg" alt="gruvbox-light" width="300"> | <img src="docs/gallery/solarized-dark.svg" alt="solarized-dark" width="300"> |
| `everforest` | `gruvbox-light` | `solarized-dark` |
| <img src="docs/gallery/solarized-light.svg" alt="solarized-light" width="300"> | <img src="docs/gallery/github-dark.svg" alt="github-dark" width="300"> | <img src="docs/gallery/ayu-dark.svg" alt="ayu-dark" width="300"> |
| `solarized-light` | `github-dark` | `ayu-dark` |

Switch in the settings (`<Leader>,`), with `<Leader>ft` (the editor recolors as you move through the list;
`<Esc>` puts the old one back), or `<Leader>ub` for the light/dark sibling. The last one picked is remembered; set
`"colorscheme"` in the config to pin one. `palette` and `highlights` overrides apply on top of any theme.

**Icon sets**: `nerd` (default, Nerd Font devicons as AstroNvim), `unicode` (any font), `emoji` (color emoji,
two cells each) and `ascii`. Switch in the settings (`<Leader>,`), `<Leader>fI`, or the `iconset` option.

|   |   |
|---|---|
| <img src="docs/gallery/icons-nerd.svg" alt="nerd" width="300"> | <img src="docs/gallery/icons-unicode.svg" alt="unicode" width="300"> |
| `nerd` | `unicode` |
| <img src="docs/gallery/icons-emoji.svg" alt="emoji" width="300"> | <img src="docs/gallery/icons-ascii.svg" alt="ascii" width="300"> |
| `emoji` | `ascii` |

**Statusline styles** (`statusline_style`, Nerd Font icon set; the pictures need a Nerd Font installed to
show the section ends):

|   |   |
|---|---|
| <img src="docs/gallery/statusline-rounded.svg" alt="rounded" width="300"> | <img src="docs/gallery/statusline-slanted.svg" alt="slanted" width="300"> |
| `rounded` | `slanted` |
| <img src="docs/gallery/statusline-arrow.svg" alt="arrow" width="300"> | <img src="docs/gallery/statusline-flat.svg" alt="flat" width="300"> |
| `arrow` | `flat` |

## Configuration

lim reads `$LIM_CONFIG`, else `$XDG_CONFIG_HOME/lim/config.json`, else `~/.config/lim/config.json`
(`%APPDATA%\lim\config.json` on Windows; `lim --config-path` prints it). Sessions, recent files and the last
colorscheme go to `$XDG_STATE_HOME/lim`, `~/.local/state/lim` or `%LOCALAPPDATA%\lim`.
JSON with `//` comments and trailing commas. Every section is optional; anything invalid is reported in a
notification and the default stays, so a broken config never stops the editor.

**Settings** (`<Leader>,` or `:settings`, as VS Code's settings page) lists every option except keys
(`mapleader`, `timeoutlen`, `keymaps` stay in the file), grouped by section, full screen and drawn in the
theme being set, so each change shows at once. `/` searches names, descriptions and sections (`<CR>` back to the list, `<Esc>` clears),
`j`/`k` move, `h`/`l` or `<CR>` change the value,
`w` writes the options into the config file (the other sections are kept; the old file is copied to
`config.json.bak`, since its comments are not kept), `<Esc>` closes and keeps the changes for this session.

```sh
mkdir -p ~/.config/lim && lim --print-config > ~/.config/lim/config.json
```

```jsonc
{
  "options": { "tabstop": 4, "relativenumber": false, "mapleader": ",", "colorscheme": "catppuccin-mocha" },
  "palette": { "ui": { "base": "#11121d" } },
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
| `signcolumn` · `foldcolumn` | `true` · `true` | Two-cell git sign column · one blank column left of the numbers |
| `cursorline` | `true` | Highlight the cursor's line |
| `tabstop` · `shiftwidth` · `expandtab` | `4` · `0` · `true` | Tab width · indent step (`0` = tabstop) · indent with spaces |
| `scrolloff` / `sidescrolloff` | `0` / `0` | Lines / columns kept around the cursor |
| `ignorecase` / `smartcase` · `hlsearch` | `true` / `true` · `true` | Search case rules · highlight matches |
| `autopairs` · `autoindent` | `true` · `true` | Close brackets and quotes · keep indentation |
| `mapleader` · `timeoutlen` | `" "` · `500` | Leader key · ms to wait for the rest of a mapping |
| `showtabline` / `laststatus` / `winbar` | `true` | Tabline · statusline · per-window path bar |
| `cmdheight` | `0` | `0`: notifications and an on-demand command line; `1`: permanent command line |
| `clipboard` | `"unnamedplus"` | `""` keeps yanks inside lim |
| `colorscheme` | unset | A theme name; unset uses the last one picked (nvchad at first) |
| `icons` · `iconset` | `true` · `"nerd"` | Glyphs at all · `"nerd"`, `"unicode"`, `"emoji"` or `"ascii"` |
| `statusline_style` | `"rounded"` | Statusline section ends: `"rounded"`, `"slanted"`, `"arrow"` or `"flat"` (Nerd Font icon set only) |
| `completion` | `true` | Completion menu while typing (`<C-Space>` still opens it when off) |
| `truecolor` | `"auto"` | `true`, `false` (256 colors) or `"auto"` |
| `highlighturl` `indentguides` `matchparen` `gitsigns` `wordhighlight` `colorhighlight` `todocomments` `notifications` `dashboard` `syntax` | `true` | Turn each feature off |

### Palette

The theme's palette (astrotheme layout), overridable color by color under `palette.syntax` and `palette.ui`:

`syntax`: `red orange yellow green cyan blue purple text comment mute` ·
`ui`: `red orange yellow green cyan blue purple accent tabline winbar tool base inactive_base statusline split float title border current_line selection highlight none_text text text_active text_inactive prompt`

### Highlight groups

Each takes `fg`, `bg` (`"#RRGGBB"`), `bold`, `italic`, `underline`, or `link` to another group.

| Kind | Groups |
|---|---|
| Editor | `Normal` `NormalNC` `CursorLine` `LineNr` `CursorLineNr` `Visual` `Search` `CurSearch` `MatchParen` `LspReferenceText` `HighlightURL` `IblIndent` `Directory` |
| Syntax | `@variable` `@comment` `@string` `@number` `@keyword` `@function` `@type` `@type.builtin` `@constant` `@function.macro` `@property` |
| Statusline | `StatusLine` `StatusLineSection` `StatusLineNormal` `StatusLineInsert` `StatusLineVisual` `StatusLineCommand` `StatusLineReplace` `StatusLineCwd` `StatusLinePosition` `StatusLineRecording` |
| Tabline, windows | `TabLine` `TabLineSel` `TabLineFill` `TabLineIndicator` `TabLineModified` `BufferPicker` `WinSeparator` `WinBar` `WinBarNC` |
| Messages, floats | `MsgArea` `ErrorMsg` `NormalFloat` `FloatBorder` `FloatTitle` `PmenuSel` `SnacksNotifierInfo` `SnacksNotifierError` |
| Completion | `BlinkCmpLabelMatch` `BlinkCmpKind` `BlinkCmpSource` (menu: `NormalFloat`, `FloatBorder`, `PmenuSel`) |
| Git, TODO | `GitSignsAdd` `GitSignsChange` `GitSignsDelete` · `TodoFgFIX` `TodoFgTODO` `TodoFgHACK` `TodoFgWARN` `TodoFgPERF` `TodoFgNOTE` `TodoFgTEST` |
| Plugins look-alikes | `WhichKey` `WhichKeyDesc` `WhichKeyGroup` `WhichKeySeparator` · `SnacksPickerMatch` `SnacksPickerDir` · `NeoTreeNormal` `NeoTreeRootName` `NeoTreeDirectoryIcon` `NeoTreeFileName` `NeoTreeIndentMarker` `NeoTreeFileNameOpened` `NeoTreeGitModified` `NeoTreeGitAdded` `NeoTreeGitUntracked` `NeoTreeGitDeleted` |
| lim | `LimCheatsheetKey` `LimCheatsheetDesc` (the start screen uses `Normal`) |

### Keymaps

`keymaps` holds one object per mode: `"n"`, `"i"`, `"v"`, `"nv"` (normal and visual). Each entry maps a
left-hand side to a key sequence (`":w<CR>"`), an object with a which-key description
(`{ "rhs": ":w<CR>", "desc": "Save" }`), or `false` to disable the key. Right-hand sides run without
remapping and can call built-in mappings (`"<Leader>ff"`). Your mappings appear in which-key, the keymaps
picker and the cheat sheet.

Key notation: plain characters, `<Leader>`, `<CR>`, `<Esc>`, `<BS>`, `<Tab>`, `<S-Tab>`, `<Space>`, `<C-x>`,
`<C-Up>`…, `<F1>`…`<F12>`, `<Up>` `<Down>` `<Left>` `<Right>`, `<Home>` `<End>`, `<Del>`, `<PageUp>`
`<PageDown>`, `<lt>`, `<Bar>`, `<Bslash>`.

## Plugins

A plugin is a folder in `plugins/` next to the config file (`~/.config/lim/plugins/<name>/plugin.json`,
`%APPDATA%\lim\plugins\…` on Windows). Its `plugin.json` holds the same sections as the config (`options`,
`palette`, `highlights`, `keymaps`), plus `commands` and the informative `name`, `version`, `description`.
Plugins load in folder-name order, then the config, so a later one wins and your config wins over all.
Problems are reported with the file they come from.

Each command is `:Name`, uppercase first as Vim user commands. Its program gets the lines on stdin: the whole
buffer, or a range (`:'<,'>Name`, `:%Name`, `:3,9Name`). Words typed after the name are added to `run`.
`output` is `"message"` (default: stdout is shown) or `"replace"` (stdout replaces the lines, as one undo
step). A program that fails (non-zero exit), prints nothing, or runs more than 10 seconds leaves the text as it
was and its first stderr line is shown. In `run`, `${file}` is the buffer's path and `${plugin}` the plugin's
folder, so a plugin can ship its own scripts. Commands appear in `:` completion and the commands picker
(`<Leader>fC`); bind them with a keymap. `commands` also works in the config itself.

```jsonc
// ~/.config/lim/plugins/format/plugin.json
{
  "name": "format",
  "description": "Prettier, sorting, word count",
  "commands": {
    "Prettier": { "run": ["prettier", "--stdin-filepath", "${file}"], "output": "replace", "desc": "Format with Prettier" },
    "Sort": { "run": ["sort"], "output": "replace", "desc": "Sort lines" },
    "Words": { "run": ["python3", "${plugin}/count.py"], "desc": "Count words" }
  },
  "keymaps": { "n": { "<Leader>lf": { "rhs": ":Prettier<CR>", "desc": "Format" } }, "v": { "<Leader>s": ":Sort<CR>" } }
}
```

On Windows, programs are found as `.exe`; run `.cmd`/`.bat` tools (npm ones like `prettier`) through
`["cmd", "/c", "prettier", …]`.

## How it is built

No crates: the terminal is driven through a handful of libc calls (`tcsetattr`, `ioctl`, `select`, `read`,
`signal`) or, on Windows, kernel32 console calls with VT input and output, so both send the same escape
sequences to one key decoder. The Windows clipboard is user32. Everything else is the standard library.

```
src/
├── main.rs        startup, event loop, panic recovery
├── error.rs       Vim-worded errors (E32, E37, E492…)
├── term/          raw mode, window size, resize events, key decoding (sys/unix.rs, sys/windows.rs)
├── buffer/        lines, undo history, motions, text objects, git signs, file I/O
├── editor/        modes, operators, registers, keymaps, the built-in binding table, windows, picker,
│                  file tree, which-key, cheat sheet, completion, sessions, prompts, clipboard, macros
├── syntax/        keyword highlighter with multi-line state, language tables, todo-comments
├── config/        JSON reader, key notation, options/palette/highlights/keymaps, plugin manifests
└── render/        cell grid, themes and palettes, icon sets, windows, bars, floats, picker, tree, start screen
```

- **Rendering** draws each frame into a grid of cells (floats are later writes), encodes it to ANSI and sends
  only the rows that changed; frames are skipped while keys are queued.
- **Built-in keys** live in one table (`editor/bindings.rs`): each prefixed key's action and description.
  The dispatcher, which-key, the cheat sheet and the keymaps picker all read it, so they cannot disagree.
- **Undo** stores the lines an edit replaced, not copies of the file, and keeps 1000 steps. The same
  line-range changes move marks and jump-list anchors, so they follow the text.
- **Completion** words are cached per buffer and re-read only when that buffer's text changed.
- **Highlighting** state at each line start is cached and invalidated from the first edited line.
- **Git signs** read the index once per open/save and diff in memory when the text changes.
- **Saving** writes a temp file next to the target, syncs it, keeps permissions, then renames it over the
  original; writing over another existing file needs `!`.
- **No crashes**: `unwrap`, `expect`, `panic!` are denied outside tests. A panic is caught by the event loop,
  which repairs the editor, copies unsaved buffers to the temp directory, says where, and goes on. `.` and
  macros stop after 100 000 replayed keys, so a self-calling macro cannot hang the editor.

## Limits

Not there, by design: LSP (`<Leader>l…`, `gd`, diagnostics), completion, debugger, treesitter (a keyword
highlighter stands in), plugins that script the editor itself (they only filter text through a process), an
embedded terminal (`<F7>` suspends to your shell), Vim tab pages,
line wrapping, mouse, folds, spell checking, persistent undo. `:s` patterns are literal (plus
`\<`/`\>`), not regexes. Visual-block columns count characters, so tabs and wide characters select unevenly.

## Development

```sh
cargo test                     # 69 tests, including a random-key fuzz over every mode and screen size
cargo clippy --all-targets     # pedantic, zero warnings; unwrap/expect/panic are denied
cargo fmt --check              # rustfmt.toml, 120 columns
```

Screenshots are real lim frames captured in a pseudo-terminal and rasterised.
