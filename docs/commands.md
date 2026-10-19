# Commands

The config's `commands` section adds `:Name` commands, uppercase first as Vim user commands. Its program gets the
lines on stdin: the whole buffer, or a range (`:'<,'>Name`, `:%Name`, `:3,9Name`). Words typed after the name are
added to `run`. `output` is `"message"` (default: stdout is shown) or `"replace"` (stdout replaces the lines, as
one undo step). A command typed by hand runs in the background: editing goes on, and its output lands when it
arrives (a `"replace"` is dropped, with a message, if the text changed meanwhile). A program that fails (non-zero
exit), prints nothing, or runs too long (10 minutes; 10 seconds for format on save, which runs before the write)
leaves the text as it was and its first stderr line is shown. In `run`, `${file}` is the buffer's path and `${line}` the
cursor's line. An optional `windows` array replaces `run` on Windows. Commands appear in `:` completion and the
commands picker (`<Leader>fC`); bind them with a [keymap](configuration.md#keymaps).

## Format on save

`"on": "save"` runs a `"replace"` command on the whole buffer before every `:w`, limited to
the extensions in `"files"` (`["js", "ts"]`; none means every file). A tool that fails or is missing is reported
after the write, and the file is written unchanged.

```jsonc
// ~/.config/lim/config.json
{
  "commands": {
    "Prettier": {
      "run": ["prettier", "--stdin-filepath", "${file}"],
      "windows": ["cmd", "/c", "prettier", "--stdin-filepath", "${file}"],
      "output": "replace",
      "on": "save",
      "files": ["js", "ts", "css", "json", "md"],
      "desc": "Format with Prettier"
    },
    "Rustfmt": { "run": ["rustfmt", "--edition", "2024", "--emit", "stdout", "--quiet"], "output": "replace", "on": "save", "files": ["rs"] },
    "Blame": { "run": ["git", "blame", "-L", "${line},${line}", "--", "${file}"], "desc": "Who changed this line" }
  },
  "keymaps": { "n": { "<Leader>lf": { "rhs": ":Prettier<CR>", "desc": "Format" } } }
}
```

On Windows, programs are found as `.exe`; run `.cmd`/`.bat` tools (npm ones like `prettier`) through
`["cmd", "/c", "prettier", …]`.
