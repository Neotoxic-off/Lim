//! snacks.picker: a fuzzy finder over files, git files, text (grep), buffers, recent files,
//! keymaps, commands and lines, with a preview.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};

use super::command::Command;
use super::{Editor, Mode};
use crate::buffer::{Pos, text};
use crate::config::keys;
use crate::term::Key;

/// Listing stops here so a huge tree never stalls the editor.
const MAX_FILES: usize = 50_000;
/// Grep stops after this many hits, and skips files larger than this.
const MAX_HITS: usize = 1_000;
const MAX_GREP_BYTES: u64 = 2 * 1024 * 1024;
/// Grep starts at this query length; shorter queries match nearly everything.
const MIN_GREP_QUERY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Files { all: bool },
    GitFiles,
    Grep { all: bool },
    Buffers,
    Recent,
    Keymaps,
    Commands,
    Lines,
}

impl PickerKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Files { .. } => "Files",
            Self::GitFiles => "Git Files",
            Self::Grep { .. } => "Grep",
            Self::Buffers => "Buffers",
            Self::Recent => "Recent",
            Self::Keymaps => "Keymaps",
            Self::Commands => "Commands",
            Self::Lines => "Lines",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    File(PathBuf, Option<Pos>),
    Buffer(usize),
    Keys(Vec<Key>),
    Command(String),
    Line(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub target: Target,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub item: usize,
    pub score: i64,
    /// Char indexes of `label` to highlight.
    pub positions: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Picker {
    pub kind: PickerKind,
    pub query: String,
    pub items: Vec<Item>,
    pub matches: Vec<Match>,
    pub selected: usize,
    /// Files to search, for grep (whose items depend on the query).
    files: Vec<PathBuf>,
}

impl Picker {
    pub fn selected_item(&self) -> Option<&Item> {
        self.matches.get(self.selected).and_then(|m| self.items.get(m.item))
    }

    fn refilter(&mut self) {
        if let PickerKind::Grep { .. } = self.kind {
            self.items = grep(&self.files, &self.query);
            self.matches = self
                .items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let positions = text::find_all(&item.label, &self.query).into_iter().flatten().collect();
                    Match { item: i, score: 0, positions }
                })
                .collect();
        } else {
            self.matches = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| fuzzy(&self.query, &item.label).map(|(score, positions)| Match { item: i, score, positions }))
                .collect();
            if !self.query.is_empty() {
                // Stable: equal scores keep the source order (recent first, buffers in tab order).
                self.matches.sort_by_key(|m| std::cmp::Reverse(m.score));
            }
        }
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
    }
}

/// fzf-like subsequence match: consecutive chars and word starts score higher. Smartcase.
pub fn fuzzy(query: &str, text: &str) -> Option<(i64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let sensitive = query.chars().any(char::is_uppercase);
    let fold = |c: char| if sensitive { c } else { c.to_ascii_lowercase() };
    let chars: Vec<char> = text.chars().collect();
    // Matches in the file name beat matches in its directories.
    let name_start = chars.iter().rposition(|&c| c == '/').map_or(0, |i| i + 1);
    let mut positions = Vec::with_capacity(query.len());
    let mut score = 0i64;
    let mut from = 0;
    for q in query.chars().map(fold) {
        let i = (from..chars.len()).find(|&i| fold(chars[i]) == q)?;
        let boundary = i == 0 || matches!(chars[i - 1], '/' | '_' | '-' | '.' | ' ' | ':')
            || (chars[i].is_uppercase() && chars[i - 1].is_lowercase());
        score += 16;
        score += if positions.last() == Some(&(i.wrapping_sub(1))) { 12 } else { -((i - from) as i64).min(8) };
        score += if boundary { 10 } else { 0 };
        score += if i >= name_start { 4 } else { 0 };
        positions.push(i);
        from = i + 1;
    }
    Some((score - chars.len() as i64 / 8, positions))
}

/// Files under the working directory: `git ls-files` when in a repo (respects .gitignore), else a walk.
fn list_files(all: bool) -> Vec<PathBuf> {
    if !all && let Some(files) = git_files() {
        return files;
    }
    let mut files = Vec::new();
    let mut stack = vec![PathBuf::from(".")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let skip = name == ".git" || (!all && (name.starts_with('.') || name == "target" || name == "node_modules"));
            let Ok(kind) = entry.file_type() else { continue };
            if skip {
                continue;
            }
            let path = entry.path();
            let path = path.strip_prefix("./").map(Path::to_path_buf).unwrap_or(path);
            // Symlinked directories are not followed, so a link loop cannot trap the walk.
            if kind.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
                if files.len() >= MAX_FILES {
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

fn git_files() -> Option<Vec<PathBuf>> {
    let out = Process::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    Some(text.split('\0').filter(|p| !p.is_empty() && Path::new(p).is_file()).take(MAX_FILES).map(PathBuf::from).collect())
}

/// Literal search (following ignorecase/smartcase) in text files.
// shortcut: scans files on each keystroke; fine for project-sized trees, use an index for huge monorepos.
fn grep(files: &[PathBuf], query: &str) -> Vec<Item> {
    let mut items = Vec::new();
    if text::char_len(query) < MIN_GREP_QUERY {
        return items;
    }
    for path in files {
        if fs::metadata(path).is_ok_and(|m| m.len() > MAX_GREP_BYTES) {
            continue;
        }
        let Ok(bytes) = fs::read(path) else { continue };
        if bytes.iter().take(8000).any(|&b| b == 0) {
            continue;
        }
        let Ok(content) = String::from_utf8(bytes) else { continue };
        for (y, line) in content.lines().enumerate() {
            if let Some(hit) = text::find_all(line, query).first() {
                let label = format!("{}:{}:{}  {}", path.display(), y + 1, hit.start + 1, line.trim());
                items.push(Item { label, target: Target::File(path.clone(), Some(Pos::new(y, hit.start))) });
                if items.len() >= MAX_HITS {
                    return items;
                }
            }
        }
    }
    items
}

impl Editor {
    pub(super) fn open_picker(&mut self, kind: PickerKind) {
        let items = match kind {
            PickerKind::Files { all } => file_items(list_files(all)),
            PickerKind::GitFiles => match git_files() {
                Some(files) => file_items(files),
                None => return self.info("Not a git repository"),
            },
            PickerKind::Grep { .. } => Vec::new(),
            PickerKind::Buffers => self
                .buffers
                .iter()
                .enumerate()
                .map(|(i, b)| Item {
                    label: b.path.as_ref().map_or_else(|| b.name(), |p| p.display().to_string()),
                    target: Target::Buffer(i),
                })
                .collect(),
            PickerKind::Recent => file_items(self.recent.iter().filter(|p| p.is_file()).cloned().collect()),
            PickerKind::Keymaps => self.keymap_items(),
            PickerKind::Commands => COMMANDS
                .iter()
                .map(|(cmd, desc)| Item { label: format!("{cmd:<12}{desc}"), target: Target::Command((*cmd).to_owned()) })
                .collect(),
            PickerKind::Lines => self
                .buf()
                .lines()
                .iter()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(y, l)| Item { label: format!("{:>5}  {}", y + 1, l.trim()), target: Target::Line(y) })
                .collect(),
        };
        let files = match kind {
            PickerKind::Grep { all } => list_files(all),
            _ => Vec::new(),
        };
        let mut picker = Picker { kind, query: String::new(), items, matches: Vec::new(), selected: 0, files };
        picker.refilter();
        self.picker = Some(picker);
        self.mode = Mode::Picker;
    }

    /// `<Leader>fc`: grep for the word under the cursor.
    pub(super) fn grep_word(&mut self) {
        let buf = self.buf();
        let word = buf.word_object(buf.cursor, false).map(|(s, e)| buf.text(s, e)).unwrap_or_default();
        self.open_picker(PickerKind::Grep { all: false });
        if let Some(picker) = &mut self.picker {
            picker.query = word;
            picker.refilter();
        }
    }

    /// `<Leader>f<CR>`: reopen the last picker as it was.
    pub(super) fn resume_picker(&mut self) {
        if let Some(picker) = self.last_picker.take() {
            self.picker = Some(picker);
            self.mode = Mode::Picker;
        }
    }

    pub(super) fn picker_key(&mut self, key: Key) {
        let Some(picker) = &mut self.picker else {
            self.mode = Mode::Normal;
            return;
        };
        let count = picker.matches.len();
        match key {
            Key::Esc | Key::Ctrl('c') => self.close_picker(),
            Key::Enter => self.accept_picker(None),
            Key::Ctrl('v') => self.accept_picker(Some(true)),
            Key::Ctrl('s') => self.accept_picker(Some(false)),
            Key::Down | Key::Ctrl('n') | Key::Ctrl('j') | Key::Tab => picker.selected = (picker.selected + 1).min(count.saturating_sub(1)),
            Key::Up | Key::Ctrl('p') | Key::Ctrl('k') | Key::BackTab => picker.selected = picker.selected.saturating_sub(1),
            Key::PageDown | Key::Ctrl('d') => picker.selected = (picker.selected + 10).min(count.saturating_sub(1)),
            Key::PageUp | Key::Ctrl('u') => picker.selected = picker.selected.saturating_sub(10),
            Key::Backspace | Key::Ctrl('h') => {
                picker.query.pop();
                picker.refilter();
            }
            Key::Ctrl('w') => {
                picker.query.clear();
                picker.refilter();
            }
            Key::Char(c) => {
                picker.query.push(c);
                picker.selected = 0;
                picker.refilter();
            }
            _ => {}
        }
    }

    fn close_picker(&mut self) {
        self.last_picker = self.picker.take();
        self.mode = Mode::Normal;
    }

    /// Opens the selection; `split` is Some(vertical) for `<C-v>` / `<C-s>`.
    fn accept_picker(&mut self, split: Option<bool>) {
        let target = self.picker.as_ref().and_then(Picker::selected_item).map(|i| i.target.clone());
        self.close_picker();
        let Some(target) = target else { return };
        if let Some(vertical) = split
            && matches!(target, Target::File(..) | Target::Buffer(_))
        {
            self.split(vertical);
        }
        match target {
            Target::File(path, pos) => {
                self.open(&path);
                self.dashboard = false;
                if let Some(pos) = pos {
                    let buf = self.buf_mut();
                    buf.cursor = pos;
                    buf.clamp(false);
                }
            }
            Target::Buffer(i) => {
                self.switch_to(i);
                self.dashboard = false;
            }
            Target::Line(y) => {
                let buf = self.buf_mut();
                buf.cursor = Pos::new(y, buf.first_non_blank(y.min(buf.last_line())));
                buf.clamp(false);
            }
            Target::Command(cmd) => match Command::parse(&cmd) {
                Ok(Some(command)) => self.run(command),
                Ok(None) => {}
                Err(e) => self.error(e),
            },
            Target::Keys(keys) => {
                for key in keys {
                    self.handle(key);
                }
            }
        }
    }

    fn keymap_items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = self
            .keymaps
            .iter()
            .filter_map(|m| {
                let rhs = m.rhs.clone()?;
                let desc = if m.desc.is_empty() { keys::display(&rhs) } else { m.desc.clone() };
                Some(Item { label: format!("{:<14}{desc}", keys::display(&m.lhs)), target: Target::Keys(m.lhs.clone()) })
            })
            .collect();
        for (prefix, entries) in super::ui::builtin_maps() {
            for (key, desc) in entries {
                let lhs = format!("{prefix}{key}");
                let Ok(keys) = keys::parse(&lhs) else { continue };
                items.push(Item { label: format!("{lhs:<14}{desc}"), target: Target::Keys(keys) });
            }
        }
        items
    }
}

fn file_items(files: Vec<PathBuf>) -> Vec<Item> {
    files.into_iter().map(|p| Item { label: p.display().to_string(), target: Target::File(p, None) }).collect()
}

/// `<Leader>fC`: the ex commands lim understands.
const COMMANDS: &[(&str, &str)] = &[
    ("w", "Write the buffer"),
    ("wa", "Write all buffers"),
    ("q", "Close the window or quit"),
    ("qa", "Quit"),
    ("wq", "Write and quit"),
    ("x", "Write if changed and quit"),
    ("enew", "New empty buffer"),
    ("bn", "Next buffer"),
    ("bp", "Previous buffer"),
    ("bd", "Delete buffer"),
    ("split", "Split window"),
    ("vsplit", "Split window vertically"),
    ("only", "Close other windows"),
    ("close", "Close window"),
    ("noh", "Clear search highlight"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_scores_name_and_runs() {
        assert!(fuzzy("xyz", "src/main.rs").is_none());
        let (_, pos) = fuzzy("mrs", "src/main.rs").unwrap_or_default();
        assert_eq!(pos, vec![4, 9, 10]);
        let score = |q, t| fuzzy(q, t).map(|(s, _)| s).unwrap_or(i64::MIN);
        assert!(score("main", "src/main.rs") > score("main", "src/manifest/inner.rs"));
        assert!(score("view", "src/render/view.rs") > score("view", "src/editor/visual_review.rs"));
        assert!(fuzzy("Main", "src/main.rs").is_none());
    }
}
