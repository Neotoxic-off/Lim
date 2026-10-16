//! Text-object primitives behind Vim motions: words, paragraphs, char finds, search.

use std::{iter::Skip, str::Chars};

use super::{Buffer, Pos, text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharClass {
    Blank,
    Word,
    Punct,
}

impl CharClass {
    /// For WORD motions (`W`, `B`, `E`): anything that is not blank is one class.
    pub fn of_big(c: Option<char>) -> Self {
        match Self::of(c) {
            Self::Blank => Self::Blank,
            _ => Self::Word,
        }
    }

    pub fn of(c: Option<char>) -> Self {
        match c {
            None => Self::Blank,
            Some(c) if c.is_whitespace() => Self::Blank,
            Some(c) if c.is_alphanumeric() || c == '_' => Self::Word,
            Some(_) => Self::Punct,
        }
    }
}

enum Step {
    Same,
    NewLine,
    End,
}

/// Walks char by char across lines, decoding each line once.
struct Walker<'a> {
    big: bool,
    lines: &'a [String],
    line: usize,
    chars: Vec<char>,
    col: usize,
}

impl<'a> Walker<'a> {
    fn new(buf: &'a Buffer, at: Pos) -> Self {
        Self { big: false, lines: &buf.lines, line: at.line, chars: buf.lines[at.line].chars().collect(), col: at.col }
    }

    fn big(buf: &'a Buffer, at: Pos, big: bool) -> Self {
        Self { big, ..Self::new(buf, at) }
    }

    fn class(&self) -> CharClass {
        let c: Option<char> = self.chars.get(self.col).copied();
        if self.big { CharClass::of_big(c) } else { CharClass::of(c) }
    }

    fn blank_line(&self) -> bool {
        self.chars.is_empty()
    }

    fn pos(&self) -> Pos {
        Pos::new(self.line, self.col)
    }

    fn line_end(&self) -> Pos {
        Pos::new(self.line, self.chars.len())
    }

    fn load(&mut self, line: usize) {
        self.line = line;
        self.chars = self.lines[line].chars().collect();
    }

    fn next(&mut self) -> Step {
        if self.col + 1 < self.chars.len() {
            self.col += 1;
            Step::Same
        } else if self.line + 1 < self.lines.len() {
            self.load(self.line + 1);
            self.col = 0;
            Step::NewLine
        } else {
            Step::End
        }
    }

    fn prev(&mut self) -> Step {
        if self.col > 0 {
            self.col -= 1;
            Step::Same
        } else if self.line > 0 {
            self.load(self.line - 1);
            self.col = self.chars.len().saturating_sub(1);
            Step::NewLine
        } else {
            Step::End
        }
    }
}

impl Buffer {
    pub fn class_at(&self, p: Pos) -> CharClass {
        CharClass::of(self.lines[p.line].chars().nth(p.col))
    }

    /// True when the next char on the line starts another class (or there is none).
    pub fn at_word_end(&self, p: Pos) -> bool {
        let mut chars: Skip<Chars<'_>> = self.lines[p.line].chars().skip(p.col);
        let here: CharClass = CharClass::of(chars.next());
        chars.next().is_none_or(|c| CharClass::of(Some(c)) != here)
    }

    /// `w` / `W`: start of the next word; empty lines count as words.
    pub fn word_forward(&self, at: Pos, big: bool) -> Pos {
        let mut w: Walker<'_> = Walker::big(self, at, big);
        let start: CharClass = w.class();
        loop {
            match w.next() {
                Step::End => return w.line_end(),
                Step::NewLine => break,
                Step::Same if w.class() != start => break,
                Step::Same => {}
            }
        }
        while w.class() == CharClass::Blank && !w.blank_line() {
            if let Step::End = w.next() {
                return w.line_end();
            }
        }
        w.pos()
    }

    /// `e` / `E`: end of the current or next word.
    pub fn word_end(&self, at: Pos, big: bool) -> Pos {
        let mut w: Walker<'_> = Walker::big(self, at, big);
        if let Step::End = w.next() {
            return at;
        }
        while w.class() == CharClass::Blank {
            if let Step::End = w.next() {
                return w.pos();
            }
        }
        let class: CharClass = w.class();
        loop {
            let here: Pos = w.pos();
            match w.next() {
                Step::Same if w.class() == class => {}
                _ => return here,
            }
        }
    }

    /// `b` / `B`: start of the current or previous word.
    pub fn word_back(&self, at: Pos, big: bool) -> Pos {
        let mut w: Walker<'_> = Walker::big(self, at, big);
        if let Step::End = w.prev() {
            return at;
        }
        while w.class() == CharClass::Blank && !w.blank_line() {
            if let Step::End = w.prev() {
                return w.pos();
            }
        }
        let class: CharClass = w.class();
        loop {
            let here: Pos = w.pos();
            match w.prev() {
                Step::Same if w.class() == class => {}
                _ => return here,
            }
        }
    }

    /// `ge` / `gE`: end of the previous word.
    pub fn word_end_back(&self, at: Pos, big: bool) -> Pos {
        let mut w: Walker<'_> = Walker::big(self, at, big);
        let start: CharClass = w.class();
        // Leave the word under the cursor, then skip blanks back to the previous word's last char.
        loop {
            match w.prev() {
                Step::End => return at,
                Step::NewLine if w.blank_line() => return w.pos(),
                Step::NewLine => break,
                Step::Same if w.class() != start => break,
                Step::Same => {}
            }
        }
        while w.class() == CharClass::Blank && !w.blank_line() {
            if let Step::End = w.prev() {
                return w.pos();
            }
        }
        w.pos()
    }

    /// `<C-a>` / `<C-x>`: adds `delta` to the number at or after the cursor; returns its last column.
    pub fn number_at(&self, at: Pos) -> Option<(usize, usize, i64)> {
        let chars: Vec<char> = self.lines[at.line].chars().collect();
        let mut start: usize = (at.col..chars.len()).find(|&i| chars[i].is_ascii_digit())?;
        // The cursor may sit inside the number: walk back to its first digit.
        while start > 0 && chars[start - 1].is_ascii_digit() {
            start -= 1;
        }
        let mut end: usize = start;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
        let negative: bool = start > 0 && chars[start - 1] == '-';
        let digits: String = chars[start..end].iter().collect();
        let value: i64 = digits.parse().ok()?;
        let start: usize = if negative { start - 1 } else { start };
        Some((start, end, if negative { -value } else { value }))
    }

    /// `}` / `{`: the next empty line past the current paragraph.
    pub fn paragraph(&self, mut line: usize, forward: bool) -> usize {
        let step = |l: usize| if forward { (l < self.last_line()).then_some(l + 1) } else { l.checked_sub(1) };
        while self.lines[line].is_empty() {
            match step(line) {
                Some(l) => line = l,
                None => return line,
            }
        }
        while let Some(l) = step(line) {
            line = l;
            if self.lines[line].is_empty() {
                break;
            }
        }
        line
    }

    /// `%`: the partner of the first bracket at or after `at` on its line.
    pub fn matching_bracket(&self, at: Pos) -> Option<Pos> {
        let (col, c): (usize, char) =
            self.lines[at.line].chars().enumerate().skip(at.col).find(|(_, c)| "()[]{}".contains(*c))?;
        self.bracket_partner(Pos::new(at.line, col), c, usize::MAX)
    }

    /// Partner of bracket `c` at `at`, looking at most `max_lines` lines away.
    pub fn bracket_partner(&self, at: Pos, c: char, max_lines: usize) -> Option<Pos> {
        let (me, other, forward): (char, char, bool) = match c {
            '(' => ('(', ')', true),
            '[' => ('[', ']', true),
            '{' => ('{', '}', true),
            ')' => (')', '(', false),
            ']' => (']', '[', false),
            '}' => ('}', '{', false),
            _ => return None,
        };
        let mut w: Walker<'_> = Walker::new(self, at);
        let mut depth: usize = 0usize;
        loop {
            match w.chars.get(w.col).copied() {
                Some(ch) if ch == me => depth += 1,
                Some(ch) if ch == other => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(w.pos());
                    }
                }
                _ => {}
            }
            let step: Step = if forward { w.next() } else { w.prev() };
            if matches!(step, Step::End) || w.line.abs_diff(at.line) > max_lines {
                return None;
            }
        }
    }

    /// `iw` / `aw`: the run of same-class chars under `at`, plus trailing (or leading) blanks for `aw`.
    pub fn word_object(&self, at: Pos, around: bool) -> Option<(Pos, Pos)> {
        let chars: Vec<char> = self.lines[at.line].chars().collect();
        let col: usize = at.col.min(chars.len().checked_sub(1)?);
        let class = |i: usize| CharClass::of(chars.get(i).copied());
        let here: CharClass = class(col);
        let mut start: usize = col;
        while start > 0 && class(start - 1) == here {
            start -= 1;
        }
        let mut end: usize = col + 1;
        while end < chars.len() && class(end) == here {
            end += 1;
        }
        if around && here != CharClass::Blank {
            let word_end: usize = end;
            while end < chars.len() && class(end) == CharClass::Blank {
                end += 1;
            }
            if end == word_end {
                while start > 0 && class(start - 1) == CharClass::Blank {
                    start -= 1;
                }
            }
        }
        Some((Pos::new(at.line, start), Pos::new(at.line, end)))
    }

    /// `i(` / `a(` and friends: inside (or including) the brackets enclosing `at`.
    pub fn pair_object(&self, at: Pos, open: char, close: char, around: bool) -> Option<(Pos, Pos)> {
        let mut w: Walker<'_> = Walker::new(self, at);
        let mut depth: usize = 0usize;
        let start: Pos = loop {
            match w.chars.get(w.col).copied() {
                Some(c) if c == open && depth == 0 => break w.pos(),
                Some(c) if c == open => depth -= 1,
                Some(c) if c == close && w.pos() != at => depth += 1,
                _ => {}
            }
            if let Step::End = w.prev() {
                return None;
            }
        };
        let end: Pos = self.bracket_partner(start, open, usize::MAX)?;
        Some(if around { (start, Pos::new(end.line, end.col + 1)) } else { (Pos::new(start.line, start.col + 1), end) })
    }

    /// `i"` / `a"`: the quoted string around `at`, or the next one on the line.
    pub fn quote_object(&self, at: Pos, quote: char, around: bool) -> Option<(Pos, Pos)> {
        let mut quotes: Vec<usize> = Vec::new();
        let mut escaped: bool = false;
        for (i, c) in self.lines[at.line].chars().enumerate() {
            if c == quote && !escaped {
                quotes.push(i);
            }
            escaped = c == '\\' && !escaped;
        }
        let pairs: Vec<(usize, usize)> = quotes.chunks_exact(2).map(|p| (p[0], p[1])).collect();
        let (s, e): (usize, usize) = pairs
            .iter()
            .copied()
            .find(|&(s, e)| s <= at.col && at.col <= e)
            .or_else(|| pairs.iter().copied().find(|&(s, _)| s > at.col))?;
        Some(if around {
            (Pos::new(at.line, s), Pos::new(at.line, e + 1))
        } else {
            (Pos::new(at.line, s + 1), Pos::new(at.line, e))
        })
    }

    /// `f`/`F`: column of the `count`-th `ch` after (or before) `at` on its line.
    pub fn find_char(&self, at: Pos, ch: char, forward: bool, count: usize) -> Option<usize> {
        let line: &String = &self.lines[at.line];
        let mut hits = line.chars().enumerate().filter(|&(_, c)| c == ch).map(|(i, _)| i);
        if forward {
            hits.filter(|&i| i > at.col).nth(count.max(1) - 1)
        } else {
            let before: Vec<usize> = hits.by_ref().take_while(|&i| i < at.col).collect();
            before.into_iter().rev().nth(count.max(1) - 1)
        }
    }

    /// Next match of `pattern` after (or before) `from`, wrapping around the buffer.
    pub fn search(&self, pattern: &str, from: Pos, forward: bool) -> Option<Pos> {
        if pattern.is_empty() {
            return None;
        }
        let n: usize = self.lines.len();
        for i in 0..=n {
            let line: usize = if forward { (from.line + i) % n } else { (from.line + 2 * n - i) % n };
            let mut starts = text::find_all(&self.lines[line], pattern).into_iter().map(|r| r.start);
            let wrapped: bool = i > 0;
            let hit: Option<usize> = if forward {
                starts.find(|&c| wrapped || c > from.col)
            } else {
                starts.rev().find(|&c| wrapped || c < from.col)
            };
            if let Some(col) = hit {
                return Some(Pos::new(line, col));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_motions() {
        let b: Buffer = Buffer::from_text(None, "foo.bar  baz\n\nqux");
        let w = |p| b.word_forward(p, false);
        assert_eq!(w(Pos::new(0, 0)), Pos::new(0, 3));
        assert_eq!(w(Pos::new(0, 3)), Pos::new(0, 4));
        assert_eq!(w(Pos::new(0, 4)), Pos::new(0, 9));
        assert_eq!(w(Pos::new(0, 9)), Pos::new(1, 0));
        assert_eq!(w(Pos::new(1, 0)), Pos::new(2, 0));
        assert_eq!(b.word_back(Pos::new(2, 0), false), Pos::new(1, 0));
        assert_eq!(b.word_back(Pos::new(0, 9), false), Pos::new(0, 4));
        assert_eq!(b.word_end(Pos::new(0, 0), false), Pos::new(0, 2));
        assert_eq!(b.word_forward(Pos::new(0, 0), true), Pos::new(0, 9));
        assert_eq!(b.word_end(Pos::new(0, 0), true), Pos::new(0, 6));
        assert_eq!(b.word_end_back(Pos::new(0, 9), false), Pos::new(0, 6));
        assert_eq!(b.word_end_back(Pos::new(0, 9), true), Pos::new(0, 6));
        let n: Buffer = Buffer::from_text(None, "x = -12, y");
        assert_eq!(n.number_at(Pos::new(0, 0)), Some((4, 7, -12)));
        assert_eq!(n.number_at(Pos::new(0, 6)), Some((4, 7, -12)));
        assert_eq!(b.paragraph(0, true), 1);
    }

    #[test]
    fn brackets_and_objects() {
        let b: Buffer = Buffer::from_text(None, "f(a, [b]) {\n  x\n}\nsay \"hi there\" now");
        assert_eq!(b.matching_bracket(Pos::new(0, 0)), Some(Pos::new(0, 8)));
        assert_eq!(b.matching_bracket(Pos::new(0, 9)), Some(Pos::new(2, 0)));
        assert_eq!(b.matching_bracket(Pos::new(2, 0)), Some(Pos::new(0, 10)));
        assert_eq!(b.pair_object(Pos::new(0, 6), '(', ')', false), Some((Pos::new(0, 2), Pos::new(0, 8))));
        assert_eq!(b.pair_object(Pos::new(1, 2), '{', '}', true), Some((Pos::new(0, 10), Pos::new(2, 1))));
        assert_eq!(b.word_object(Pos::new(3, 1), true), Some((Pos::new(3, 0), Pos::new(3, 4))));
        assert_eq!(b.quote_object(Pos::new(3, 0), '"', false), Some((Pos::new(3, 5), Pos::new(3, 13))));
        assert_eq!(b.quote_object(Pos::new(3, 7), '"', true), Some((Pos::new(3, 4), Pos::new(3, 14))));
    }

    #[test]
    fn find_and_search() {
        let b: Buffer = Buffer::from_text(None, "a-b-c\nalpha Beta\nbeta");
        assert_eq!(b.find_char(Pos::new(0, 0), '-', true, 2), Some(3));
        assert_eq!(b.find_char(Pos::new(0, 4), '-', false, 1), Some(3));
        assert_eq!(b.search("beta", Pos::new(0, 0), true), Some(Pos::new(1, 6)));
        assert_eq!(b.search("beta", Pos::new(2, 0), true), Some(Pos::new(1, 6)));
        assert_eq!(b.search("Beta", Pos::new(1, 6), true), Some(Pos::new(1, 6)));
        assert_eq!(b.search("beta", Pos::new(1, 6), false), Some(Pos::new(2, 0)));
    }
}
