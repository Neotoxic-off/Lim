//! The `:` command line and `/` search prompt.

use std::path::{Path, PathBuf};

use super::motion::Motion;
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos};
use crate::error::Error;
use crate::term::Key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Write { path: Option<PathBuf>, force: bool },
    WriteAll,
    /// `:q`: closes the window, or quits when it is the last one.
    Quit { force: bool },
    QuitAll { force: bool },
    CloseWindow,
    Only,
    Split { vertical: bool, path: Option<PathBuf> },
    WriteQuit { path: Option<PathBuf>, force: bool, only_if_dirty: bool },
    WriteAllQuit,
    Edit(PathBuf),
    New,
    BufferNext,
    BufferPrev,
    BufferDelete { force: bool },
    NoHighlight,
    Goto(usize),
}

impl Command {
    /// Ok(None) for an empty line.
    pub fn parse(input: &str) -> Result<Option<Self>, Error> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(None);
        }
        if let Ok(line) = input.parse() {
            return Ok(Some(Self::Goto(line)));
        }
        let (name, arg) = input.split_once(char::is_whitespace).map_or((input, ""), |(n, a)| (n, a.trim()));
        let (name, force) = name.strip_suffix('!').map_or((name, false), |n| (n, true));
        let path = (!arg.is_empty()).then(|| PathBuf::from(arg));
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
            _ => return Err(Error::NotACommand(input.into())),
        }))
    }
}

impl Editor {
    pub(super) fn command_line(&mut self, key: Key) {
        match key {
            Key::Esc | Key::Ctrl('c') => self.mode = Mode::Normal,
            Key::Backspace | Key::Ctrl('h') if self.cmdline.is_empty() => self.mode = Mode::Normal,
            Key::Backspace | Key::Ctrl('h') => {
                self.cmdline.pop();
            }
            Key::Char(c) => self.cmdline.push(c),
            Key::Enter | Key::Ctrl('j') => {
                let input = std::mem::take(&mut self.cmdline);
                let search = self.mode == Mode::Search;
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
                        Err(e) => self.error(e),
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn run(&mut self, command: Command) {
        if let Err(e) = self.try_run(command) {
            self.error(e);
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
            Command::BufferNext => self.cycle(1),
            Command::BufferPrev => self.cycle(-1),
            Command::BufferDelete { force } => self.close_buffer(force)?,
            Command::NoHighlight => self.highlight_search = false,
            Command::Goto(n) => {
                let buf = self.buf_mut();
                let line = n.saturating_sub(1).min(buf.last_line());
                buf.cursor = Pos::new(line, buf.first_non_blank(line));
            }
        }
        Ok(())
    }

    fn write(&mut self, path: Option<&Path>, force: bool) -> Result<(), Error> {
        let summary = self.buf_mut().save(path, force)?;
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
    }
}
