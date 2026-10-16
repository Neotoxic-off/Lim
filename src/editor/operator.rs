//! Operators (`d`, `c`, `y`, `>`, `<`) over a motion or a selection.

use std::ops::RangeInclusive;

use super::motion::Span;
use super::registers::Register;
use super::vim::Case;
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    Indent,
    Dedent,
    /// `gc`: toggle line comments.
    Comment,
    /// `gu`, `gU`, `g~`
    Lower,
    Upper,
    ToggleCase,
}

impl Operator {
    pub fn from_char(c: char) -> Option<Self> {
        Some(match c {
            'd' => Self::Delete,
            'c' => Self::Change,
            'y' => Self::Yank,
            '>' => Self::Indent,
            '<' => Self::Dedent,
            _ => return None,
        })
    }

    /// The key that, typed twice, applies the operator to whole lines (`dd`, `>>`).
    pub fn key(self) -> char {
        match self {
            Self::Delete => 'd',
            Self::Change => 'c',
            Self::Yank => 'y',
            Self::Indent => '>',
            Self::Dedent => '<',
            Self::Comment => 'c',
            Self::Lower => 'u',
            Self::Upper => 'U',
            Self::ToggleCase => '~',
        }
    }
}

impl Editor {
    pub(super) fn apply(&mut self, op: Operator, a: Pos, b: Pos, span: Span) {
        let (start, end): (Pos, Pos) = if a <= b { (a, b) } else { (b, a) };
        match (op, span) {
            (Operator::Indent | Operator::Dedent, _) => self.shift(op == Operator::Indent, start.line..=end.line),
            (Operator::Comment, _) => self.toggle_comment(start.line..=end.line),
            (Operator::Lower | Operator::Upper | Operator::ToggleCase, span) => {
                let case: Case = match op {
                    Operator::Lower => Case::Lower,
                    Operator::Upper => Case::Upper,
                    _ => Case::Toggle,
                };
                let end: Pos = if span == Span::Inclusive { Pos::new(end.line, end.col + 1) } else { end };
                self.change_case(case, start, end, span == Span::Linewise);
            }
            (_, Span::Linewise) => self.apply_lines(op, start.line, end.line),
            (_, span) => self.apply_chars(op, start, end, span == Span::Inclusive),
        }
    }

    fn apply_lines(&mut self, op: Operator, first: usize, last: usize) {
        let lines: String = self.buf().lines()[first..=last].join("\n");
        if matches!(op, Operator::Delete | Operator::Change | Operator::Yank) {
            self.store_register(Register { text: lines, linewise: true, block: false }, op == Operator::Yank);
        }
        let buf: &mut Buffer = &mut self.buffers[self.current];
        match op {
            Operator::Delete => {
                buf.splice(first..last + 1, Vec::new());
                let line: usize = first.min(buf.last_line());
                buf.cursor = Pos::new(line, buf.first_non_blank(line));
            }
            Operator::Change => {
                let indent: String = text::indent(buf.line(first)).to_owned();
                let col: usize = text::char_len(&indent);
                buf.splice(first..last + 1, vec![indent]);
                buf.cursor = Pos::new(first, col);
                self.auto_indent = (col > 0).then_some(first);
                self.mode = Mode::Insert;
            }
            _ => buf.cursor.line = first,
        }
    }

    fn apply_chars(&mut self, op: Operator, start: Pos, mut end: Pos, inclusive: bool) {
        let buf: &mut Buffer = &mut self.buffers[self.current];
        let start: Pos = Pos::new(start.line, start.col.min(buf.line_len(start.line)));
        end.col = (end.col + usize::from(inclusive)).min(buf.line_len(end.line));
        if start == end && op != Operator::Change {
            return;
        }
        let text: String = if op == Operator::Yank { buf.text(start, end) } else { buf.remove(start, end) };
        buf.cursor = start;
        if !text.is_empty() {
            self.store_register(Register { text, linewise: false, block: false }, op == Operator::Yank);
        }
        if op == Operator::Change {
            self.mode = Mode::Insert;
        }
    }

    /// Comments the lines out at their common indent, or uncomments them if all already are.
    fn toggle_comment(&mut self, lines: RangeInclusive<usize>) {
        let first: usize = *lines.start();
        let buf: &mut Buffer = self.buf_mut();
        let leader: &str = buf.lang.map_or("#", |l| l.comment);
        // shortcut: block-comment-only languages (HTML, CSS, Markdown) are left as they are.
        if leader.is_empty() {
            return;
        }
        let targets: Vec<usize> = lines.filter(|&y| !buf.line(y).trim().is_empty()).collect();
        let commented: bool = targets.iter().all(|&y| buf.line(y).trim_start().starts_with(leader));
        let indent: usize = targets.iter().map(|&y| text::indent(buf.line(y)).len()).min().unwrap_or(0);
        for y in targets {
            let line: &str = buf.line(y);
            let toggled: String = if commented {
                let (ind, rest): (&str, &str) = line.split_at(text::indent(line).len());
                let rest: &str = &rest[leader.len()..];
                format!("{ind}{}", rest.strip_prefix(' ').unwrap_or(rest))
            } else {
                // Indents may mix multi-byte blanks; never split a char.
                let at: usize = if line.is_char_boundary(indent) { indent } else { text::indent(line).len() };
                let (ind, rest): (&str, &str) = line.split_at(at);
                format!("{ind}{leader} {rest}")
            };
            buf.set_line(y, toggled);
        }
        buf.cursor.line = first;
    }

    /// `<C-t>` / `<C-d>` in insert mode: shift one line without moving to its first char.
    pub(super) fn shift_keep_cursor(&mut self, indent: bool, line: usize) {
        let cursor: Pos = self.buf().cursor;
        self.shift(indent, line..=line);
        self.buf_mut().cursor = cursor;
    }

    fn shift(&mut self, indent: bool, lines: RangeInclusive<usize>) {
        let first: usize = *lines.start();
        let width: usize = self.options.indent_width();
        let unit: String = if self.options.expandtab { " ".repeat(width) } else { "\t".to_owned() };
        let buf: &mut Buffer = self.buf_mut();
        for y in lines {
            let line: &str = buf.line(y);
            let shifted: String = if indent {
                if line.is_empty() {
                    continue;
                }
                format!("{unit}{line}")
            } else {
                let spaces: usize = line.bytes().take(width).take_while(|&b| b == b' ').count();
                let strip: usize = if spaces == 0 && line.starts_with('\t') { 1 } else { spaces };
                if strip == 0 {
                    continue;
                }
                line[strip..].to_owned()
            };
            buf.set_line(y, shifted);
        }
        buf.cursor = Pos::new(first, buf.first_non_blank(first));
    }
}
