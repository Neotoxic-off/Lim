//! `:s///c`: each match is shown and asks `y` (replace), `n` (skip), `a` (all the rest), `l` (this one, then
//! stop) or `q` / `<Esc>` (stop). The whole session is one undo step.

use super::vim::{LineRange, expand_replacement};
use super::{Editor, Mode};
use crate::buffer::regex::Match;
use crate::buffer::{Buffer, Pos, text};
use crate::term::Key;

/// Where a `:s///c` session stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirm {
    pattern: String,
    replacement: String,
    global: bool,
    /// The next match is searched from here.
    at: Pos,
    /// Last line of the range; it moves when a replacement adds or removes lines.
    last: usize,
    replaced: usize,
}

impl Editor {
    /// Starts asking about each match of `pattern` in `range`; false when there is none.
    pub(super) fn start_confirm(&mut self, range: LineRange, pattern: &str, replacement: &str, global: bool) -> bool {
        let session: Confirm = Confirm {
            pattern: pattern.to_owned(),
            replacement: replacement.to_owned(),
            global,
            at: Pos::new(range.first, 0),
            last: range.last.min(self.buf().last_line()),
            replaced: 0,
        };
        self.confirm = Some(session);
        if self.confirm_next().is_none() {
            self.confirm = None;
            return false;
        }
        pattern.clone_into(&mut self.search);
        self.highlight_search = true;
        let buf: &mut Buffer = self.buf_mut();
        buf.seal();
        buf.hold_undo(true);
        self.mode = Mode::Confirm;
        self.show_confirm();
        true
    }

    /// The next match at or after the session's position, with its line; the cursor goes to it.
    fn confirm_next(&mut self) -> Option<(usize, Match)> {
        let session: &Confirm = self.confirm.as_ref()?;
        let buf: &Buffer = &self.buffers[self.current];
        let found: Option<(usize, Match)> = (session.at.line..=session.last.min(buf.last_line())).find_map(|y| {
            let from: usize = if y == session.at.line { session.at.col } else { 0 };
            text::find_matches(buf.line(y), &session.pattern)
                .into_iter()
                .find(|m| m.range.start >= from)
                .map(|m| (y, m))
        });
        if let Some((y, m)) = &found {
            self.buf_mut().cursor = Pos::new(*y, m.range.start);
        }
        found
    }

    fn show_confirm(&mut self) {
        let replacement: String = self.confirm.as_ref().map(|s| s.replacement.clone()).unwrap_or_default();
        self.info(format!("replace with {replacement} (y/n/a/q/l)?"));
    }

    pub(super) fn confirm_key(&mut self, key: Key) {
        match key {
            Key::Char('y') => {
                self.confirm_step(true);
            }
            Key::Char('n') => {
                self.confirm_step(false);
            }
            Key::Char('l') => {
                self.confirm_step(true);
                self.finish_confirm();
            }
            Key::Char('a') => while self.confirm_step(true) {},
            Key::Char('q') | Key::Esc | Key::Ctrl('c') => self.finish_confirm(),
            _ => {}
        }
    }

    /// Replaces (or skips) the current match and moves to the next; false once there is none.
    fn confirm_step(&mut self, replace: bool) -> bool {
        let Some((y, m)): Option<(usize, Match)> = self.confirm_next() else {
            self.finish_confirm();
            return false;
        };
        let Some(session): Option<&Confirm> = self.confirm.as_ref() else { return false };
        let (replacement, global): (String, bool) = (session.replacement.clone(), session.global);
        let chars: Vec<char> = self.buf().line(y).chars().collect();
        // Where the search resumes, in the line as it is after this step.
        let (mut at, mut added): (Pos, isize) = (Pos::new(y, m.range.end.max(m.range.start + 1)), 0);
        if replace {
            let expanded: String = expand_replacement(&replacement, &chars, &m);
            let head: String = chars[..m.range.start].iter().collect();
            let tail: String = chars[m.range.end..].iter().collect();
            let parts: Vec<String> = format!("{head}{expanded}{tail}").split('\n').map(str::to_owned).collect();
            let rows: usize = parts.len();
            let last_col: usize = text::char_len(&parts[rows - 1]) - text::char_len(&tail);
            self.buf_mut().splice(y..y + 1, parts);
            added = isize::try_from(rows).unwrap_or(1) - 1;
            // An empty match moves on one char so the session always advances.
            let col: usize = if rows == 1 { m.range.start + text::char_len(&expanded) } else { last_col };
            at = Pos::new(y + rows - 1, if m.range.is_empty() { col + 1 } else { col });
        }
        let Some(session): Option<&mut Confirm> = self.confirm.as_mut() else { return false };
        session.replaced += usize::from(replace);
        session.last = session.last.saturating_add_signed(added);
        // Without `g`, one match per line: the next one is on a later line.
        session.at = if global { at } else { Pos::new(at.line + 1, 0) };
        if self.confirm_next().is_none() {
            self.finish_confirm();
            return false;
        }
        self.show_confirm();
        true
    }

    fn finish_confirm(&mut self) {
        let Some(session): Option<Confirm> = self.confirm.take() else { return };
        let buf: &mut Buffer = self.buf_mut();
        buf.hold_undo(false);
        buf.seal();
        buf.clamp(false);
        self.mode = Mode::Normal;
        let n: usize = session.replaced;
        self.info(format!("{n} substitution{}", if n == 1 { "" } else { "s" }));
    }
}
