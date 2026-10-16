//! Recently opened files (`<Leader>fo`, dashboard "Recents"), kept across sessions in
//! `$XDG_STATE_HOME/lim/recent` (default `~/.local/state/lim/recent`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::Editor;

const MAX_RECENT: usize = 100;

fn state_file() -> Option<PathBuf> {
    Some(crate::config::state_dir()?.join("recent"))
}

pub fn load() -> Vec<PathBuf> {
    if cfg!(test) {
        return Vec::new();
    }
    let Some(file): Option<PathBuf> = state_file() else { return Vec::new() };
    fs::read_to_string(file).map(|s| s.lines().map(PathBuf::from).collect()).unwrap_or_default()
}

impl Editor {
    /// Moves `path` to the top of the recent list and saves it; failures only cost the history.
    pub(super) fn remember(&mut self, path: &Path) {
        let path: PathBuf = crate::buffer::canonical(path);
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
        if cfg!(test) {
            return;
        }
        if let Some(file) = state_file() {
            if let Some(dir) = file.parent() {
                let _: io::Result<()> = fs::create_dir_all(dir);
            }
            let list: Vec<String> = self.recent.iter().map(|p| p.display().to_string()).collect();
            let _: io::Result<()> = fs::write(file, list.join("\n"));
        }
    }
}
