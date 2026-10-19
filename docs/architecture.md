# How lim is built

No crates: the terminal is driven through a handful of libc calls (`tcsetattr`, `ioctl`, `select`, `read`,
`signal`) or, on Windows, kernel32 console calls with VT input and output, so both send the same escape
sequences to one key decoder. The Windows clipboard is user32. HTTPS, for the update, goes through the system:
WinHTTP on Windows, and on Linux and macOS libcurl, loaded with `dlopen` when needed so lim does not link
against it. Everything else is the standard library.

```
src/
├── main.rs        startup, event loop, panic recovery
├── update.rs      :upgrade / <Leader>pu: the latest GitHub release's binary, swapped in
├── error.rs       Vim-worded errors (E32, E37, E492…)
├── term/          raw mode, window size, resize events, key decoding, HTTPS (sys/unix.rs, sys/windows.rs)
├── buffer/        lines, undo history, motions, text objects, git signs, file I/O, hex dumps
├── editor/        modes, operators, registers, keymaps, the built-in binding table, windows, picker,
│                  file tree, which-key, cheat sheet, completion, sessions, prompts, clipboard, macros,
│                  text tools, reverse-engineering commands, config commands
│   └── tools/     the <Leader>x tools, one module each (calc, units, bases, time, color, json, case,
│                  generated text), and their popup
├── syntax/        keyword highlighter with multi-line state, language tables, todo-comments
├── config/        JSON reader, key notation, options/palette/highlights/keymaps/commands
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
- **Highlighting** state at each line start is cached and invalidated from the first edited line. A far jump
  finds its start state with a scan that only tracks comments and strings (skipping straight to the bytes that
  can open one) and leaves a checkpoint every 4096 lines, so it stays exact: the first `G` in a 3-million-line
  file takes about 80 ms, later jumps well under a millisecond. Only the visible part of a long line is colored.
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
