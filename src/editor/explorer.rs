//! neo-tree: a file tree on the left (`<Leader>e` toggles, `<Leader>o` focuses).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::prompt::PromptAction;
use super::{Editor, Mode};
use crate::term::Key;

/// neo-tree's default window width.
pub const WIDTH: usize = 30;
/// Entries listed per directory at most.
const MAX_ENTRIES: usize = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitStatus {
    Modified,
    Added,
    Untracked,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    pub expanded: bool,
    /// Last child of its directory (`└` marker).
    pub last: bool,
    pub git: Option<GitStatus>,
}

#[derive(Debug)]
pub struct Explorer {
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    pub cursor: usize,
    pub scroll: usize,
    pub focused: bool,
    show_hidden: bool,
    expanded: HashSet<PathBuf>,
    git: HashMap<PathBuf, GitStatus>,
}

impl Explorer {
    pub fn new(root: PathBuf) -> Self {
        let mut explorer = Self {
            root,
            entries: Vec::new(),
            cursor: 0,
            scroll: 0,
            focused: true,
            show_hidden: false,
            expanded: HashSet::new(),
            git: HashMap::new(),
        };
        explorer.refresh();
        explorer
    }

    /// Re-reads the tree and git status, keeping the cursor on the same path when it still exists.
    pub fn refresh(&mut self) {
        let selected = self.entries.get(self.cursor).map(|e| e.path.clone());
        self.git = git_status(&self.root);
        self.entries.clear();
        let root = self.root.clone();
        self.list(&root, 0);
        self.cursor = selected.and_then(|p| self.entries.iter().position(|e| e.path == p)).unwrap_or(0);
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    fn list(&mut self, dir: &Path, depth: usize) {
        let Ok(read) = fs::read_dir(dir) else { return };
        let mut children: Vec<(String, PathBuf, bool)> = read
            .flatten()
            .take(MAX_ENTRIES)
            .map(|e| {
                let path = e.path();
                // Symlinked directories show as directories but are only listed when opened.
                (e.file_name().to_string_lossy().into_owned(), path.clone(), path.is_dir())
            })
            .filter(|(name, _, _)| self.show_hidden || !name.starts_with('.'))
            .collect();
        children.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        let count = children.len();
        for (i, (name, path, dir)) in children.into_iter().enumerate() {
            let expanded = dir && self.expanded.contains(&path);
            let git = self.status_of(&path, dir);
            self.entries.push(Entry { path: path.clone(), name, depth, dir, expanded, last: i + 1 == count, git });
            if expanded {
                self.list(&path, depth + 1);
            }
        }
    }

    /// A directory shows as modified when anything below it changed, like neo-tree.
    fn status_of(&self, path: &Path, dir: bool) -> Option<GitStatus> {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        if dir {
            self.git.keys().any(|p| p.starts_with(rel)).then_some(GitStatus::Modified)
        } else {
            self.git.get(rel).copied()
        }
    }

    pub fn selected(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }
}

/// `git status --porcelain` under `root`, by path relative to it.
fn git_status(root: &Path) -> HashMap<PathBuf, GitStatus> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "-z", "--untracked-files=all", "--relative"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else { return HashMap::new() };
    let text = String::from_utf8_lossy(&output.stdout);
    text.split('\0')
        .filter_map(|record| {
            let (code, path) = (record.get(..2)?, record.get(3..)?);
            let status = match code {
                "??" => GitStatus::Untracked,
                c if c.contains('D') => GitStatus::Deleted,
                c if c.starts_with('A') => GitStatus::Added,
                _ => GitStatus::Modified,
            };
            Some((PathBuf::from(path), status))
        })
        .collect()
}

impl Editor {
    /// `<Leader>e`
    pub(super) fn toggle_explorer(&mut self) {
        if self.explorer.take().is_none() {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            self.explorer = Some(Explorer::new(root));
            self.mode = Mode::Explorer;
        } else if self.mode == Mode::Explorer {
            self.mode = Mode::Normal;
        }
    }

    /// `<Leader>o`: focus the tree, or back to the file when it already has focus.
    pub(super) fn focus_explorer(&mut self) {
        match &mut self.explorer {
            Some(explorer) if self.mode == Mode::Explorer => {
                explorer.focused = false;
                self.mode = Mode::Normal;
            }
            Some(explorer) => {
                explorer.focused = true;
                self.mode = Mode::Explorer;
            }
            None => self.toggle_explorer(),
        }
    }

    pub(super) fn explorer_key(&mut self, key: Key) {
        // `<C-w>` window commands work from the tree too.
        if self.pending.prefix.take() == Some(super::ui::CTRL_W) {
            match key {
                Key::Char('w' | 'l' | 'p') | Key::Ctrl('w' | 'l' | 'p') => self.leave_tree_to_first_window(),
                Key::Char('q' | 'c') | Key::Ctrl('q' | 'c') => {
                    self.explorer = None;
                    self.mode = Mode::Normal;
                }
                _ => {}
            }
            return;
        }
        if key == Key::Ctrl('w') {
            self.pending.prefix = Some(super::ui::CTRL_W);
            return;
        }
        let Some(explorer) = &mut self.explorer else {
            self.mode = Mode::Normal;
            return;
        };
        let last = explorer.entries.len().saturating_sub(1);
        match key {
            Key::Char('j') | Key::Down => explorer.cursor = (explorer.cursor + 1).min(last),
            Key::Char('k') | Key::Up => explorer.cursor = explorer.cursor.saturating_sub(1),
            Key::Char('G') => explorer.cursor = last,
            Key::Char('g') => explorer.cursor = 0,
            Key::Ctrl('d') => explorer.cursor = (explorer.cursor + 10).min(last),
            Key::Ctrl('u') => explorer.cursor = explorer.cursor.saturating_sub(10),
            Key::Char('H') => {
                explorer.show_hidden = !explorer.show_hidden;
                explorer.refresh();
            }
            Key::Char('R') => explorer.refresh(),
            Key::Char('h') | Key::Left => {
                let Some(entry) = explorer.selected().cloned() else { return };
                if entry.dir && entry.expanded {
                    explorer.expanded.remove(&entry.path);
                    explorer.refresh();
                } else if let Some(parent) = explorer.entries[..explorer.cursor].iter().rposition(|e| e.depth < entry.depth) {
                    explorer.cursor = parent;
                }
            }
            Key::Enter | Key::Char('l' | 'o') | Key::Right => {
                let Some(entry) = explorer.selected().cloned() else { return };
                if entry.dir {
                    if !explorer.expanded.remove(&entry.path) {
                        explorer.expanded.insert(entry.path);
                    }
                    explorer.refresh();
                } else {
                    explorer.focused = false;
                    self.mode = Mode::Normal;
                    self.dashboard = false;
                    self.open(&entry.path);
                }
            }
            Key::Char('q') => {
                self.explorer = None;
                self.mode = Mode::Normal;
            }
            // neo-tree's file operations.
            Key::Char('a' | 'A') => {
                let dir = explorer.selected().map_or_else(|| explorer.root.clone(), |e| {
                    if e.dir { e.path.clone() } else { e.path.parent().map_or_else(|| explorer.root.clone(), Path::to_path_buf) }
                });
                let directory = key == Key::Char('A');
                let title = if directory { "Add a directory" } else { "Add a file (end with / for a directory)" };
                self.open_prompt(title, "", PromptAction::TreeAdd { dir, directory });
            }
            Key::Char('r') => {
                let Some(entry) = explorer.selected().cloned() else { return };
                self.open_prompt("Rename", &entry.name, PromptAction::TreeRename { from: entry.path });
            }
            Key::Char('d') => {
                let Some(entry) = explorer.selected().cloned() else { return };
                self.open_prompt(&format!("Delete {}? (y/N)", entry.name), "", PromptAction::TreeDelete { path: entry.path });
            }
            Key::Char('c' | 'm') => {
                let Some(entry) = explorer.selected().cloned() else { return };
                let shown = entry.path.strip_prefix(&explorer.root).unwrap_or(&entry.path).display().to_string();
                if key == Key::Char('c') {
                    self.open_prompt("Copy to", &shown, PromptAction::TreeCopy { from: entry.path });
                } else {
                    self.open_prompt("Move to", &shown, PromptAction::TreeMove { from: entry.path });
                }
            }
            Key::Char('y' | 'Y') => {
                let Some(entry) = explorer.selected().cloned() else { return };
                let text = if key == Key::Char('Y') { entry.path.display().to_string() } else { entry.name };
                self.store_register(super::registers::Register { text: text.clone(), linewise: false, block: false }, true);
                self.info(format!("Copied {text}"));
            }
            Key::Char('s' | 'S') => {
                let Some(entry) = explorer.selected().cloned().filter(|e| !e.dir) else { return };
                explorer.focused = false;
                self.mode = Mode::Normal;
                self.dashboard = false;
                self.split(key == Key::Char('s'));
                self.open(&entry.path);
            }
            Key::Char('P') => {
                // Preview: open the file but keep the focus in the tree.
                let Some(entry) = explorer.selected().cloned().filter(|e| !e.dir) else { return };
                self.dashboard = false;
                self.open(&entry.path);
            }
            Key::Char('O') => {
                let Some(entry) = explorer.selected().cloned() else { return };
                super::extras::open_external(&entry.path.display().to_string());
            }
            Key::Char('.') => {
                let Some(entry) = explorer.selected().cloned().filter(|e| e.dir) else { return };
                explorer.root = entry.path;
                explorer.cursor = 0;
                explorer.refresh();
            }
            Key::Backspace => {
                if let Some(parent) = explorer.root.parent().map(Path::to_path_buf) {
                    explorer.root = parent;
                    explorer.cursor = 0;
                    explorer.refresh();
                }
            }
            Key::Char('?') => self.open_cheatsheet(),
            Key::Esc | Key::Ctrl('l') => {
                explorer.focused = false;
                self.mode = Mode::Normal;
            }
            Key::Char(':') => {
                explorer.focused = false;
                self.cmdline.clear();
                self.mode = Mode::Command;
            }
            Key::Char(c) if c == crate::config::keys::LEADER => {
                explorer.focused = false;
                self.mode = Mode::Normal;
                self.pending.prefix = Some(c);
            }
            _ => {}
        }
    }
}
