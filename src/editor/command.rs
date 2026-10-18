//! The `:` command line and `/` search prompt.

use std::path::{Path, PathBuf};
use std::str::Chars;

use super::cmdhistory::CmdHistory;
use super::lines::SortKey;
use super::motion::{Motion, Span};
use super::operator::Operator;
use super::picker::PickerKind;
use super::re::PackType;
use super::textops::TextOp;
use super::tools::WordCase;
use super::vim::LineRange;
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos, text};
use crate::error::Error;
use crate::term::Key;

/// Every ex command by its full name, for command-line completion and `<Leader>fC`.
pub const COMMANDS: &[(&str, &str)] = &[
    ("base32", "Base32-encode the lines (! decodes)"),
    ("base64", "Base64-encode the lines (! decodes)"),
    ("bdelete", "Delete buffer"),
    ("bnext", "Next buffer"),
    ("bprevious", "Previous buffer"),
    ("calc", "Evaluate the arithmetic of the lines"),
    ("case", "Change the case of the lines: snake camel pascal kebab constant title lower upper"),
    ("close", "Close window"),
    ("detect", "File type, architecture, entropy, packer of the bytes"),
    ("edit", "Edit a file"),
    ("enew", "New empty buffer"),
    ("help", "Cheatsheet"),
    ("hex", "Hex-encode the lines (! decodes)"),
    ("hexedit", "Toggle the hex view of the buffer"),
    ("inspect", "The bytes at the cursor as every number type"),
    ("nohlsearch", "Clear search highlight"),
    ("only", "Close other windows"),
    ("pack", "Write a number at the cursor: :pack u32le 0x1234"),
    ("qall", "Quit"),
    ("quit", "Close the window or quit"),
    ("rename", "Rename the file on disk and the buffer"),
    ("rot13", "ROT13 the lines"),
    ("settings", "Settings: every option, searchable (also :setup)"),
    ("sha256", "SHA-256 of the lines"),
    ("split", "Split window"),
    ("strings", "Printable strings of the bytes, with offsets"),
    ("uniq", "Drop repeated adjacent lines"),
    ("url", "Percent-encode each line (! decodes)"),
    ("vsplit", "Split window vertically"),
    ("wall", "Write all buffers"),
    ("wc", "Count lines, words and chars"),
    ("wq", "Write and quit"),
    ("wqall", "Write all and quit"),
    ("write", "Write the buffer"),
    ("xit", "Write if changed and quit"),
    ("xor", "XOR the bytes with a key: :xor 0x41 or :xor key"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Write {
        path: Option<PathBuf>,
        force: bool,
    },
    WriteAll,
    /// `:q`: closes the window, or quits when it is the last one.
    Quit {
        force: bool,
    },
    QuitAll {
        force: bool,
    },
    CloseWindow,
    Only,
    Split {
        vertical: bool,
        path: Option<PathBuf>,
    },
    WriteQuit {
        path: Option<PathBuf>,
        force: bool,
        only_if_dirty: bool,
    },
    WriteAllQuit,
    Edit(PathBuf),
    New,
    BufferNext,
    BufferPrev,
    BufferDelete {
        force: bool,
    },
    NoHighlight,
    Goto(usize),
    /// `:$`, `:'a`, `:.`: a single address other than a number.
    GotoAddress(String),
    /// `:[range]g/pattern/command`, `:g!` and `:v` (`invert`) for the lines that do not match.
    Global {
        range: String,
        pattern: String,
        command: String,
        invert: bool,
    },
    /// `:[range]norm[!] keys`; `remap` is false with `!`.
    Normal {
        range: String,
        keys: String,
        remap: bool,
    },
    /// `:[range]sort[!] [i] [n] [u]`
    Sort {
        range: String,
        key: SortKey,
        reverse: bool,
        unique: bool,
    },
    /// `:[range]uniq`, `base64[!]`, `sha256`, `wc`, `calc`: the whole buffer without a range.
    Text {
        range: String,
        op: TextOp,
    },
    /// `:[range]m {address}`, or `:t` / `:co` with `copy`.
    MoveLines {
        range: String,
        address: String,
        copy: bool,
    },
    /// `:[line]r file`
    Read {
        range: String,
        path: PathBuf,
    },
    /// `:e!`: the file again, dropping changes.
    Reload,
    /// `:sav[eas][!] file`: write to `file` and edit it from now on.
    SaveAs {
        path: PathBuf,
        force: bool,
    },
    /// `:hexedit`, `:detect`, `:inspect`, `:strings`: the reverse-engineering views of the buffer's bytes.
    HexEdit,
    Detect,
    Inspect,
    Strings,
    /// `:pack {type} {value}` at the cursor of a hex buffer.
    Pack {
        ty: PackType,
        value: String,
    },
    /// `:[range]xor {key}` in a hex buffer.
    Xor {
        range: String,
        key: String,
    },
    /// `:rename file`: the file on disk and the buffer, as `<Leader>R`.
    Rename(String),
    /// `:ls`: the buffer list (the buffers picker).
    ListBuffers,
    /// `:b N` / `:b name`
    Buffer(String),
    /// `:[range]s/pattern/replacement/[flags]`
    Substitute {
        range: String,
        pattern: String,
        replacement: String,
        flags: SubFlags,
    },
    /// `:[range]d`, `y`, `>`, `<`, `j`
    LineOp {
        range: String,
        op: LineOp,
    },
    Help,
    Setup,
    /// `:[range]Name args`: a config command, by Vim's rule that user commands start uppercase.
    User {
        range: String,
        name: String,
        args: Vec<String>,
    },
}

/// `:s` flags: `g` every match of a line, `n` count only, `c` confirm each, `e` no error when nothing matches,
/// `i`/`I` case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubFlags {
    pub global: bool,
    pub action: SubAction,
    pub quiet: bool,
    pub case: SubCase,
}

/// What `:s` does with the matches: replace them, only count them (`n`), or ask about each (`c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubAction {
    #[default]
    Replace,
    Count,
    Confirm,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubCase {
    /// `ignorecase` / `smartcase` decide.
    #[default]
    Options,
    Ignore,
    Match,
}

impl SubFlags {
    fn parse(flags: &str) -> Result<Self, Error> {
        let mut parsed: Self = Self::default();
        for c in flags.trim().chars() {
            match c {
                'g' => parsed.global = true,
                'n' => parsed.action = SubAction::Count,
                'c' => parsed.action = SubAction::Confirm,
                'e' => parsed.quiet = true,
                'i' => parsed.case = SubCase::Ignore,
                'I' => parsed.case = SubCase::Match,
                _ => return Err(Error::Vim(format!("E488: Trailing characters: {}", flags.trim()))),
            }
        }
        Ok(parsed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOp {
    Delete,
    Yank,
    Indent,
    Dedent,
    Join,
}

/// Splits a leading line range (`%`, `.`, `$`, `N`, `'x`, `a,b`) off an ex command.
fn split_range(input: &str) -> (&str, &str) {
    let bytes: &[u8] = input.as_bytes();
    if let Some(rest) = input.strip_prefix('%') {
        return ("%", rest);
    }
    let digits = |mut i: usize| -> usize {
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        i
    };
    let address = |i: usize| -> usize {
        let mut i: usize = match bytes.get(i) {
            Some(b'.' | b'$') => i + 1,
            Some(b'\'') if i + 1 < bytes.len() && bytes[i + 1].is_ascii() => i + 2,
            Some(b) if b.is_ascii_digit() => digits(i),
            _ => i,
        };
        // `+N` / `-N` offsets after it (or alone, from the cursor's line).
        while matches!(bytes.get(i), Some(b'+' | b'-')) {
            i = digits(i + 1);
        }
        i
    };
    let mut end: usize = address(0);
    if end > 0 && bytes.get(end) == Some(&b',') {
        let second: usize = address(end + 1);
        if second > end + 1 {
            end = second;
        }
    }
    (&input[..end], &input[end..])
}

/// `g/pattern/command`, `g!…` or `v/…` (then `invert`): the pattern up to the next unescaped delimiter, then
/// the command. The delimiter is any punctuation, so `:vs` and `:vnew` are left alone.
fn parse_global(rest: &str) -> Option<(bool, String, String)> {
    let (invert, body): (bool, &str) =
        [("global!", true), ("g!", true), ("vglobal", true), ("v", true), ("global", false), ("g", false)]
            .iter()
            .find_map(|(name, invert)| rest.strip_prefix(name).map(|body| (*invert, body)))?;
    let mut chars = body.chars();
    let delim: char = chars.next().filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && *c != '"')?;
    let mut pattern: String = String::new();
    let mut escaped: bool = false;
    for c in chars.by_ref() {
        match c {
            _ if escaped => {
                if c != delim {
                    pattern.push('\\');
                }
                pattern.push(c);
                escaped = false;
            }
            '\\' => escaped = true,
            c if c == delim => break,
            c => pattern.push(c),
        }
    }
    Some((invert, pattern, chars.collect()))
}

/// `s/pattern/replacement/flags` with any punctuation as the delimiter and `\` escaping it.
fn parse_substitute(rest: &str) -> Option<(String, String, String)> {
    let mut chars: Chars<'_> = rest.strip_prefix('s')?.chars();
    let delim: char = chars.next().filter(|c| !c.is_alphanumeric() && !c.is_whitespace())?;
    let mut parts: Vec<String> = vec![String::new()];
    let mut escaped: bool = false;
    for c in chars {
        match c {
            _ if escaped => {
                if c != delim {
                    parts.last_mut()?.push('\\');
                }
                parts.last_mut()?.push(c);
                escaped = false;
            }
            '\\' => escaped = true,
            c if c == delim && parts.len() < 3 => parts.push(String::new()),
            c => parts.last_mut()?.push(c),
        }
    }
    Some((parts.first()?.clone(), parts.get(1).cloned().unwrap_or_default(), parts.get(2).cloned().unwrap_or_default()))
}

impl Command {
    /// Ok(None) for an empty line.
    #[expect(clippy::too_many_lines, reason = "flat dispatch: one arm per command name")]
    pub fn parse(input: &str) -> Result<Option<Self>, Error> {
        let input: &str = input.trim();
        if input.is_empty() {
            return Ok(None);
        }
        if let Ok(line) = input.parse() {
            return Ok(Some(Self::Goto(line)));
        }
        let (range, rest): (&str, &str) = split_range(input);
        let rest: &str = rest.trim();
        if !range.is_empty() && rest.is_empty() {
            return Ok(Some(Self::GotoAddress(range.to_owned())));
        }
        if let Some((pattern, replacement, flags)) = parse_substitute(rest) {
            let flags: SubFlags = SubFlags::parse(&flags)?;
            return Ok(Some(Self::Substitute { range: range.to_owned(), pattern, replacement, flags }));
        }
        let line_op: Option<LineOp> = match rest {
            "d" | "de" | "del" | "delete" => Some(LineOp::Delete),
            "y" | "ya" | "yank" => Some(LineOp::Yank),
            ">" => Some(LineOp::Indent),
            "<" => Some(LineOp::Dedent),
            "j" | "join" => Some(LineOp::Join),
            _ => None,
        };
        if let Some(op) = line_op {
            return Ok(Some(Self::LineOp { range: range.to_owned(), op }));
        }
        if let Some((invert, pattern, command)) = parse_global(rest) {
            return Ok(Some(Self::Global { range: range.to_owned(), pattern, command, invert }));
        }
        let word: &str = rest.split(char::is_whitespace).next().unwrap_or_default();
        let arg: &str = rest[word.len()..].trim();
        let (bare, force): (&str, bool) = word.strip_suffix('!').map_or((word, false), |w| (w, true));
        match bare {
            "sor" | "sort" => {
                let key: SortKey = if arg.contains('n') {
                    SortKey::Numeric
                } else if arg.contains('i') {
                    SortKey::IgnoreCase
                } else {
                    SortKey::Text
                };
                let unique: bool = arg.contains('u');
                return Ok(Some(Self::Sort { range: range.to_owned(), key, reverse: force, unique }));
            }
            "uniq" | "base64" | "base32" | "hex" | "url" | "rot13" | "sha256" | "wc" | "calc" if arg.is_empty() => {
                let op: TextOp = match bare {
                    "uniq" => TextOp::Uniq,
                    "base64" => TextOp::Base64 { decode: force },
                    "base32" => TextOp::Base32 { decode: force },
                    "hex" => TextOp::Hex { decode: force },
                    "url" => TextOp::Url { decode: force },
                    "rot13" => TextOp::Rot13,
                    "sha256" => TextOp::Sha256,
                    "wc" => TextOp::Wc,
                    _ => TextOp::Calc,
                };
                return Ok(Some(Self::Text { range: range.to_owned(), op }));
            }
            "case" => {
                let case: WordCase = WordCase::parse(arg).ok_or_else(|| Error::NotACommand(input.into()))?;
                return Ok(Some(Self::Text { range: range.to_owned(), op: TextOp::Case(case) }));
            }
            "json" if arg.is_empty() => {
                return Ok(Some(Self::Text { range: range.to_owned(), op: TextOp::Json { minify: force } }));
            }
            "hexedit" | "detect" | "inspect" | "strings" if arg.is_empty() && range.is_empty() => {
                return Ok(Some(match bare {
                    "hexedit" => Self::HexEdit,
                    "detect" => Self::Detect,
                    "inspect" => Self::Inspect,
                    _ => Self::Strings,
                }));
            }
            "pack" if range.is_empty() => {
                let (ty, value): (&str, &str) = arg.split_once(char::is_whitespace).unwrap_or((arg, ""));
                let ty: PackType =
                    PackType::parse(ty).ok_or_else(|| Error::Vim(format!("E475: Invalid argument: {ty}")))?;
                return Ok(Some(Self::Pack { ty, value: value.trim().to_owned() }));
            }
            "xor" if !arg.is_empty() => return Ok(Some(Self::Xor { range: range.to_owned(), key: arg.to_owned() })),
            "m" | "mo" | "move" | "t" | "co" | "copy" if !arg.is_empty() => {
                let copy: bool = !bare.starts_with('m');
                return Ok(Some(Self::MoveLines { range: range.to_owned(), address: arg.to_owned(), copy }));
            }
            // `:r !cmd` would run a shell: lim's command line never does.
            "r" | "read" if !arg.is_empty() && !arg.starts_with('!') => {
                return Ok(Some(Self::Read { range: range.to_owned(), path: PathBuf::from(arg) }));
            }
            // The address may follow the name directly: `:m0`, `:t.`, `:m$`, `:co'a`.
            _ => {
                for (name, copy) in
                    [("move", false), ("mo", false), ("m", false), ("copy", true), ("co", true), ("t", true)]
                {
                    if let Some(address) = rest
                        .strip_prefix(name)
                        .filter(|a| a.starts_with(|c: char| c.is_ascii_digit() || ".$'+-".contains(c)))
                    {
                        let address: String = address.trim().to_owned();
                        return Ok(Some(Self::MoveLines { range: range.to_owned(), address, copy }));
                    }
                }
            }
        }
        if matches!(word, "norm" | "normal" | "norm!" | "normal!") {
            let keys: &str = rest[word.len()..].strip_prefix(' ').unwrap_or(&rest[word.len()..]);
            return Ok(Some(Self::Normal {
                range: range.to_owned(),
                keys: keys.to_owned(),
                remap: !word.ends_with('!'),
            }));
        }
        let (name, args): (&str, &str) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        if name.starts_with(|c: char| c.is_ascii_uppercase()) {
            let args: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
            return Ok(Some(Self::User { range: range.to_owned(), name: name.to_owned(), args }));
        }
        if !range.is_empty() {
            return Err(Error::NotACommand(input.into()));
        }
        let (name, arg): (&str, &str) =
            input.split_once(char::is_whitespace).map_or((input, ""), |(n, a)| (n, a.trim()));
        let (name, force): (&str, bool) = name.strip_suffix('!').map_or((name, false), |n| (n, true));
        let path: Option<PathBuf> = (!arg.is_empty()).then(|| PathBuf::from(arg));
        Ok(Some(match name {
            "w" | "write" => Self::Write { path, force },
            "wa" | "wall" => Self::WriteAll,
            "q" | "quit" => Self::Quit { force },
            "qa" | "qall" | "quitall" => Self::QuitAll { force },
            "clo" | "close" => Self::CloseWindow,
            "on" | "only" => Self::Only,
            "sp" | "split" | "new" => Self::Split { vertical: false, path },
            "vs" | "vsplit" | "vnew" => Self::Split { vertical: true, path },
            "wq" => Self::WriteQuit { path, force, only_if_dirty: false },
            "x" | "xit" | "exit" => Self::WriteQuit { path, force, only_if_dirty: true },
            "wqa" | "wqall" | "xa" | "xall" => Self::WriteAllQuit,
            "e" | "edit" if force && path.is_none() => Self::Reload,
            "e" | "edit" => Self::Edit(path.ok_or(Error::NoFileName)?),
            "sav" | "saveas" => Self::SaveAs { path: path.ok_or(Error::NoFileName)?, force },
            "rename" if !arg.is_empty() => Self::Rename(arg.to_owned()),
            "ls" | "buffers" | "files" => Self::ListBuffers,
            "b" | "bu" | "buf" | "buffer" => Self::Buffer(arg.to_owned()),
            b if b.len() > 1 && b.starts_with('b') && b[1..].bytes().all(|c| c.is_ascii_digit()) => {
                Self::Buffer(b[1..].to_owned())
            }
            "enew" => Self::New,
            "bn" | "bnext" => Self::BufferNext,
            "bp" | "bprevious" | "bN" | "bNext" => Self::BufferPrev,
            "bd" | "bdelete" => Self::BufferDelete { force },
            "noh" | "nohlsearch" => Self::NoHighlight,
            "h" | "help" => Self::Help,
            "settings" | "setup" => Self::Setup,
            _ => return Err(Error::NotACommand(input.into())),
        }))
    }
}

impl Editor {
    /// The history of the command line being typed: `:` lines, or `/` `?` patterns.
    pub(super) fn cmdline_history(&mut self) -> &mut CmdHistory {
        if self.mode == Mode::Command { &mut self.cmd_history } else { &mut self.search_history }
    }

    pub(super) fn command_line(&mut self, key: Key) {
        let commands: bool = self.mode == Mode::Command;
        match key {
            Key::Esc | Key::Ctrl('c') => self.mode = Mode::Normal,
            Key::Backspace | Key::Ctrl('h') if self.cmdline.is_empty() => self.mode = Mode::Normal,
            Key::Up | Key::Down | Key::ShiftUp | Key::ShiftDown => {
                let older: bool = matches!(key, Key::Up | Key::ShiftUp);
                let current: String = self.cmdline.clone();
                if let Some(recalled) = self.cmdline_history().step(older, &current) {
                    self.cmdline = recalled;
                    self.cmd_completion = None;
                }
            }
            Key::Backspace | Key::Ctrl('h') => {
                self.cmdline_history().reset();
                self.cmdline.pop();
                if commands {
                    self.update_cmd_completion();
                }
            }
            Key::Char(c) => {
                self.cmdline_history().reset();
                self.cmdline.push(c);
                if commands {
                    self.update_cmd_completion();
                }
            }
            Key::Tab | Key::Ctrl('n') if commands => {
                if self.cmd_completion.is_none() {
                    self.update_cmd_completion();
                }
                self.cycle_cmd_completion(true);
            }
            Key::BackTab | Key::Ctrl('p') if commands => self.cycle_cmd_completion(false),
            Key::Enter | Key::Ctrl('j') => {
                let input: String = std::mem::take(&mut self.cmdline);
                self.cmdline_history().push(&input);
                let search: bool = self.mode == Mode::Search;
                self.mode = Mode::Normal;
                if search {
                    if !input.is_empty() {
                        self.search = input;
                    }
                    self.run_motion(Motion::SearchNext { forward: true });
                } else {
                    match Command::parse(&input) {
                        Ok(Some(command)) => self.run(command),
                        Ok(None) => {}
                        Err(e) => self.error(&e),
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn run(&mut self, command: Command) {
        if let Err(e) = self.try_run(command) {
            self.error(&e);
        }
    }

    #[expect(clippy::too_many_lines, reason = "flat dispatch: one arm per command")]
    pub(super) fn try_run(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::Write { path, force } => self.write(path.as_deref(), force)?,
            Command::WriteAll => self.write_all()?,
            Command::Quit { force } => {
                if !self.close_window() {
                    self.try_quit(force)?;
                }
            }
            Command::QuitAll { force } => self.try_quit(force)?,
            Command::CloseWindow => {
                self.close_window();
            }
            Command::Only => self.only_window(),
            Command::Split { vertical, path } => {
                self.split(vertical);
                if let Some(path) = path {
                    self.open(&path);
                }
            }
            Command::WriteQuit { path, force, only_if_dirty } => {
                if !only_if_dirty || path.is_some() || self.buf().is_dirty() {
                    self.write(path.as_deref(), force)?;
                }
                if !self.close_window() {
                    self.try_quit(force)?;
                }
            }
            Command::WriteAllQuit => {
                self.write_all()?;
                self.try_quit(false)?;
            }
            Command::Edit(path) => self.open(&path),
            Command::New => self.new_buffer(),
            Command::BufferNext => self.cycle(true, 1),
            Command::BufferPrev => self.cycle(false, 1),
            Command::BufferDelete { force } => self.close_buffer(force)?,
            Command::NoHighlight => self.highlight_search = false,
            Command::GotoAddress(spec) => {
                let range: LineRange = self.parse_range(&spec).ok_or_else(|| Error::NotACommand(spec.clone()))?;
                self.push_jump();
                let buf: &mut Buffer = self.buf_mut();
                buf.cursor = Pos::new(range.last, buf.first_non_blank(range.last));
            }
            Command::Global { range, pattern, command, invert } => {
                let lines: LineRange = self.range_or_all(&range)?;
                let pattern: String = if pattern.is_empty() { self.search.clone() } else { pattern };
                if pattern.is_empty() {
                    return Err(Error::NoPattern);
                }
                self.global(lines, &pattern, invert, &command)?;
            }
            Command::Normal { range, keys, remap } => {
                let lines: Option<LineRange> = if range.is_empty() {
                    None
                } else {
                    Some(self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?)
                };
                self.normal_on_lines(lines, &keys, remap)?;
            }
            Command::Sort { range, key, reverse, unique } => {
                let lines: LineRange = self.range_or_all(&range)?;
                self.sort_lines(lines, key, reverse, unique);
            }
            Command::HexEdit => self.buf_mut().toggle_hex()?,
            Command::Detect => self.detect()?,
            Command::Inspect => self.inspect()?,
            Command::Strings => self.strings()?,
            Command::Pack { ty, value } => self.pack(ty, &value)?,
            Command::Xor { range, key } => {
                let lines: LineRange = self.range_or_all(&range)?;
                self.xor(lines, &key)?;
            }
            Command::Text { range, op } => {
                let lines: LineRange = self.range_or_all(&range)?;
                self.text_op(lines, op)?;
            }
            Command::MoveLines { range, address, copy } => {
                let lines: LineRange = self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?;
                self.move_lines(lines, &address, copy)?;
            }
            Command::Read { range, path } => {
                let below: usize = self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?.last;
                self.read_file(below, &path)?;
            }
            Command::Reload => {
                let summary: String = self.buf_mut().revert()?;
                self.info(summary);
            }
            Command::SaveAs { path, force } => {
                if path.exists() && !force {
                    return Err(Error::FileExists(path));
                }
                let buf: &mut Buffer = self.buf_mut();
                buf.set_path(path);
                let summary: String = buf.save(None, false)?;
                self.info(summary);
            }
            Command::Rename(name) => {
                let done: String = self.rename_current(&name).map_err(Error::Vim)?;
                self.info(done);
            }
            Command::ListBuffers => self.open_picker(PickerKind::Buffers),
            Command::Buffer(name) => self.switch_to_named(&name)?,
            Command::Substitute { range, pattern, replacement, flags } => {
                let lines: LineRange = self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?;
                let pattern: String = if pattern.is_empty() { self.search.clone() } else { pattern };
                if pattern.is_empty() {
                    return Err(Error::NoPattern);
                }
                let pattern: String = match flags.case {
                    SubCase::Options => pattern,
                    SubCase::Ignore => format!("\\c{pattern}"),
                    SubCase::Match => format!("\\C{pattern}"),
                };
                text::compile(&pattern).map_err(Error::Vim)?;
                let plural = |n: usize| if n == 1 { "" } else { "s" };
                match flags.action {
                    SubAction::Count => {
                        let (matches, on): (usize, usize) = self.count_matches(lines, &pattern, flags.global);
                        let es: &str = if matches == 1 { "" } else { "es" };
                        self.info(format!("{matches} match{es} on {on} line{}", plural(on)));
                    }
                    SubAction::Confirm => {
                        if !self.start_confirm(lines, &pattern, &replacement, flags.global) && !flags.quiet {
                            return Err(Error::PatternNotFound(pattern));
                        }
                    }
                    SubAction::Replace => {
                        self.buf_mut().seal();
                        match self.substitute(lines, &pattern, &replacement, flags.global) {
                            0 if flags.quiet => {}
                            0 => return Err(Error::PatternNotFound(pattern)),
                            n => self.info(format!("{n} line{} changed", plural(n))),
                        }
                    }
                }
            }
            Command::LineOp { range, op } => {
                let lines: LineRange = self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?;
                let (first, last): (Pos, Pos) = (Pos::new(lines.first, 0), Pos::new(lines.last, 0));
                self.buf_mut().seal();
                match op {
                    LineOp::Delete => self.apply(Operator::Delete, first, last, Span::Linewise),
                    LineOp::Yank => self.apply(Operator::Yank, first, last, Span::Linewise),
                    LineOp::Indent => self.apply(Operator::Indent, first, last, Span::Linewise),
                    LineOp::Dedent => self.apply(Operator::Dedent, first, last, Span::Linewise),
                    LineOp::Join => {
                        self.buf_mut().cursor = first;
                        self.join(lines.last - lines.first + 1, true);
                    }
                }
            }
            Command::Help => self.open_cheatsheet(),
            Command::Setup => self.open_setup(),
            Command::User { range, name, args } => self.run_user_command(&range, &name, &args, true)?,
            Command::Goto(n) => {
                let buf: &mut Buffer = self.buf_mut();
                let line: usize = n.saturating_sub(1).min(buf.last_line());
                buf.cursor = Pos::new(line, buf.first_non_blank(line));
            }
        }
        Ok(())
    }

    fn write(&mut self, path: Option<&Path>, force: bool) -> Result<(), Error> {
        let target: Option<PathBuf> = path.map(Path::to_path_buf).or_else(|| self.buf().path.clone());
        let format_error: Option<Error> = target.and_then(|t| self.run_on_save(&t));
        let gitsigns: bool = self.options.gitsigns;
        let buf: &mut Buffer = self.buf_mut();
        let summary: String = buf.save(path, force)?;
        if gitsigns {
            buf.load_git_base();
        }
        self.info(summary);
        // The file is written either way; a formatter that failed says so after the summary.
        if let Some(e) = format_error {
            self.error(&e);
        }
        Ok(())
    }

    fn write_all(&mut self) -> Result<(), Error> {
        for buf in self.buffers.iter_mut().filter(|b| b.is_dirty() && b.path.is_some()) {
            buf.save(None, false)?;
        }
        Ok(())
    }

    fn try_quit(&mut self, force: bool) -> Result<(), Error> {
        if !force && let Some(i) = self.buffers.iter().position(Buffer::is_dirty) {
            self.switch_to(i);
            return Err(Error::Unsaved(self.buffers[i].name()));
        }
        self.quit = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        assert_eq!(Command::parse("  ").unwrap(), None);
        assert_eq!(Command::parse("42").unwrap(), Some(Command::Goto(42)));
        assert_eq!(Command::parse("q!").unwrap(), Some(Command::Quit { force: true }));
        assert_eq!(
            Command::parse("w out.txt").unwrap(),
            Some(Command::Write { path: Some("out.txt".into()), force: false })
        );
        assert!(matches!(Command::parse("e"), Err(Error::NoFileName)));
        assert!(matches!(Command::parse("frob"), Err(Error::NotACommand(_))));
        // The command line never reaches a shell: no `:!`, `:cd`, `:sh` or `:terminal`.
        // Themes and icon sets are picked in `:settings`, not by command.
        assert!(matches!(Command::parse("colorscheme nord"), Err(Error::NotACommand(_))));
        for shell in ["!ls", "%!sort", "cd /tmp", "lcd x", "sh", "shell", "terminal", "r !ls"] {
            assert!(matches!(Command::parse(shell), Err(Error::NotACommand(_))), "{shell}");
        }
        assert_eq!(
            Command::parse("%s/a\\/b/c/g").unwrap(),
            Some(Command::Substitute {
                range: "%".into(),
                pattern: "a/b".into(),
                replacement: "c".into(),
                flags: SubFlags { global: true, ..SubFlags::default() }
            })
        );
        let Ok(Some(Command::Substitute { flags, .. })): Result<Option<Command>, Error> = Command::parse("s/a/b/gi")
        else {
            panic!("s/a/b/gi")
        };
        assert_eq!(flags, SubFlags { global: true, case: SubCase::Ignore, ..SubFlags::default() });
        assert!(matches!(Command::parse("s/a/b/x"), Err(Error::Vim(_))));
        assert_eq!(
            Command::parse("'<,'>d").unwrap(),
            Some(Command::LineOp { range: "'<,'>".into(), op: LineOp::Delete })
        );
        assert_eq!(Command::parse("$").unwrap(), Some(Command::GotoAddress("$".into())));
        assert_eq!(Command::parse("2,4>").unwrap(), Some(Command::LineOp { range: "2,4".into(), op: LineOp::Indent }));
        assert_eq!(
            Command::parse("'<,'>base64!").unwrap(),
            Some(Command::Text { range: "'<,'>".into(), op: TextOp::Base64 { decode: true } })
        );
        assert_eq!(Command::parse("uniq").unwrap(), Some(Command::Text { range: String::new(), op: TextOp::Uniq }));
        assert_eq!(Command::parse(".calc").unwrap(), Some(Command::Text { range: ".".into(), op: TextOp::Calc }));
        // The text commands take no argument, and `:b64` stays buffer 64.
        assert_eq!(
            Command::parse("%case k").unwrap(),
            Some(Command::Text { range: "%".into(), op: TextOp::Case(WordCase::Kebab) })
        );
        assert_eq!(
            Command::parse("json!").unwrap(),
            Some(Command::Text { range: String::new(), op: TextOp::Json { minify: true } })
        );
        for bad in ["uniq x", "wc -l", "sha256 file", "rename", "case", "case nope", "json x"] {
            assert!(matches!(Command::parse(bad), Err(Error::NotACommand(_))), "{bad}");
        }
        assert_eq!(Command::parse("b64").unwrap(), Some(Command::Buffer("64".into())));
        assert_eq!(
            Command::parse("'<,'>Sort -r  -u").unwrap(),
            Some(Command::User { range: "'<,'>".into(), name: "Sort".into(), args: vec!["-r".into(), "-u".into()] })
        );
    }
}
