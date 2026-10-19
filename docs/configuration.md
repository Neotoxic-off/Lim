# Configuration

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

The documented defaults are also in [`config.example.json`](../config.example.json).

```jsonc
{
  "options": { "tabstop": 4, "relativenumber": true, "mapleader": ",", "colorscheme": "catppuccin-mocha" },
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

## Options

Names and defaults follow AstroNvim.

| Option | Default | Meaning |
|---|---|---|
| `number` / `relativenumber` | `true` / `false` | Line numbers; both on shows the current line's absolute number |
| `signcolumn` · `foldcolumn` | `true` · `true` | Two-cell git sign column · one blank column left of the numbers |
| `cursorline` | `false` | Highlight the cursor's line; off, only its number stands out (NvChad) |
| `tabstop` · `shiftwidth` · `expandtab` | `4` · `0` · `true` | Tab width · indent step (`0` = tabstop) · indent with spaces |
| `scrolloff` / `sidescrolloff` | `0` / `0` | Lines / columns kept around the cursor |
| `ignorecase` / `smartcase` · `hlsearch` | `true` / `true` · `true` | Search case rules · highlight matches |
| `autopairs` · `autoindent` | `true` · `true` | Close brackets and quotes · keep indentation |
| `mapleader` · `timeoutlen` | `" "` · `500` | Leader key · ms to wait for the rest of a mapping |
| `showtabline` / `laststatus` / `winbar` | `true` / `true` / `false` | Tabline · statusline · per-window path bar |
| `cmdheight` | `0` | `0`: notifications and an on-demand command line; `1`: permanent command line |
| `clipboard` | `"unnamedplus"` | `""` keeps yanks inside lim |
| `colorscheme` | unset | A theme name; unset uses the last one picked (nvchad at first) |
| `icons` · `iconset` | `true` · `"nerd"` | Glyphs at all · `"nerd"`, `"unicode"`, `"emoji"` or `"ascii"` |
| `statusline_style` | `"rounded"` | Statusline section ends: `"rounded"`, `"slanted"`, `"arrow"` or `"flat"` (Nerd Font icon set only) |
| `splash` | `"random"` | Start screen hydra: `"random"` (one that fits, each start), `"braille"` or `"small"` |
| `mouse` | `true` | Click (text, tabs, tabline buttons), drag to select, wheel; `false` gives the terminal its own selection back (to copy) |
| `completion` | `true` | Completion menu while typing (`<C-Space>` still opens it when off) |
| `truecolor` | `"auto"` | `true`, `false` (256 colors) or `"auto"` |
| `highlighturl` `indentguides` `matchparen` `gitsigns` `wordhighlight` `colorhighlight` `todocomments` `notifications` `dashboard` `syntax` | `true` | Turn each feature off |

## Palette

The theme's palette (astrotheme layout), overridable color by color under `palette.syntax` and `palette.ui`:

`syntax`: `red orange yellow green cyan blue purple text comment mute` ·
`ui`: `red orange yellow green cyan blue purple accent tabline winbar tool base inactive_base statusline split float title border current_line selection highlight none_text text text_active text_inactive prompt`

## Highlight groups

Each takes `fg`, `bg` (`"#RRGGBB"`), `bold`, `italic`, `underline`, or `link` to another group.

| Kind | Groups |
|---|---|
| Editor | `Normal` `NormalNC` `CursorLine` `LineNr` `CursorLineNr` `Visual` `Search` `CurSearch` `MatchParen` `LspReferenceText` `HighlightURL` `IblIndent` `Directory` |
| Syntax | `@variable` `@comment` `@string` `@number` `@keyword` `@function` `@type` `@type.builtin` `@constant` `@function.macro` `@property` |
| Statusline | `StatusLine` `StatusLineSection` `StatusLineNormal` `StatusLineInsert` `StatusLineVisual` `StatusLineCommand` `StatusLineReplace` `StatusLineCwd` `StatusLinePosition` `StatusLineRecording` `StatusLineModeSep` (the grey step after the mode) `StatusLineGit` `StatusLineJob` (the spinner) |
| Tabline, windows | `TabLine` `TabLineSel` `TabLineFill` `TabLineModified` `TabLineButton` (light/dark button) `TabLineCloseAll` `BufferPicker` `WinSeparator` `WinBar` `WinBarNC` |
| Messages, floats | `MsgArea` `ErrorMsg` `NormalFloat` `FloatBorder` (fg = bg hides the frame, the default; set an fg to bring lines back) `FloatTitle` (title pill) `PmenuSel` `SnacksNotifierInfo` `SnacksNotifierError` |
| Completion | `BlinkCmpLabelMatch` `BlinkCmpKind` `BlinkCmpSource` (menu: `NormalFloat`, `FloatBorder`, `PmenuSel`) |
| Git, TODO | `GitSignsAdd` `GitSignsChange` `GitSignsDelete` · `TodoFgFIX` `TodoFgTODO` `TodoFgHACK` `TodoFgWARN` `TodoFgPERF` `TodoFgNOTE` `TodoFgTEST` |
| Plugins look-alikes | `WhichKey` `WhichKeyNormal` (menu background, the editor's by default) `WhichKeyDesc` `WhichKeyGroup` `WhichKeySeparator` · `SnacksPickerMatch` `SnacksPickerDir` `SnacksPickerInput` · `NeoTreeNormal` `NeoTreeDirectoryIcon` `NeoTreeFileName` `NeoTreeIndentMarker` `NeoTreeFileNameOpened` `NeoTreeGitModified` `NeoTreeGitAdded` `NeoTreeGitUntracked` `NeoTreeGitDeleted` |
| lim | `LimCheatsheetKey` `LimCheatsheetDesc` (the start screen uses `Normal` for the hydra, these two for its buttons) |

## Keymaps

`keymaps` holds one object per mode: `"n"`, `"i"`, `"v"`, `"nv"` (normal and visual). Each entry maps a
left-hand side to a key sequence (`":w<CR>"`), an object with a which-key description
(`{ "rhs": ":w<CR>", "desc": "Save" }`), or `false` to disable the key. Right-hand sides run without
remapping and can call built-in mappings (`"<Leader>ff"`). Your mappings appear in which-key, the keymaps
picker and the cheat sheet.

Key notation: plain characters, `<Leader>`, `<CR>`, `<Esc>`, `<BS>`, `<Tab>`, `<S-Tab>`, `<Space>`, `<C-x>`,
`<C-Up>`…, `<F1>`…`<F12>`, `<Up>` `<Down>` `<Left>` `<Right>`, `<Home>` `<End>`, `<Del>`, `<PageUp>`
`<PageDown>`, `<lt>`, `<Bar>`, `<Bslash>`.

## Commands

The `commands` section adds `:Name` commands that pipe text through external programs: see [Commands](commands.md).
