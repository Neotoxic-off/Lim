# Shortcut debt

Every `shortcut:` comment in the code: what was simplified, its limit, and when to revisit it.
Regenerate with `git grep -nE '(#|//|/[*]) ?(shortcut|ponytail):'`.

## src/buffer/git.rs

- `src/buffer/git.rs:23`, git-sign diff runs LCS on the differing middle only. ceiling: signs are approximate on huge rewrites. upgrade: switch to Myers if huge rewrites need exact signs.

## src/buffer/text.rs

- `src/buffer/text.rs:49`, char width from a compact range table (wcwidth + wide emoji). ceiling: not the full EastAsianWidth data. upgrade: generate it from EastAsianWidth.txt if CJK or symbol users report misaligned columns.
- `src/buffer/text.rs:155`, case-insensitive matching by lowercasing. ceiling: lowercasing that changes char counts (e.g. `İ`) shifts columns. upgrade: fold char by char if search is used on Turkish or other such text.

## src/editor/block.rs

- `src/editor/block.rs:3`, visual-block columns counted in chars, not screen cells. ceiling: tabs and wide chars select unevenly. upgrade: switch to display columns if block edits on tab-indented or CJK files come up.

## src/editor/picker.rs

- `src/editor/picker.rs:260`, files scanned on each keystroke. ceiling: fine for project-sized trees. upgrade: use an index for huge monorepos.

## src/editor/vim.rs

- `src/editor/vim.rs:196`, `:s` patterns are literal, not Vim regexes. ceiling: no regex. upgrade: add a small regex engine if users need them.

## src/editor/plugin.rs

- `src/editor/plugin.rs:15`, plugin commands run synchronously with a 10 s timeout. ceiling: the editor freezes while the program runs. upgrade: run it in the background if plugins with long jobs show up.

## src/render/view.rs

- `src/render/view.rs:236`, the whole line is highlighted even when only part is visible. ceiling: slow on huge lines. upgrade: slice to the view for huge lines.

8 markers, 0 with no trigger.
