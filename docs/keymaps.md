# Keymaps

The leader is `<Space>`. Unless marked *lim*, everything matches AstroNvim's defaults, plus the
`<Tab>`/`<S-Tab>` buffer navigation of the reference config. Vim's own keys and Ex commands are in
[Vim editing](vim.md).

Press `<F1>` (or `<Leader>fh`, `:help`) in lim for the cheat sheet, or `<Space>` and wait for which-key.

## General

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

## Buffers

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

## Windows

| Keys | Action |
|---|---|
| `\|` · `\` | Vertical · horizontal split |
| `<C-h>` `<C-j>` `<C-k>` `<C-l>` | Move between windows (and into the file tree) |
| `<C-Up>` `<C-Down>` `<C-Left>` `<C-Right>` | Resize the split |
| `<C-w>v` `s` `c` `q` `o` `w` `h` `j` `k` `l`, `<C-w><arrows>` | Vim window commands |

## Find

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

## Git

| Keys | Action |
|---|---|
| `]g` `[g` · `]G` `[G` | Next / previous hunk · last / first hunk |
| `<Leader>gg` | Graph of every branch (`git log --graph --all`); `<CR>` on a commit shows it |
| `<Leader>gc` · `gC` · `gt` · `gT` | Log · log of this file · status · stash |
| `<Leader>go` | Open the file and line on the git host |

## File tree

`<Leader>e` toggles it, `<Leader>o` moves the focus in and out. Inside, as in neo-tree:

| Keys | Action |
|---|---|
| `j` `k` · `<CR>` `l` `o` · `h` | Move · open file or fold directory · close directory / go to parent |
| `a` · `A` · `r` · `d` | Add file (`name/` for a directory) · add directory · rename · delete (asks) |
| `c` · `m` · `y` · `Y` | Copy · move · copy name · copy path |
| `s` · `S` · `P` · `O` | Open in vertical / horizontal split · preview · open with the system |
| `.` · `<BS>` · `H` · `R` | Make root · root up · show hidden · refresh |
| `q` · `<C-l>` `<Esc>` · `?` | Close · back to the file · cheat sheet |

## Terminal and tools

`<F7>`, `<Leader>tf`, `<Leader>th`, `<Leader>tv` suspend lim and open your `$SHELL` (`%COMSPEC%` on Windows; `exit` comes back);
`<Leader>tn` node, `<Leader>tp` python, `<Leader>tt` btm, `<Leader>tu` gdu.
Files changed meanwhile are reloaded. *lim*: there is no embedded terminal.

## Sessions

`<Leader>Sl` last session · `Ss` save (named) · `SS` save for this directory · `S.` load this directory's ·
`Sf` / `SF` pick a session / directory session · `Sd` / `SD` delete one. The last session is saved on exit.

## Packages

`<Leader>pu` (or `:upgrade`) updates lim in the background: when GitHub has a newer release, cargo rebuilds it
while you keep editing, then a notification says to restart lim. *lim*: there are no plugins to update.

## UI toggles

| Keys | Action |
|---|---|
| `<Leader>ua` · `up` · `uc`/`uC` | Autopairs · paste mode · autocompletion |
| `<Leader>ub` | Light / dark variant of the theme |
| `<Leader>ug` · `u>` · `ul` · `ut` | Sign column · fold column · statusline · tabline |
| `<Leader>un` · `ui` | Cycle line numbering · indent width (prompt) |
| `<Leader>ur` · `uu` · `uz` · `uy` · `u\|` | References · URLs · colors · syntax · indent guides |
| `<Leader>uN` · `uD` · `uZ` | Notifications · dismiss them · zen mode |
| `<Leader>um` | Markdown preview beside Markdown buffers (*lim*) |
| `<Leader>x…` | [Tools](tools.md): `xc` calculator · `xu` units · `xb` bases · `xt` time · `xh` color · `xk` case · `xd`/`xD`/`xi`/`xp`/`xl` insert date, UUID, password, lorem · `xj`/`xJ` JSON · `xw`/`xs` count, hash (*lim*) |

## Your own keymaps

See [Configuration › Keymaps](configuration.md#keymaps).
