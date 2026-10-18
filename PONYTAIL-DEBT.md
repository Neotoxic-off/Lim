# Shortcut debt

Every `shortcut:` comment in the code: what was simplified, its limit, and when to revisit it.
Regenerate with `git grep -nE '(#|//|/[*]) ?(shortcut|ponytail):'`.

## src/buffer/mod.rs

- `src/buffer/mod.rs:104`, the whole file is read into memory. ceiling: text over 512 MB and binaries over 64 MB are refused. upgrade: page it in if lim must edit them (a piece table or rope behind `Buffer::lines`, which every module reads).

1 marker, 0 with no trigger.
