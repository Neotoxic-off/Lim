//! More of Vim: replace mode (`R`), case operators, `<C-a>`/`<C-x>`, marks and the jump list,
//! scrolling (`zz`, `<C-e>`…), `:s`, and reloading files changed outside lim.

use super::{Editor, Message, Mode};
use crate::buffer::{Pos, text};
use crate::term::Key;

/// Jump list length, as Vim's.
const MAX_JUMPS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Lower,
    Upper,
    Toggle,
}

pub fn convert(s: &str, case: Case) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match case {
            Case::Lower => out.extend(c.to_lowercase()),
            Case::Upper => out.extend(c.to_uppercase()),
            Case::Toggle if c.is_uppercase() => out.extend(c.to_lowercase()),
            Case::Toggle => out.extend(c.to_uppercase()),
        }
    }
    out
}

/// A line range for ex commands: `%`, `.`, `$`, `N`, `N,M`, `'<,'>`, `'a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRange {
    pub first: usize,
    pub last: usize,
}

impl Editor {
    pub(super) fn start_replace(&mut self) {
        let line = self.buf().cursor.line;
        self.replace_original = self.buf().line(line).to_owned();
        self.mode = Mode::Replace;
    }

    pub(super) fn replace_key(&mut self, key: Key) {
        let buf = &mut self.buffers[self.current];
        let here = buf.cursor;
        let len = buf.line_len(here.line);
        match key {
            Key::Esc | Key::Ctrl('c') => {
                buf.cursor.col = here.col.saturating_sub(1);
                self.mode = Mode::Normal;
            }
            Key::Char(c) => {
                let end = Pos::new(here.line, (here.col + 1).min(len));
                buf.remove(here, end);
                buf.insert(here, &c.to_string());
                buf.cursor.col += 1;
            }
            Key::Backspace | Key::Ctrl('h') if here.col > 0 => {
                // Backspace restores what was overwritten.
                let col = here.col - 1;
                let original = self.replace_original.chars().nth(col);
                buf.remove(Pos::new(here.line, col), Pos::new(here.line, here.col));
                if let Some(c) = original {
                    buf.insert(Pos::new(here.line, col), &c.to_string());
                }
                buf.cursor.col = col;
            }
            Key::Enter => {
                buf.insert(here, "\n");
                buf.cursor = Pos::new(here.line + 1, 0);
                self.replace_original = buf.line(here.line + 1).to_owned();
            }
            Key::Left => buf.cursor.col = here.col.saturating_sub(1),
            Key::Right => buf.cursor.col = (here.col + 1).min(len),
            _ => {}
        }
    }

    /// `gu`, `gU`, `g~` and visual `u`, `U`, `~` over a char range or whole lines.
    pub(super) fn change_case(&mut self, case: Case, start: Pos, end: Pos, linewise: bool) {
        let buf = self.buf_mut();
        if linewise {
            for y in start.line..=end.line.min(buf.last_line()) {
                let converted = convert(buf.line(y), case);
                if converted != buf.line(y) {
                    buf.set_line(y, converted);
                }
            }
            buf.cursor = Pos::new(start.line, buf.cursor.col);
            return;
        }
        let end = Pos::new(end.line, end.col.min(buf.line_len(end.line)));
        let original = buf.text(start, end);
        let converted = convert(&original, case);
        if converted != original {
            buf.remove(start, end);
            buf.insert(start, &converted);
        }
        buf.cursor = start;
    }

    /// `<C-a>` / `<C-x>` on the number at or after the cursor.
    pub(super) fn increment(&mut self, delta: i64) {
        let buf = self.buf_mut();
        let here = buf.cursor;
        let Some((start, end, value)) = buf.number_at(here) else { return };
        let new = value.saturating_add(delta).to_string();
        buf.remove(Pos::new(here.line, start), Pos::new(here.line, end));
        buf.insert(Pos::new(here.line, start), &new);
        buf.cursor = Pos::new(here.line, start + text::char_len(&new) - 1);
    }

    pub(super) fn set_mark(&mut self, name: char) {
        if name.is_ascii_lowercase() || name == '\'' {
            let at = self.buf().cursor;
            self.buf_mut().marks.insert(name, at);
        }
    }

    /// Records the cursor before a jump: `''` and `<C-o>` come back here.
    pub(super) fn push_jump(&mut self) {
        let at = self.buf().cursor;
        let path = self.buf().path.clone();
        self.buf_mut().marks.insert('\'', at);
        let same = self.jumps.last().is_some_and(|(p, id, _)| *p == path && self.buf().anchor(*id) == Some(at));
        if same {
            return;
        }
        // An anchor in the buffer keeps the entry on the same text while lines come and go.
        self.next_anchor += 1;
        let id = self.next_anchor;
        self.buf_mut().add_anchor(id, at);
        self.jumps.push((path, id, at));
        if self.jumps.len() > MAX_JUMPS {
            let (old_path, old_id, _) = self.jumps.remove(0);
            if let Some(buf) = self.buffers.iter_mut().find(|b| b.path == old_path) {
                buf.remove_anchor(old_id);
            }
        }
    }

    /// `<C-o>`
    pub(super) fn jump_back(&mut self, count: usize) {
        for _ in 0..count {
            let Some((path, id, fallback)) = self.jumps.pop() else { return self.info("Jump list is empty") };
            if path != self.buf().path
                && let Some(path) = &path
            {
                self.open(path);
            }
            let buf = self.buf_mut();
            let at = buf.anchor(id).unwrap_or(fallback);
            buf.remove_anchor(id);
            buf.cursor = at;
            buf.clamp(false);
        }
    }

    /// `zt`, `zz`, `zb`: put the cursor line at the top, middle or bottom of the window.
    pub(super) fn scroll_cursor_to(&mut self, place: char) {
        let height = self.view_height.max(1);
        let buf = self.buf_mut();
        let line = buf.cursor.line;
        buf.top = match place {
            't' => line,
            'b' => (line + 1).saturating_sub(height),
            _ => line.saturating_sub(height / 2),
        };
    }

    /// `<C-e>` / `<C-y>`: scroll the view, dragging the cursor only when it would leave it.
    pub(super) fn scroll_view(&mut self, delta: isize, count: usize) {
        let height = self.view_height.max(1);
        let buf = self.buf_mut();
        let step = count as isize * delta;
        buf.top = buf.top.saturating_add_signed(step).min(buf.last_line());
        if buf.cursor.line < buf.top {
            buf.cursor.line = buf.top;
        } else if buf.cursor.line >= buf.top + height {
            buf.cursor.line = buf.top + height - 1;
        }
        buf.clamp(false);
    }

    /// `:[range]s/pattern/replacement/[g]`: literal pattern (with `\<`/`\>`), `&` in the
    /// replacement stands for the match. Returns the number of lines changed.
    // shortcut: patterns are literal, not Vim regexes; add a small regex engine if users need them.
    pub(super) fn substitute(&mut self, range: LineRange, pattern: &str, replacement: &str, global: bool) -> usize {
        self.last_substitute = Some((pattern.to_owned(), replacement.to_owned(), global));
        let buf = self.buf_mut();
        let mut changed = 0;
        // Bottom-up, since a `\n` in the replacement splits a line and shifts the ones below.
        for y in (range.first..=range.last.min(buf.last_line())).rev() {
            let line = buf.line(y).to_owned();
            let mut hits = text::find_all(&line, pattern);
            if !global {
                hits.truncate(1);
            }
            if hits.is_empty() {
                continue;
            }
            let chars: Vec<char> = line.chars().collect();
            let mut out = String::with_capacity(line.len());
            let mut col = 0;
            for hit in hits {
                out.extend(&chars[col..hit.start]);
                let matched: String = chars[hit.clone()].iter().collect();
                out.push_str(&expand_replacement(replacement, &matched));
                col = hit.end;
            }
            out.extend(&chars[col..]);
            buf.splice(y..y + 1, out.split('\n').map(str::to_owned).collect());
            buf.cursor = Pos::new(y, buf.first_non_blank(y));
            changed += 1;
        }
        changed
    }

    /// `&`: the last `:s` again on the current line.
    pub(super) fn repeat_substitute(&mut self) {
        let Some((pattern, replacement, global)) = self.last_substitute.clone() else {
            return self.set_message(Message::Error("E35: No previous regular expression".into()));
        };
        let line = self.buf().cursor.line;
        self.substitute(LineRange { first: line, last: line }, &pattern, &replacement, global);
    }

    /// After a shell or lazygit: unmodified buffers whose file changed on disk are reloaded.
    pub fn reload_changed_buffers(&mut self) {
        for buf in &mut self.buffers {
            buf.reload_if_changed();
        }
    }

    /// Parses `%`, `.`, `$`, `N`, `'x` and pairs of them joined by `,`.
    pub(super) fn parse_range(&self, spec: &str) -> Option<LineRange> {
        let buf = self.buf();
        let last = buf.last_line();
        let line = |s: &str| -> Option<usize> {
            match s {
                "" | "." => Some(buf.cursor.line),
                "$" => Some(last),
                s if s.starts_with('\'') => {
                    let mark = s.chars().nth(1)?;
                    buf.marks.get(&mark).map(|p| p.line)
                }
                s => s.parse::<usize>().ok().map(|n| n.saturating_sub(1).min(last)),
            }
        };
        if spec == "%" {
            return Some(LineRange { first: 0, last });
        }
        let (a, b) = spec.split_once(',').unwrap_or((spec, spec));
        let (first, second) = (line(a)?, line(b)?);
        Some(LineRange { first: first.min(second), last: first.max(second) })
    }
}

fn expand_replacement(replacement: &str, matched: &str) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            '&' => out.push_str(matched),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_and_replacement() {
        assert_eq!(convert("aBc", Case::Toggle), "AbC");
        assert_eq!(convert("straße", Case::Upper), "STRASSE");
        assert_eq!(expand_replacement("<&>\\&", "x"), "<x>&");
    }
}
