//! Vim motions: key → typed motion → target position.

use crate::buffer::{Buffer, Pos};
use crate::term::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    FirstNonBlank,
    LineEnd,
    WordForward,
    WordBack,
    WordEnd,
    ParagraphForward,
    ParagraphBack,
    FileStart,
    FileEnd,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    MatchPair,
    Find { ch: char, forward: bool, till: bool },
    SearchNext { forward: bool },
}

/// How an operator treats the text up to the motion target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Exclusive,
    Inclusive,
    Linewise,
}

impl Motion {
    /// Single-key motions; `gg` and `f{c}` come through prefixes.
    pub fn from_key(key: Key) -> Option<Self> {
        Some(match key {
            Key::Char('h') | Key::Left | Key::Backspace => Self::Left,
            Key::Char('l') | Key::Right => Self::Right,
            Key::Char('j') | Key::Down | Key::Ctrl('n') => Self::Down,
            Key::Char('k') | Key::Up | Key::Ctrl('p') => Self::Up,
            Key::Char('0') | Key::Home => Self::LineStart,
            Key::Char('^') => Self::FirstNonBlank,
            Key::Char('$') | Key::End => Self::LineEnd,
            Key::Char('w') => Self::WordForward,
            Key::Char('b') => Self::WordBack,
            Key::Char('e') => Self::WordEnd,
            Key::Char('}') => Self::ParagraphForward,
            Key::Char('{') => Self::ParagraphBack,
            Key::Char('G') => Self::FileEnd,
            Key::Char('%') => Self::MatchPair,
            Key::Ctrl('d') => Self::HalfPageDown,
            Key::Ctrl('u') => Self::HalfPageUp,
            Key::Ctrl('f') | Key::PageDown => Self::PageDown,
            Key::Ctrl('b') | Key::PageUp => Self::PageUp,
            Key::Char('n') => Self::SearchNext { forward: true },
            Key::Char('N') => Self::SearchNext { forward: false },
            _ => return None,
        })
    }

    pub fn is_vertical(self) -> bool {
        matches!(
            self,
            Self::Up | Self::Down | Self::HalfPageDown | Self::HalfPageUp | Self::PageDown | Self::PageUp
        )
    }

    pub fn span(self) -> Span {
        match self {
            Self::FileStart | Self::FileEnd => Span::Linewise,
            _ if self.is_vertical() => Span::Linewise,
            Self::LineEnd | Self::WordEnd | Self::MatchPair | Self::Find { forward: true, .. } => Span::Inclusive,
            _ => Span::Exclusive,
        }
    }

    /// Where `count` repetitions land; None when the motion fails (no match).
    pub fn target(self, buf: &Buffer, count: Option<usize>, view_height: usize, pattern: &str) -> Option<Pos> {
        let n = count.unwrap_or(1);
        let Pos { line, col } = buf.cursor;
        let last = buf.last_line();
        let vertical = |l: usize| Pos::new(l, buf.want_col);
        let repeat = |step: &dyn Fn(Pos) -> Pos| (0..n).fold(buf.cursor, |p, _| step(p));
        Some(match self {
            Self::Left => Pos::new(line, col.saturating_sub(n)),
            Self::Right => Pos::new(line, (col + n).min(buf.line_len(line))),
            Self::Down => vertical((line + n).min(last)),
            Self::Up => vertical(line.saturating_sub(n)),
            Self::HalfPageDown => vertical((line + view_height / 2).min(last)),
            Self::HalfPageUp => vertical(line.saturating_sub(view_height / 2)),
            Self::PageDown => vertical((line + view_height).min(last)),
            Self::PageUp => vertical(line.saturating_sub(view_height)),
            Self::LineStart => Pos::new(line, 0),
            Self::FirstNonBlank => Pos::new(line, buf.first_non_blank(line)),
            Self::LineEnd => {
                let l = (line + n - 1).min(last);
                Pos::new(l, buf.line_len(l).saturating_sub(1))
            }
            Self::WordForward => repeat(&|p| buf.word_forward(p)),
            Self::WordBack => repeat(&|p| buf.word_back(p)),
            Self::WordEnd => repeat(&|p| buf.word_end(p)),
            Self::ParagraphForward => Pos::new((0..n).fold(line, |l, _| buf.paragraph(l, true)), 0),
            Self::ParagraphBack => Pos::new((0..n).fold(line, |l, _| buf.paragraph(l, false)), 0),
            Self::FileStart | Self::FileEnd => {
                let default = if self == Self::FileStart { 0 } else { last };
                let l = count.map_or(default, |c| c.saturating_sub(1)).min(last);
                Pos::new(l, buf.first_non_blank(l))
            }
            Self::Find { ch, forward, till } => {
                let i = buf.find_char(buf.cursor, ch, forward, n)?;
                Pos::new(line, if !till { i } else if forward { i - 1 } else { i + 1 })
            }
            Self::MatchPair => buf.matching_bracket(buf.cursor)?,
            Self::SearchNext { forward } => buf.search(pattern, buf.cursor, forward)?,
        })
    }
}
