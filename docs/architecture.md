# How lim is built

The terminal is driven through a handful of libc calls (`tcsetattr`, `ioctl`, `select`, `read`, `signal`) or,
on Windows, kernel32 console calls with VT input and output, so both send the same escape sequences to one key
decoder.

Crates do what would otherwise mean running programs, binding system libraries or writing a parser by hand:
gix reads git repositories (`src/repo.rs`: signs, statuses, the file list, the git pickers, git browse) and
imara-diff (through gix) diffs for gitsigns; syntect with bat's grammars (two-face) highlights, pulldown-cmark
reads Markdown for the preview, jsonc-parser reads the config; ureq with rustls fetches the update over HTTPS;
arboard is the system clipboard and open hands URLs and files to the default app; data-encoding,
percent-encoding, sha2, getrandom and jiff do the encodings, hashes, random bytes and time zones.

No git process runs, and nothing a repository's own `.git/config` names runs either (gix takes filter and diff
drivers only from the user's and the system's config), so opening a folder from anywhere is safe. Programs still
run where that is the feature: config `commands`, the shell and the `<Leader>t…` tools.

```
src/
├── main.rs        startup, event loop, panic recovery
├── update.rs      :upgrade / <Leader>pu: the latest GitHub release's binary, swapped in
├── error.rs       Vim-worded errors (E32, E37, E492…)
├── repo.rs        git through gix: index text, status, file list, log/graph, show, stashes, browse
├── term/          raw mode, window size, resize events, key decoding, HTTPS (sys/unix.rs, sys/windows.rs)
├── buffer/        lines, undo history, motions, text objects, git signs, file I/O, hex dumps
├── editor/        modes, operators, registers, keymaps, the built-in binding table, windows, picker,
│                  file tree, which-key, cheat sheet, completion, sessions, prompts, clipboard, macros,
│                  text tools, reverse-engineering commands, config commands
│   └── tools/     the <Leader>x tools, one module each (calc, units, bases, time, color, json, case,
│                  generated text), and their popup
├── syntax/        syntect highlighting with per-line state, language facts, todo-comments
├── config/        JSON (jsonc-parser), key notation, options/palette/highlights/keymaps/commands
└── render/        cell grid, themes and palettes, icon sets, windows, bars, floats, picker, tree, start screen
```

- **Rendering** draws each frame into a grid of cells, back to front (`render/mod.rs`, `compose`): the tabline,
  the side panels, the windows, the bars, then the floats over them. It encodes the grid to ANSI and sends only
  the rows that changed; frames are skipped while keys are queued. A line of text is drawn in layers
  (`render/view.rs`, `LinePaint`): its syntax color, then URL, word, TODO, hex color, bracket and selection or
  search highlights over it.
- **Built-in keys** live in one table (`editor/bindings.rs`): each prefixed key's action and description.
  The dispatcher, which-key, the cheat sheet and the keymaps picker all read it, so they cannot disagree.
- **Undo** stores the lines an edit replaced, not copies of the file, and keeps 1000 steps. The same
  line-range changes move marks and jump-list anchors, so they follow the text.
- **Completion** words are cached per buffer and re-read only when that buffer's text changed.
- **Highlighting** is syntect with bat's grammars (two-face). The parse state at each line start and the colors
  of each drawn line are cached, and invalidated from the first edited line, so a line is parsed once. A far jump
  parses from 300 lines above it (Vim's `syntax sync minlines`), so a comment opened further up is missed until
  scrolled through. Lines over 20,000 bytes are drawn plain. The grammars load on a background thread at start.
- **Live grep** reads the project once per picker and indexes its trigrams: each key greps only the files that
  hold every trigram of the query.
- **Hex buffers** are ordinary lines in a dump format; only the hex digits count when writing, so every Vim edit
  works on bytes and a broken line is refused before anything touches the disk.
- **Git signs** read the index once per open/save and diff in memory when the text changes (Myers in linear
  space, after dropping the lines only one side has, as xdiff does).
- **Regexes** compile to a small program run by a backtracking machine with its own stack: a group can repeat
  any number of times without deepening the call stack, and a step budget stops runaway patterns.
- **Saving** writes a temp file next to the target, syncs it, keeps permissions, then renames it over the
  original; writing over another existing file needs `!`.
- **No crashes**: `unwrap`, `expect`, `panic!` are denied outside tests. A panic is caught by the event loop,
  which repairs the editor, copies unsaved buffers to the temp directory, says where, and goes on. `.` and
  macros stop after 100 000 replayed keys, so a self-calling macro cannot hang the editor.
- **Files are read whole** into memory, as in Vim: text over 512 MB and binaries over 64 MB are refused with a
  message. Lifting that would mean a piece table or rope behind `Buffer::lines`, which every module reads.
