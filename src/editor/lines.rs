//! Ex commands over whole lines: `:sort`, `:m` (move), `:t` / `:co` (copy), `:r` (read a file in).

use std::fs;
use std::io;
use std::path::Path;

use super::Editor;
use super::vim::LineRange;
use crate::buffer::{Buffer, Pos, text};
use crate::error::Error;

/// What `:sort` compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Text,
    /// `:sort i`
    IgnoreCase,
    /// `:sort n`: the first (signed) number on the line; lines without one come first.
    Numeric,
}

fn first_number(line: &str) -> Option<i64> {
    text::find_all(line, "-\\=\\d\\+")
        .first()
        .and_then(|r| line.chars().skip(r.start).take(r.len()).collect::<String>().parse().ok())
}

impl Editor {
    /// `:[range]sort[!] [i] [n] [u]`: stable sort, `!` reverses, `u` keeps the first of equal lines.
    pub(super) fn sort_lines(&mut self, range: LineRange, key: SortKey, reverse: bool, unique: bool) {
        let buf: &mut Buffer = self.buf_mut();
        let last: usize = range.last.min(buf.last_line());
        let mut lines: Vec<String> = buf.lines()[range.first..=last].to_vec();
        match key {
            SortKey::Text => lines.sort(),
            SortKey::IgnoreCase => lines.sort_by_key(|l| l.to_lowercase()),
            SortKey::Numeric => lines.sort_by_key(|l| first_number(l)),
        }
        if reverse {
            lines.reverse();
        }
        if unique {
            lines.dedup_by(|a, b| match key {
                SortKey::IgnoreCase => a.to_lowercase() == b.to_lowercase(),
                SortKey::Numeric => first_number(a) == first_number(b),
                SortKey::Text => a == b,
            });
        }
        if buf.lines()[range.first..=last] != lines[..] {
            buf.seal();
            buf.splice(range.first..last + 1, lines);
        }
        buf.cursor = Pos::new(range.first, 0);
        buf.clamp(false);
    }

    /// `:[range]m {address}` (`copy` false) or `:[range]t {address}`: the lines go below `address`; `0` is above
    /// the first line.
    pub(super) fn move_lines(&mut self, range: LineRange, address: &str, copy: bool) -> Result<(), Error> {
        let below: Option<usize> = if address.trim() == "0" {
            None
        } else {
            Some(self.parse_range(address.trim()).ok_or_else(|| Error::Vim("E14: Invalid address".into()))?.last)
        };
        let buf: &mut Buffer = self.buf_mut();
        let last: usize = range.last.min(buf.last_line());
        let moved: Vec<String> = buf.lines()[range.first..=last].to_vec();
        let count: usize = moved.len();
        let at: usize = below.map_or(0, |b| b + 1);
        if !copy && (range.first + 1..=last).contains(&at) {
            return Err(Error::Vim("E134: Cannot move a range of lines into itself".into()));
        }
        buf.seal();
        buf.splice(at..at, moved);
        let first_new: usize = at;
        if !copy {
            // The originals shifted down if the copy went above them.
            let from: usize = if at <= range.first { range.first + count } else { range.first };
            buf.splice(from..from + count, Vec::new());
        }
        let landed: usize = if !copy && at > range.first { first_new - count } else { first_new };
        let line: usize = landed + count - 1;
        buf.cursor = Pos::new(line, buf.first_non_blank(line));
        Ok(())
    }

    /// `:[line]r file`: the file's lines below `line` (the cursor's by default).
    pub(super) fn read_file(&mut self, below: usize, path: &Path) -> Result<(), Error> {
        let content: String = match fs::read(path) {
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| Error::NotUtf8(path.into()))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::Vim(format!("E484: Can't open file {}", path.display())));
            }
            Err(e) => return Err(Error::Io(path.into(), e)),
        };
        let lines: Vec<String> = content.lines().map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned()).collect();
        if lines.is_empty() {
            return Ok(());
        }
        let buf: &mut Buffer = self.buf_mut();
        let at: usize = below.min(buf.last_line()) + 1;
        buf.seal();
        buf.splice(at..at, lines);
        buf.cursor = Pos::new(at, buf.first_non_blank(at));
        Ok(())
    }
}
