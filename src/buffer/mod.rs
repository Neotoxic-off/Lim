//! A text buffer: lines, cursor, file I/O and undo.

pub mod git;
mod history;
mod motion;
pub mod regex;
pub mod text;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::string::FromUtf8Error;

use crate::error::Error;
use crate::syntax::{self, Hl, Lang};
use history::{Change, History};
pub use motion::CharClass;

/// A position in chars: `col` counts chars, not bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

pub struct Buffer {
    pub path: Option<PathBuf>,
    pub cursor: Pos,
    /// Column `j`/`k` aim for; `usize::MAX` sticks to the line end.
    pub want_col: usize,
    /// First visible line and first visible screen column.
    pub top: usize,
    pub left: usize,
    pub lang: Option<&'static Lang>,
    /// `m{a-z}` marks, plus `'` (position before the last jump) and `^` (last insert).
    pub marks: HashMap<char, Pos>,
    /// Name shown for buffers without a file (`git show` output…).
    pub title: Option<String>,
    /// Positions the jump list points at, kept in step with edits like marks.
    anchors: HashMap<u64, Pos>,
    /// Bumped on every change (unlike `version`, which typing within one undo step keeps).
    revision: u64,
    /// Completion words and the revision they were read at.
    word_cache: (u64, Vec<String>),
    lines: Vec<String>,
    crlf: bool,
    history: History,
    saved: u64,
    syntax: syntax::Cache,
    git: git::GitState,
}

impl Buffer {
    pub fn scratch() -> Self {
        Self::from_text(None, "")
    }

    pub fn from_text(path: Option<PathBuf>, text: &str) -> Self {
        let crlf: bool = text.contains("\r\n");
        let body: &str = text.strip_suffix('\n').unwrap_or(text);
        let lines: Vec<String> =
            body.split('\n').map(|l| if crlf { l.strip_suffix('\r').unwrap_or(l) } else { l }.to_owned()).collect();
        Self {
            lang: path.as_deref().and_then(syntax::lang_for),
            marks: HashMap::new(),
            title: None,
            anchors: HashMap::new(),
            revision: 0,
            word_cache: (u64::MAX, Vec::new()),
            path,
            cursor: Pos::default(),
            want_col: 0,
            top: 0,
            left: 0,
            lines,
            crlf,
            history: History::default(),
            saved: 0,
            syntax: syntax::Cache::default(),
            git: git::GitState::default(),
        }
    }

    /// Opens `path`, or an empty buffer bound to it when the file does not exist yet.
    pub fn open(path: &Path) -> Result<Self, Error> {
        // Devices and FIFOs would block or never end.
        if fs::metadata(path).is_ok_and(|m| !m.is_file()) {
            return Err(Error::NotAFile(path.into()));
        }
        match fs::read(path) {
            Ok(bytes) => {
                let text: String = String::from_utf8(bytes).map_err(|_| Error::NotUtf8(path.into()))?;
                Ok(Self::from_text(Some(path.into()), &text))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::from_text(Some(path.into()), "")),
            Err(e) => Err(Error::Io(path.into(), e)),
        }
    }

    /// Writes to `target` (default: own path). Refuses to clobber another existing file unless `force`.
    pub fn save(&mut self, target: Option<&Path>, force: bool) -> Result<String, Error> {
        let path: PathBuf = match (target, &self.path) {
            (Some(t), Some(own)) if t != own && t.exists() && !force => return Err(Error::FileExists(t.into())),
            (Some(t), _) => t.to_path_buf(),
            (None, Some(own)) => own.clone(),
            (None, None) => return Err(Error::NoFileName),
        };
        // Randomised tests type `:w name`; they must never write into the project.
        #[cfg(test)]
        if !path.starts_with(std::env::temp_dir()) {
            return Err(Error::Io(path, io::Error::other("tests only write under the temp dir")));
        }
        let out: String = self.contents();
        write_atomic(&path, out.as_bytes()).map_err(|e| Error::Io(path.clone(), e))?;
        let summary: String = format!("\"{}\" {}L, {}B written", path.display(), self.lines.len(), out.len());
        if self.path.is_none() {
            self.lang = syntax::lang_for(&path);
            self.syntax.invalidate(0);
            self.path = Some(path.clone());
        }
        if self.path.as_deref() == Some(path.as_path()) {
            self.history.seal();
            self.saved = self.history.state();
        }
        Ok(summary)
    }

    /// The file as it would be written: one terminator per line, nothing for an empty buffer.
    fn contents(&self) -> String {
        let newline: &str = if self.crlf { "\r\n" } else { "\n" };
        let mut out: String = String::with_capacity(self.lines.iter().map(|l| l.len() + 2).sum());
        if !(self.lines.len() == 1 && self.lines.first().is_some_and(String::is_empty)) {
            for line in &self.lines {
                out.push_str(line);
                out.push_str(newline);
            }
        }
        out
    }

    /// Emergency copy after an internal error; leaves the buffer's own state alone.
    pub fn write_copy(&self, path: &Path) -> io::Result<()> {
        fs::write(path, self.contents())
    }

    /// Restores the invariants (non-empty, cursor and scroll in range) after an internal error.
    pub fn repair(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.line = self.cursor.line.min(self.last_line());
        self.clamp(false);
        self.top = self.top.min(self.cursor.line);
        self.syntax.invalidate(0);
    }

    pub fn is_dirty(&self) -> bool {
        self.history.state() != self.saved
    }

    /// `:e!`: the file as it is on disk, dropping unsaved changes (undo brings them back).
    pub fn revert(&mut self) -> Result<String, Error> {
        let path: PathBuf = self.path.clone().ok_or(Error::NoFileName)?;
        let fresh: Buffer = Self::open(&path)?;
        let all: Range<usize> = 0..self.lines.len();
        self.history.seal();
        self.splice(all, fresh.lines);
        self.history.seal();
        self.saved = self.history.state();
        self.clamp(false);
        Ok(format!("\"{}\" {}L reloaded", path.display(), self.lines.len()))
    }

    /// `:saveas`: this buffer now edits `path`.
    pub fn set_path(&mut self, path: PathBuf) {
        self.lang = syntax::lang_for(&path);
        self.syntax.invalidate(0);
        self.path = Some(path);
    }

    /// Re-reads an unmodified buffer whose file changed on disk (after a shell or lazygit).
    pub fn reload_if_changed(&mut self) {
        let Some(path): Option<PathBuf> = self.path.clone() else { return };
        if self.is_dirty() {
            return;
        }
        let Ok(bytes): Result<Vec<u8>, io::Error> = fs::read(&path) else { return };
        let Ok(text): Result<String, FromUtf8Error> = String::from_utf8(bytes) else { return };
        let fresh: Buffer = Self::from_text(None, &text);
        if fresh.lines != self.lines {
            let all: Range<usize> = 0..self.lines.len();
            self.splice(all, fresh.lines);
            self.history.seal();
            self.saved = self.history.state();
            self.clamp(false);
            self.load_git_base();
        }
    }

    /// Changes whenever the text does (edits, undo, redo).
    pub fn version(&self) -> u64 {
        self.history.state()
    }

    pub fn name(&self) -> String {
        self.path.as_deref().and_then(Path::file_name).map_or_else(
            || self.title.clone().unwrap_or_else(|| "[No Name]".into()),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn line(&self, y: usize) -> &str {
        &self.lines[y]
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn last_line(&self) -> usize {
        self.lines.len() - 1
    }

    pub fn line_len(&self, y: usize) -> usize {
        text::char_len(&self.lines[y])
    }

    pub fn first_non_blank(&self, y: usize) -> usize {
        text::char_len(text::indent(&self.lines[y]))
    }

    /// Keeps the cursor on a char, or one past the end when `past_end` (insert mode).
    pub fn clamp(&mut self, past_end: bool) {
        let line: usize = self.cursor.line.min(self.last_line());
        let len: usize = self.line_len(line);
        let max: usize = if past_end { len } else { len.saturating_sub(1) };
        self.cursor = Pos::new(line, self.cursor.col.min(max));
    }

    pub fn seal(&mut self) {
        self.history.seal();
    }

    /// Keeps every change in the current undo step until released (`:g`, `:norm`).
    pub fn hold_undo(&mut self, held: bool) {
        self.history.hold(held);
    }

    pub fn undo(&mut self) -> bool {
        self.syntax.invalidate(0);
        self.revision += 1;
        let (marks, anchors): (&mut HashMap<char, Pos>, &mut HashMap<u64, Pos>) = (&mut self.marks, &mut self.anchors);
        let cursor: Option<Pos> = self
            .history
            .undo(&mut self.lines, &mut |at, removed, inserted| shift_positions(marks, anchors, at, removed, inserted));
        cursor.map(|p| self.cursor = p).is_some()
    }

    pub fn redo(&mut self) -> bool {
        self.syntax.invalidate(0);
        self.revision += 1;
        let (marks, anchors): (&mut HashMap<char, Pos>, &mut HashMap<u64, Pos>) = (&mut self.marks, &mut self.anchors);
        let cursor: Option<Pos> = self
            .history
            .redo(&mut self.lines, &mut |at, removed, inserted| shift_positions(marks, anchors, at, removed, inserted));
        cursor.map(|p| self.cursor = p).is_some()
    }

    /// Words for completion, re-read only when the text changed. Huge files contribute the
    /// lines around the cursor only.
    pub fn words(&mut self) -> &[String] {
        const WINDOW: usize = 5_000;
        if self.word_cache.0 != self.revision {
            let lines: &[String] = if self.lines.len() > 4 * WINDOW {
                let from: usize = self.cursor.line.saturating_sub(WINDOW);
                &self.lines[from..(self.cursor.line + WINDOW).min(self.lines.len())]
            } else {
                &self.lines[..]
            };
            self.word_cache = (self.revision, crate::editor::words_of(lines));
        }
        &self.word_cache.1
    }

    /// Remembers `at` as a position that follows edits (jump list entries); returns its id.
    pub fn add_anchor(&mut self, id: u64, at: Pos) {
        self.anchors.insert(id, at);
    }

    pub fn anchor(&self, id: u64) -> Option<Pos> {
        self.anchors.get(&id).copied()
    }

    pub fn remove_anchor(&mut self, id: u64) {
        self.anchors.remove(&id);
    }

    /// The single mutation point: every edit goes through here so it is undoable.
    pub fn splice(&mut self, range: Range<usize>, new: Vec<String>) {
        self.syntax.invalidate(range.start);
        self.revision += 1;
        shift_positions(&mut self.marks, &mut self.anchors, range.start, range.len(), new.len());
        let new_len: usize = new.len();
        let old: Vec<String> = self.lines.splice(range.clone(), new).collect();
        self.history.record(Change { at: range.start, old, new_len, cursor: self.cursor });
        if self.lines.is_empty() {
            self.splice(0..0, vec![String::new()]);
        }
    }

    pub fn set_line(&mut self, y: usize, line: String) {
        self.splice(y..y + 1, vec![line]);
    }

    /// Text from `start` to `end` (exclusive), lines joined by '\n'.
    pub fn text(&self, start: Pos, end: Pos) -> String {
        let (from, to): (usize, usize) = (self.byte(start), self.byte(end));
        if start.line == end.line {
            return self.lines[start.line][from..to].to_owned();
        }
        let mut s: String = self.lines[start.line][from..].to_owned();
        for line in &self.lines[start.line + 1..end.line] {
            s.push('\n');
            s.push_str(line);
        }
        s.push('\n');
        s.push_str(&self.lines[end.line][..to]);
        s
    }

    pub fn remove(&mut self, start: Pos, end: Pos) -> String {
        let removed: String = self.text(start, end);
        let joined: String =
            format!("{}{}", &self.lines[start.line][..self.byte(start)], &self.lines[end.line][self.byte(end)..]);
        self.splice(start.line..end.line + 1, vec![joined]);
        removed
    }

    /// Inserts `s` (which may hold '\n') at `at`; returns the position right after it.
    pub fn insert(&mut self, at: Pos, s: &str) -> Pos {
        let (head, tail): (&str, &str) = self.lines[at.line].split_at(self.byte(at));
        let mut parts: Vec<String> = s.split('\n').map(str::to_owned).collect();
        let last: usize = parts.len() - 1;
        let end_col: usize = if last == 0 { at.col } else { 0 } + text::char_len(&parts[last]);
        parts[0].insert_str(0, head);
        parts[last].push_str(tail);
        self.splice(at.line..at.line + 1, parts);
        Pos::new(at.line + last, end_col)
    }

    /// Re-reads the git index version (on open and after writes).
    pub fn load_git_base(&mut self) {
        if let Some(path) = self.path.clone() {
            self.git.load(&path);
        }
    }

    /// gitsigns for every line, recomputed only when the text changed.
    pub fn git_signs(&mut self) -> &[Option<git::Sign>] {
        let version: u64 = self.version();
        self.git.signs(&self.lines, version)
    }

    /// `]g` / `[g`: first line of the next (or previous) hunk, wrapping around.
    pub fn next_hunk(&mut self, from: usize, forward: bool) -> Option<usize> {
        let signs: &[Option<git::Sign>] = self.git_signs();
        let starts: Vec<usize> =
            (0..signs.len()).filter(|&y| signs[y].is_some() && (y == 0 || signs[y - 1].is_none())).collect();
        if forward {
            starts.iter().find(|&&y| y > from).or(starts.first()).copied()
        } else {
            starts.iter().rev().find(|&&y| y < from).or(starts.last()).copied()
        }
    }

    /// Highlight groups for line `y`, one per char; None for files without a known language.
    pub fn highlight_line(&mut self, y: usize) -> Option<Vec<Hl>> {
        Some(self.syntax.line(&self.lines, self.lang?, y))
    }

    fn byte(&self, p: Pos) -> usize {
        text::byte_index(&self.lines[p.line], p.col)
    }
}

/// Anchors with this bit set vanish when their line is deleted (the lines `:g` still has to visit).
pub const DROP_ON_DELETE: u64 = 1 << 63;

/// Moves marks and anchors after `removed` lines at `at` were replaced by `inserted` lines.
/// Lines below shift; positions on lines that went away land on the last new line, and a
/// letter mark whose line was deleted outright is dropped, as in Vim.
fn shift_positions(
    marks: &mut HashMap<char, Pos>,
    anchors: &mut HashMap<u64, Pos>,
    at: usize,
    removed: usize,
    inserted: usize,
) {
    if removed == inserted {
        return;
    }
    let moved = |p: &mut Pos| -> bool {
        if p.line >= at + removed {
            p.line = p.line - removed + inserted;
        } else if p.line >= at + inserted {
            if inserted == 0 {
                // Clamped to the buffer when used.
                p.line = at;
                p.col = 0;
                return false;
            }
            p.line = at + inserted - 1;
        }
        true
    };
    marks.retain(|name, p| moved(p) || !name.is_ascii_lowercase());
    anchors.retain(|id, p| moved(p) || id & DROP_ON_DELETE == 0);
}

/// The absolute path with symlinks resolved, or `path` itself when that fails. On Windows the `\\?\` prefix
/// `canonicalize` adds is dropped, so paths compare equal to and read like the ones the user typed.
pub fn canonical(path: &Path) -> PathBuf {
    let resolved: PathBuf = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if cfg!(windows)
        && let Some(plain) = resolved.to_str().and_then(|s| s.strip_prefix(r"\\?\"))
        && !plain.starts_with(r"UNC\")
    {
        return PathBuf::from(plain);
    }
    resolved
}

/// Writes through a temp file + rename, so a crash or full disk never leaves a truncated file.
/// Follows symlinks and keeps permissions; falls back to an in-place write when the directory is read-only.
fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let target: PathBuf = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let Some(name): Option<&OsStr> = target.file_name() else { return fs::write(&target, data) };
    let tmp: PathBuf = target.with_file_name(format!(".{}.lim-{}.tmp", name.to_string_lossy(), std::process::id()));
    let staged: Result<(), io::Error> = File::create(&tmp).and_then(|mut f| {
        f.write_all(data)?;
        f.sync_all()?;
        if let Ok(meta) = fs::metadata(&target) {
            f.set_permissions(meta.permissions())?;
        }
        Ok(())
    });
    if let Ok(()) = staged.and_then(|()| fs::rename(&tmp, &target)) {
        Ok(())
    } else {
        let _: Result<(), io::Error> = fs::remove_file(&tmp);
        fs::write(&target, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(b: &Buffer) -> String {
        b.lines().join("\n")
    }

    #[test]
    fn edit_undo_redo() {
        let mut b: Buffer = Buffer::from_text(None, "héllo\nworld\n");
        let end: Pos = b.insert(Pos::new(0, 1), "X\nY");
        assert_eq!((joined(&b), end), ("hX\nYéllo\nworld".into(), Pos::new(1, 1)));
        b.seal();
        assert_eq!(b.remove(Pos::new(0, 1), Pos::new(2, 2)), "X\nYéllo\nwo");
        assert_eq!(joined(&b), "hrld");
        assert!(b.undo());
        assert_eq!(joined(&b), "hX\nYéllo\nworld");
        assert!(b.undo());
        assert_eq!(joined(&b), "héllo\nworld");
        assert!(!b.is_dirty());
        assert!(b.redo());
        assert!(b.is_dirty());
    }

    #[test]
    fn save_round_trip_keeps_crlf() {
        let path: PathBuf = std::env::temp_dir().join(format!("lim-test-{}.txt", std::process::id()));
        let mut b: Buffer = Buffer::from_text(Some(path.clone()), "a\r\nb\r\n");
        b.insert(Pos::new(0, 1), "!");
        assert!(b.is_dirty());
        b.save(None, false).unwrap();
        assert!(!b.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "a!\r\nb\r\n");
        fs::remove_file(path).unwrap();
    }
}
