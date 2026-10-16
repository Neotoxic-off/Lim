//! Recently opened files (`<Leader>fo`, dashboard "Recents"), kept across sessions in
//! `$XDG_STATE_HOME/lim/recent` (default `~/.local/state/lim/recent`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::Editor;

const MAX_RECENT: usize = 100;

fn state_file() -> Option<PathBuf> {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))?;
    Some(base.join("lim").join("recent"))
}

pub fn load() -> Vec<PathBuf> {
    if cfg!(test) {
        return Vec::new();
    }
    let Some(file) = state_file() else { return Vec::new() };
    fs::read_to_string(file).map(|s| s.lines().map(PathBuf::from).collect()).unwrap_or_default()
}

impl Editor {
    /// Moves `path` to the top of the recent list and saves it; failures only cost the history.
    pub(super) fn remember(&mut self, path: &Path) {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
        if cfg!(test) {
            return;
        }
        if let Some(file) = state_file() {
            if let Some(dir) = file.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let list: Vec<String> = self.recent.iter().map(|p| p.display().to_string()).collect();
            let _ = fs::write(file, list.join("\n"));
        }
    }
}
