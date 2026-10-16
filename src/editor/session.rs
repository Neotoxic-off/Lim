//! resession: the open files and their cursors, saved by name or per working directory.
//! Stored under `$XDG_STATE_HOME/lim/sessions` and `…/dirsessions`; "Last Session" is saved on exit.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::str::SplitN;

use super::{Editor, Message};
use crate::buffer::Buffer;
use crate::buffer::Pos;

pub const LAST: &str = "Last Session";

fn dir(per_directory: bool) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(crate::config::state_dir()?.join(if per_directory { "dirsessions" } else { "sessions" }))
}

/// A file name for any session name or directory path.
fn file_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '%' }).collect::<String>()
        + ".session"
}

fn cwd_name() -> String {
    env::current_dir().map(|d| d.display().to_string()).unwrap_or_default()
}

pub fn list(per_directory: bool) -> Vec<String> {
    let Some(dir): Option<PathBuf> = dir(per_directory) else { return Vec::new() };
    let Ok(entries): io::Result<fs::ReadDir> = fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let text: String = fs::read_to_string(e.path()).ok()?;
            text.lines().next()?.strip_prefix("name\t").map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

impl Editor {
    fn session_text(&self, name: &str) -> String {
        let files = self.buffers.iter().filter_map(|buf| {
            let path: &PathBuf = buf.path.as_ref()?;
            let path: PathBuf = crate::buffer::canonical(path);
            Some(format!("{}\t{}\t{}\n", buf.cursor.line, buf.cursor.col, path.display()))
        });
        // Restoring indexes the listed files, so unnamed buffers before the current one don't count.
        let current: usize = self.buffers[..self.current].iter().filter(|b| b.path.is_some()).count();
        format!("name\t{name}\ncurrent\t{current}\n") + &files.collect::<String>()
    }

    pub(super) fn save_session(&self, name: &str) -> Result<(), String> {
        self.write_session(name, false)
    }

    fn write_session(&self, name: &str, per_directory: bool) -> Result<(), String> {
        let Some(dir): Option<PathBuf> = dir(per_directory) else { return Ok(()) };
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::write(dir.join(file_name(name)), self.session_text(name)).map_err(|e| e.to_string())
    }

    /// On exit (resession's autosave): "Last Session" and this directory's session.
    pub fn autosave_session(&self) {
        if self.buffers.iter().any(|b| b.path.is_some()) {
            let _: Result<(), String> = self.write_session(LAST, false);
            let _: Result<(), String> = self.write_session(&cwd_name(), true);
        }
    }

    pub(super) fn save_dirsession(&mut self) {
        match self.write_session(&cwd_name(), true) {
            Ok(()) => self.info("Directory session saved"),
            Err(e) => self.set_message(Message::Error(e)),
        }
    }

    pub(super) fn load_session(&mut self, name: &str, per_directory: bool) {
        let text: Option<String> = dir(per_directory).and_then(|d| fs::read_to_string(d.join(file_name(name))).ok());
        match text {
            Some(text) => self.restore_session(name, &text),
            None => self.info(format!("No session \"{name}\"")),
        }
    }

    /// Opens the files `session_text` listed, with their cursors, and focuses the saved current one.
    fn restore_session(&mut self, name: &str, text: &str) {
        let mut current: usize = 0;
        let mut opened: Vec<usize> = Vec::new();
        for line in text.lines() {
            if let Some(i) = line.strip_prefix("current\t") {
                current = i.parse().unwrap_or(0);
                continue;
            }
            let mut fields: SplitN<'_, char> = line.splitn(3, '\t');
            let (Some(y), Some(x), Some(path)): (Option<&str>, Option<&str>, Option<&str>) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let path: PathBuf = PathBuf::from(path);
            if !path.is_file() {
                continue;
            }
            self.open(&path);
            let buf: &mut Buffer = self.buf_mut();
            buf.cursor = Pos::new(y.parse().unwrap_or(0), x.parse().unwrap_or(0));
            buf.clamp(false);
            opened.push(self.current);
        }
        if let Some(&i) = opened.get(current).or(opened.last()) {
            self.switch_to(i);
        }
        self.dashboard = false;
        self.info(format!("Loaded session \"{name}\" ({} files)", opened.len()));
    }

    pub(super) fn load_dirsession(&mut self) {
        self.load_session(&cwd_name(), true);
    }

    pub(super) fn delete_session(&mut self, name: &str, per_directory: bool) {
        if let Some(dir) = dir(per_directory) {
            match fs::remove_file(dir.join(file_name(name))) {
                Ok(()) => self.info(format!("Deleted session \"{name}\"")),
                Err(e) => self.set_message(Message::Error(format!("{name}: {e}"))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("my work-1"), "my work-1.session");
        assert_eq!(file_name("/home/me/src"), "%home%me%src.session");
    }

    #[test]
    fn text_round_trips() {
        let dir: PathBuf = env::temp_dir().join(format!("lim-session-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (a, b): (PathBuf, PathBuf) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "one\ntwo\nthree").unwrap();
        fs::write(&b, "x").unwrap();

        let mut ed: Editor = Editor::new(&[]);
        ed.buffers = vec![Buffer::from_text(None, "scratch"), Buffer::open(&a).unwrap(), Buffer::open(&b).unwrap()];
        ed.buffers[1].cursor = Pos::new(2, 3);
        ed.current = 1;
        let text: String = ed.session_text("work");
        assert!(text.starts_with("name\twork\ncurrent\t0\n"), "current counts listed files only");
        assert_eq!(text.lines().count(), 4, "the unnamed buffer is skipped");

        let mut restored: Editor = Editor::new(&[]);
        restored.restore_session("work", &format!("{text}0\t0\t{}\n", dir.join("gone.txt").display()));
        let names: Vec<String> = restored.buffers.iter().filter(|b| b.path.is_some()).map(Buffer::name).collect();
        assert_eq!(names, ["a.txt", "b.txt"]);
        let a_buf: &Buffer = restored.buffers.iter().find(|b| b.name() == "a.txt").unwrap();
        assert_eq!(a_buf.cursor, Pos::new(2, 3));
        assert_eq!(restored.buf().name(), "a.txt");
        assert!(!restored.dashboard);
        assert!(restored.message_text().contains("2 files"));
        fs::remove_dir_all(dir).unwrap();
    }
}
