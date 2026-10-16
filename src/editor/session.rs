//! resession: the open files and their cursors, saved by name or per working directory.
//! Stored under `$XDG_STATE_HOME/lim/sessions` and `…/dirsessions`; "Last Session" is saved on exit.

use std::env;
use std::fs;
use std::path::PathBuf;

use super::{Editor, Message};
use crate::buffer::Pos;

pub const LAST: &str = "Last Session";

fn dir(per_directory: bool) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))?;
    Some(base.join("lim").join(if per_directory { "dirsessions" } else { "sessions" }))
}

/// A file name for any session name or directory path.
fn file_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '%' }).collect::<String>() + ".session"
}

fn cwd_name() -> String {
    env::current_dir().map(|d| d.display().to_string()).unwrap_or_default()
}

pub fn list(per_directory: bool) -> Vec<String> {
    let Some(dir) = dir(per_directory) else { return Vec::new() };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let text = fs::read_to_string(e.path()).ok()?;
            text.lines().next()?.strip_prefix("name\t").map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

impl Editor {
    fn session_text(&self, name: &str) -> String {
        let mut out = format!("name\t{name}\ncurrent\t{}\n", self.current);
        for buf in &self.buffers {
            if let Some(path) = &buf.path {
                let path = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                out.push_str(&format!("{}\t{}\t{}\n", buf.cursor.line, buf.cursor.col, path.display()));
            }
        }
        out
    }

    pub(super) fn save_session(&self, name: &str) -> Result<(), String> {
        self.write_session(name, false)
    }

    fn write_session(&self, name: &str, per_directory: bool) -> Result<(), String> {
        let Some(dir) = dir(per_directory) else { return Ok(()) };
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::write(dir.join(file_name(name)), self.session_text(name)).map_err(|e| e.to_string())
    }

    /// On exit (resession's autosave): "Last Session" and this directory's session.
    pub fn autosave_session(&self) {
        if self.buffers.iter().any(|b| b.path.is_some()) {
            let _ = self.write_session(LAST, false);
            let _ = self.write_session(&cwd_name(), true);
        }
    }

    pub(super) fn save_dirsession(&mut self) {
        match self.write_session(&cwd_name(), true) {
            Ok(()) => self.info("Directory session saved"),
            Err(e) => self.set_message(Message::Error(e)),
        }
    }

    pub(super) fn load_session(&mut self, name: &str, per_directory: bool) {
        let text = dir(per_directory).and_then(|d| fs::read_to_string(d.join(file_name(name))).ok());
        let Some(text) = text else { return self.info(format!("No session \"{name}\"")) };
        let mut current = 0;
        let mut opened = Vec::new();
        for line in text.lines() {
            if let Some(i) = line.strip_prefix("current\t") {
                current = i.parse().unwrap_or(0);
                continue;
            }
            let mut fields = line.splitn(3, '\t');
            let (Some(y), Some(x), Some(path)) = (fields.next(), fields.next(), fields.next()) else { continue };
            let path = PathBuf::from(path);
            if !path.is_file() {
                continue;
            }
            self.open(&path);
            let buf = self.buf_mut();
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
