//! Visual (`v`), visual-line (`V`) and visual-block (`<C-v>`) modes.

use super::motion::{Motion, Span};
use super::normal::VISUAL_PREFIXES;
use super::operator::Operator;
use super::vim::Case;
use super::{Editor, Mode, Pending};
use crate::buffer::Pos;
use crate::term::Key;

impl Editor {
    pub(super) fn visual(&mut self, key: Key) {
        if let Some(prefix) = self.pending.prefix.take() {
            if self.mode == Mode::VisualBlock
                && prefix == 'r'
                && let Key::Char(c) = key
            {
                return self.block_replace(c);
            }
            if prefix == 'r'
                && let Key::Char(c) = key
            {
                return self.replace_selection(c);
            }
            return self.prefixed(prefix, key);
        }
        if self.push_count(key) || self.push_prefix(key, VISUAL_PREFIXES) || self.repeat_find_key(key) {
            return;
        }
        if let Some(motion) = Motion::from_key(key) {
            return self.run_motion(motion);
        }
        let n = self.pending.count.unwrap_or(1);
        self.pending = Pending::default();
        let cursor = self.buf().cursor;
        let block = self.mode == Mode::VisualBlock;
        let span = if self.mode == Mode::VisualLine { Span::Linewise } else { Span::Inclusive };
        match key {
            Key::Esc | Key::Ctrl('c') => self.mode = Mode::Normal,
            Key::Char('v') => self.toggle_visual(Mode::Visual),
            Key::Char('V') => self.toggle_visual(Mode::VisualLine),
            Key::Ctrl('v') => self.toggle_visual(Mode::VisualBlock),
            Key::Char('o') => {
                let anchor = self.anchor;
                self.anchor = cursor;
                self.buf_mut().cursor = anchor;
            }
            Key::Char('O') if block => {
                // Other corner on the same line.
                let anchor = self.anchor;
                self.anchor = Pos::new(anchor.line, cursor.col);
                self.buf_mut().cursor = Pos::new(cursor.line, anchor.col);
            }
            Key::Char(':') => {
                let (first, last) = (self.anchor.line.min(cursor.line), self.anchor.line.max(cursor.line));
                let buf = self.buf_mut();
                buf.marks.insert('<', Pos::new(first, 0));
                buf.marks.insert('>', Pos::new(last, 0));
                self.mode = Mode::Normal;
                self.start_cmdline(Mode::Command);
                self.cmdline = "'<,'>".into();
            }
            Key::Tab | Key::BackTab => {
                // AstroNvim maps these to `>gv` / `<gv`: shift and keep the selection.
                let (anchor, mode) = (self.anchor, self.mode);
                self.buf_mut().seal();
                for _ in 0..n {
                    self.apply(if key == Key::Tab { Operator::Indent } else { Operator::Dedent }, anchor, cursor, Span::Linewise);
                }
                self.anchor = anchor;
                self.buf_mut().cursor = cursor;
                self.mode = mode;
            }
            Key::Char('I' | 'A') if block => {
                self.buf_mut().seal();
                self.block_insert_start(key == Key::Char('A'));
            }
            Key::Char('d' | 'x' | 'X' | 'D') if block => {
                self.buf_mut().seal();
                self.block_cut(true, false);
            }
            Key::Char('y' | 'Y') if block => self.block_cut(false, false),
            Key::Char('c' | 's') if block => {
                self.buf_mut().seal();
                self.block_cut(true, true);
            }
            Key::Char(c @ ('u' | 'U' | '~')) => {
                let case = match c {
                    'u' => Case::Lower,
                    'U' => Case::Upper,
                    _ => Case::Toggle,
                };
                self.buf_mut().seal();
                if block {
                    self.block_case(case);
                } else {
                    self.mode = Mode::Normal;
                    let (start, end) = if self.anchor <= cursor { (self.anchor, cursor) } else { (cursor, self.anchor) };
                    self.change_case(case, start, Pos::new(end.line, end.col + 1), span == Span::Linewise);
                }
            }
            Key::Char('J') => {
                let (first, last) = (self.anchor.line.min(cursor.line), self.anchor.line.max(cursor.line));
                self.mode = Mode::Normal;
                self.buf_mut().seal();
                self.buf_mut().cursor = Pos::new(first, 0);
                self.join(last - first + 1, true);
            }
            Key::Char('p' | 'P') => self.put_over_selection(span),
            Key::Char(c) => {
                let op = match c {
                    'x' | 'X' | 'D' => Some(Operator::Delete),
                    's' | 'S' | 'C' | 'R' => Some(Operator::Change),
                    'Y' => Some(Operator::Yank),
                    c => Operator::from_char(c),
                };
                if let Some(op) = op {
                    // Capital forms work on whole lines, as in Vim.
                    let span = if matches!(c, 'X' | 'D' | 'S' | 'C' | 'R' | 'Y') || block { Span::Linewise } else { span };
                    self.mode = Mode::Normal;
                    self.buf_mut().seal();
                    self.apply(op, self.anchor, cursor, span);
                }
            }
            _ => {}
        }
    }

    fn toggle_visual(&mut self, mode: Mode) {
        self.mode = if self.mode == mode { Mode::Normal } else { mode };
    }

    /// `r{c}`: every selected character becomes `c`.
    fn replace_selection(&mut self, ch: char) {
        let cursor = self.buf().cursor;
        let linewise = self.mode == Mode::VisualLine;
        let (start, end) = if self.anchor <= cursor { (self.anchor, cursor) } else { (cursor, self.anchor) };
        self.mode = Mode::Normal;
        let buf = self.buf_mut();
        buf.seal();
        for y in start.line..=end.line {
            let len = buf.line_len(y);
            let from = if linewise || y > start.line { 0 } else { start.col.min(len) };
            let to = if linewise || y < end.line { len } else { (end.col + 1).min(len) };
            if from < to {
                buf.remove(Pos::new(y, from), Pos::new(y, to));
                buf.insert(Pos::new(y, from), &ch.to_string().repeat(to - from));
            }
        }
        buf.cursor = start;
    }

    /// Visual `p`: the selection is replaced by the register (and goes to the unnamed register).
    fn put_over_selection(&mut self, span: Span) {
        let Some(reg) = self.fetch_register() else { return };
        let cursor = self.buf().cursor;
        self.mode = Mode::Normal;
        self.buf_mut().seal();
        self.apply(Operator::Delete, self.anchor, cursor, span);
        let buf = self.buf_mut();
        let at = buf.cursor;
        if reg.linewise {
            let line = if span == Span::Linewise { at.line } else { at.line + 1 };
            let lines: Vec<String> = reg.text.split('\n').map(str::to_owned).collect();
            if span != Span::Linewise {
                buf.insert(Pos::new(at.line, at.col), "\n");
            }
            buf.splice(line..line, lines);
            buf.cursor = Pos::new(line, buf.first_non_blank(line));
        } else {
            let end = buf.insert(at, &reg.text);
            buf.cursor = Pos::new(end.line, end.col.saturating_sub(1));
        }
    }
}
