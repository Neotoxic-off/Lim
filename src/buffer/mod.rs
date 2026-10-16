//! A text buffer: lines, cursor, file I/O and undo.

mod history;
mod motion;
pub mod text;

use std::fs::{self, File};
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

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
    lines: Vec<String>,
    crlf: bool,
    history: History,
    saved: u64,
    syntax: syntax::Cache,
}

impl Buffer {
    pub fn scratch() -> Self {
        Self::from_text(None, "")
    }

    pub fn from_text(path: Option<PathBuf>, text: &str) -> Self {
        let crlf = text.contains("\r\n");
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = body
            .split('\n')
            .map(|l| if crlf { l.strip_suffix('\r').unwrap_or(l) } else { l }.to_owned())
            .collect();
        Self {
            lang: path.as_deref().and_then(syntax::lang_for),
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
                let text = String::from_utf8(bytes).map_err(|_| Error::NotUtf8(path.into()))?;
                Ok(Self::from_text(Some(path.into()), &text))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::from_text(Some(path.into()), "")),
            Err(e) => Err(Error::Io(path.into(), e)),
        }
    }

    /// Writes to `target` (default: own path). Refuses to clobber another existing file unless `force`.
    pub fn save(&mut self, target: Option<&Path>, force: bool) -> Result<String, Error> {
        let path = match (target, &self.path) {
            (Some(t), Some(own)) if t != own && t.exists() && !force => return Err(Error::FileExists(t.into())),
            (Some(t), _) => t.to_path_buf(),
            (None, Some(own)) => own.clone(),
            (None, None) => return Err(Error::NoFileName),
        };
        let out = self.contents();
        write_atomic(&path, out.as_bytes()).map_err(|e| Error::Io(path.clone(), e))?;
        let summary = format!("\"{}\" {}L, {}B written", path.display(), self.lines.len(), out.len());
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
        let newline = if self.crlf { "\r\n" } else { "\n" };
        let mut out = String::with_capacity(self.lines.iter().map(|l| l.len() + 2).sum());
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

    /// Changes whenever the text does (edits, undo, redo).
    pub fn version(&self) -> u64 {
        self.history.state()
    }

    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| "[No Name]".into(), |n| n.to_string_lossy().into_owned())
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
        let line = self.cursor.line.min(self.last_line());
        let len = self.line_len(line);
        let max = if past_end { len } else { len.saturating_sub(1) };
        self.cursor = Pos::new(line, self.cursor.col.min(max));
    }

    pub fn seal(&mut self) {
        self.history.seal();
    }

    pub fn undo(&mut self) -> bool {
        self.syntax.invalidate(0);
        self.history.undo(&mut self.lines).map(|p| self.cursor = p).is_some()
    }

    pub fn redo(&mut self) -> bool {
        self.syntax.invalidate(0);
        self.history.redo(&mut self.lines).map(|p| self.cursor = p).is_some()
    }

    /// The single mutation point: every edit goes through here so it is undoable.
    pub fn splice(&mut self, range: Range<usize>, new: Vec<String>) {
        self.syntax.invalidate(range.start);
        let new_len = new.len();
        let old = self.lines.splice(range.clone(), new).collect();
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
        let (from, to) = (self.byte(start), self.byte(end));
        if start.line == end.line {
            return self.lines[start.line][from..to].to_owned();
        }
        let mut s = self.lines[start.line][from..].to_owned();
        for line in &self.lines[start.line + 1..end.line] {
            s.push('\n');
            s.push_str(line);
        }
        s.push('\n');
        s.push_str(&self.lines[end.line][..to]);
        s
    }

    pub fn remove(&mut self, start: Pos, end: Pos) -> String {
        let removed = self.text(start, end);
        let joined = format!(
            "{}{}",
            &self.lines[start.line][..self.byte(start)],
            &self.lines[end.line][self.byte(end)..]
        );
        self.splice(start.line..end.line + 1, vec![joined]);
        removed
    }

    /// Inserts `s` (which may hold '\n') at `at`; returns the position right after it.
    pub fn insert(&mut self, at: Pos, s: &str) -> Pos {
        let (head, tail) = self.lines[at.line].split_at(self.byte(at));
        let mut parts: Vec<String> = s.split('\n').map(str::to_owned).collect();
        let last = parts.len() - 1;
        let end_col = if last == 0 { at.col } else { 0 } + text::char_len(&parts[last]);
        parts[0].insert_str(0, head);
        parts[last].push_str(tail);
        self.splice(at.line..at.line + 1, parts);
        Pos::new(at.line + last, end_col)
    }

    /// Highlight groups for line `y`, one per char; None for files without a known language.
    pub fn highlight_line(&mut self, y: usize) -> Option<Vec<Hl>> {
        Some(self.syntax.line(&self.lines, self.lang?, y))
    }

    fn byte(&self, p: Pos) -> usize {
        text::byte_index(&self.lines[p.line], p.col)
    }
}

/// Writes through a temp file + rename, so a crash or full disk never leaves a truncated file.
/// Follows symlinks and keeps permissions; falls back to an in-place write when the directory is read-only.
fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let Some(name) = target.file_name() else { return fs::write(&target, data) };
    let tmp = target.with_file_name(format!(".{}.lim-{}.tmp", name.to_string_lossy(), std::process::id()));
    let staged = File::create(&tmp).and_then(|mut f| {
        f.write_all(data)?;
        f.sync_all()?;
        if let Ok(meta) = fs::metadata(&target) {
            f.set_permissions(meta.permissions())?;
        }
        Ok(())
    });
    match staged.and_then(|()| fs::rename(&tmp, &target)) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = fs::remove_file(&tmp);
            fs::write(&target, data)
        }
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
        let mut b = Buffer::from_text(None, "héllo\nworld\n");
        let end = b.insert(Pos::new(0, 1), "X\nY");
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
        let path = std::env::temp_dir().join(format!("lim-test-{}.txt", std::process::id()));
        let mut b = Buffer::from_text(Some(path.clone()), "a\r\nb\r\n");
        b.insert(Pos::new(0, 1), "!");
        assert!(b.is_dirty());
        b.save(None, false).unwrap();
        assert!(!b.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "a!\r\nb\r\n");
        fs::remove_file(path).unwrap();
    }
}
