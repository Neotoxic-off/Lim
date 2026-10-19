# Vim editing

## Keys

Everything usual, including: `R` replace mode, `<C-a>`/`<C-x>`, `gu`/`gU`/`g~`, `gJ`, `gi`, `gI`, `gv`,
`gx`, `gf` (also `path:line`), `gn`, `ge`, `g_`, `H`/`M`/`L`, `zz`/`zt`/`zb`, `<C-e>`/`<C-y>`, `m{a-z}` with
`'`/`` ` ``, `''`, `<C-o>`, `<C-^>` (previous buffer), `"{register}`, `q{r}`/`@{r}`/`@@`, `?`, `;`/`,`, `&`. Visual
block: `I`, `A`, `$` (ragged right edge, then `A` appends to every line), `d`, `c`, `y`, `r`, `~`/`u`/`U`,
`o`/`O`, blockwise put; its columns are screen cells, so tabs and wide characters line up as drawn. Insert mode: `<C-r>{r}`, `<C-o>`, `<C-t>`/`<C-d>`, `<C-w>`, `<C-u>`.
`<S-Left>`/`<S-Right>` move a word and `<S-Up>`/`<S-Down>` a page, in normal, visual (to grow the selection fast),
insert and after an operator. On the `:` and `/` lines, `<Up>`/`<Down>` recall older / newer entries that start with
what is typed (separate histories for commands and searches).
Marks, `''` and `<C-o>` stay on their text when lines above are added or removed (and after undo);
a letter mark goes away with its line, as in Vim.

## Completion

blink.cmp keys as AstroNvim maps them: words from the open buffers appear while you type,
current buffer first. `<Tab>`/`<C-n>`/`<C-j>`/`<Down>` next, `<S-Tab>`/`<C-p>`/`<C-k>`/`<Up>` previous,
`<CR>` accepts the selected item (with nothing selected it is a plain newline), `<C-e>` hides,
`<C-Space>` shows the menu on demand.

The `:` line completes as you type: command names, then file paths for `:e`, `:w`, `:sp`,
`:vs`. `<Tab>`/`<C-n>` and `<S-Tab>`/`<C-p>` cycle through the menu, writing the choice into the line; `<CR>` runs it.

## Ex commands

`:w [file]` `:q` `:wq` `:x` `:wa` `:qa` `:e file` `:e!` `:enew` `:saveas file` `:ls` `:b N`/`:b name`
`:bn` `:bp` `:bd` `:sp` `:vs` `:only` `:close` `:noh` `:help` `:settings` (or `:setup`); on line ranges
`:s/pat/rep/[flags]`, `:g/pat/cmd` and `:v/pat/cmd`, `:norm[!] keys`, `:sort[!] [i] [n] [u]`, `:m {address}`,
`:t {address}`, `:r file`, `:d` `:y` `:>` `:<` `:j`. Addresses: `N`, `.`, `$`, `%`, `'a`, `'<,'>`, with `+N`/`-N`
offsets (`.,+3`, `$-1`). `:rename file` moves the file on disk and the buffer with it. Commands that start with an
uppercase letter come from the config's [commands](commands.md).

## Text tools

On the whole buffer or a range (`:'<,'>base64`), built in so they never start a process:

| Command | Does |
|---|---|
| `:uniq` | Drops repeated adjacent lines |
| `:base64` `:base32` `:hex` | Encode the lines (newlines included) on one line; with `!` decode them (`:hex!` also reads `0x41, 0x42` and `\x41`) |
| `:url` `:url!` | Percent-encode / decode each line |
| `:rot13` | ROT13 each line |
| `:sha256` | SHA-256 of the lines, as a file would hold them |
| `:wc` | Lines, words and chars |
| `:calc` | Arithmetic of the lines, with the [calculator](tools.md#calculator)'s syntax (`:.calc`) |
| `:case snake` | Each line in another case: `snake` `camel` `pascal` `kebab` `constant` `title` `lower` `upper` (a prefix is enough) |
| `:json` `:json!` | Lay the lines out as indented JSON / minify them; numbers and strings stay exactly as written |

Sorting, trailing blanks and reversing are Vim's own: `:sort`, `:%s/\s\+$//`, `:g/^/m0`.

## Patterns

Vim regexes (magic by default): `.` `*` `[a-z]` `[^…]` `^` `$` `\+` `\=` `\?` `\{n,m}`
`\(…\)` `\|` `\<` `\>` `\d` `\w` `\s` `\a` `\l` `\u` (and `\D` `\W` `\S`…), `\v` for very magic (`(a|b)+`), `\c`/`\C`
to ignore or match case. In `:s` replacements `&` or `\0` is the match and `\1`…`\9` its groups. `:s` flags: `g`
every match, `c` confirm each (`y` `n` `a` `q` `l`, one undo step for the session), `i`/`I` case, `n` count
only, `e` no error when nothing matches; `\r` in the replacement splits the line. `:g`
runs its command on marked lines, so deleting lines on the way is safe, and `u` undoes the whole `:g` or `:norm`.
Live grep (`<Leader>fw`) stays plain text.
