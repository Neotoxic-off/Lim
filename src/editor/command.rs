//! The `:` command line and `/` search prompt.

use std::path::{Path, PathBuf};
use std::str::Chars;

use super::cmdhistory::CmdHistory;
use super::motion::{Motion, Span};
use super::operator::Operator;
use super::vim::LineRange;
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos};
use crate::error::Error;
use crate::term::Key;

/// Every ex command by its full name, for command-line completion and `<Leader>fC`.
pub const COMMANDS: &[(&str, &str)] = &[
    ("bdelete", "Delete buffer"),
    ("bnext", "Next buffer"),
    ("bprevious", "Previous buffer"),
    ("close", "Close window"),
    ("edit", "Edit a file"),
    ("enew", "New empty buffer"),
    ("help", "Cheatsheet"),
    ("nohlsearch", "Clear search highlight"),
    ("only", "Close other windows"),
    ("qall", "Quit"),
    ("quit", "Close the window or quit"),
    ("settings", "Settings: every option, searchable (also :setup)"),
    ("split", "Split window"),
    ("vsplit", "Split window vertically"),
    ("wall", "Write all buffers"),
    ("wq", "Write and quit"),
    ("wqall", "Write all and quit"),
    ("write", "Write the buffer"),
    ("xit", "Write if changed and quit"),
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
    /// `:[range]s/pattern/replacement/[g]`
    Substitute {
        range: String,
        pattern: String,
        replacement: String,
        global: bool,
    },
    /// `:[range]d`, `y`, `>`, `<`, `j`
    LineOp {
        range: String,
        op: LineOp,
    },
    Help,
    Setup,
    /// `:[range]Name args`: a plugin or config command, by Vim's rule that user commands start uppercase.
    User {
        range: String,
        name: String,
        args: Vec<String>,
    },
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
    let address = |mut i: usize| -> usize {
        match bytes.get(i) {
            Some(b'.' | b'$') => i + 1,
            Some(b'\'') if i + 1 < bytes.len() && bytes[i + 1].is_ascii() => i + 2,
            Some(b) if b.is_ascii_digit() => {
                while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
                i
            }
            _ => i,
        }
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

/// `s/pattern/replacement/flags` with any punctuation as the delimiter and `\` escaping it.
fn parse_substitute(rest: &str) -> Option<(String, String, bool)> {
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
    let flags: String = parts.get(2).cloned().unwrap_or_default();
    Some((parts.first()?.clone(), parts.get(1).cloned().unwrap_or_default(), flags.contains('g')))
}

impl Command {
    /// Ok(None) for an empty line.
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
        if let Some((pattern, replacement, global)) = parse_substitute(rest) {
            return Ok(Some(Self::Substitute { range: range.to_owned(), pattern, replacement, global }));
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
            "e" | "edit" => Self::Edit(path.ok_or(Error::NoFileName)?),
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

    fn try_run(&mut self, command: Command) -> Result<(), Error> {
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
            Command::Substitute { range, pattern, replacement, global } => {
                let lines: LineRange = self.parse_range(&range).ok_or_else(|| Error::NotACommand(range.clone()))?;
                let pattern: String = if pattern.is_empty() { self.search.clone() } else { pattern };
                if pattern.is_empty() {
                    return Err(Error::NoPattern);
                }
                self.buf_mut().seal();
                match self.substitute(lines, &pattern, &replacement, global) {
                    0 => return Err(Error::PatternNotFound(pattern)),
                    n => self.info(format!("{n} line{} changed", if n == 1 { "" } else { "s" })),
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
            Command::User { range, name, args } => self.run_user_command(&range, &name, &args)?,
            Command::Goto(n) => {
                let buf: &mut Buffer = self.buf_mut();
                let line: usize = n.saturating_sub(1).min(buf.last_line());
                buf.cursor = Pos::new(line, buf.first_non_blank(line));
            }
        }
        Ok(())
    }

    fn write(&mut self, path: Option<&Path>, force: bool) -> Result<(), Error> {
        let gitsigns: bool = self.options.gitsigns;
        let buf: &mut Buffer = self.buf_mut();
        let summary: String = buf.save(path, force)?;
        if gitsigns {
            buf.load_git_base();
        }
        self.info(summary);
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
                global: true
            })
        );
        assert_eq!(
            Command::parse("'<,'>d").unwrap(),
            Some(Command::LineOp { range: "'<,'>".into(), op: LineOp::Delete })
        );
        assert_eq!(Command::parse("$").unwrap(), Some(Command::GotoAddress("$".into())));
        assert_eq!(Command::parse("2,4>").unwrap(), Some(Command::LineOp { range: "2,4".into(), op: LineOp::Indent }));
        assert_eq!(
            Command::parse("'<,'>Sort -r  -u").unwrap(),
            Some(Command::User { range: "'<,'>".into(), name: "Sort".into(), args: vec!["-r".into(), "-u".into()] })
        );
    }
}
