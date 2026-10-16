//! Keyword-based highlighting standing in for treesitter, with multi-line regions
//! (block comments, long strings) carried from line to line.

mod lang;

pub use lang::{Lang, lang_for};

/// Highlight groups, named after the treesitter captures astrotheme colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hl {
    Text,
    Keyword,
    Type,
    Builtin,
    Str,
    Num,
    Comment,
    Func,
    Const,
    Macro,
    Property,
}

/// Highlighter state at a line boundary: inside which of `Lang::regions`, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    region: Option<usize>,
}

/// Start state of each line, valid for a prefix of the buffer; edits truncate it.
#[derive(Debug)]
pub struct Cache {
    starts: Vec<State>,
}

impl Default for Cache {
    fn default() -> Self {
        Self { starts: vec![State::default()] }
    }
}

impl Cache {
    /// Forgets states after `line`: its own start state only depends on earlier lines.
    pub fn invalidate(&mut self, line: usize) {
        self.starts.truncate(line + 1);
    }

    /// Groups for line `y`, scanning forward from the last known state.
    pub fn line(&mut self, lines: &[String], lang: &Lang, y: usize) -> Vec<Hl> {
        while self.starts.len() <= y {
            let i = self.starts.len() - 1;
            let chars: Vec<char> = lines[i].chars().collect();
            let end = highlight(&chars, lang, self.starts[i]).1;
            self.starts.push(end);
        }
        let chars: Vec<char> = lines[y].chars().collect();
        highlight(&chars, lang, self.starts[y]).0
    }
}

/// True when `pat` (ASCII) occurs in `line` at `i`.
fn at(line: &[char], i: usize, pat: &str) -> bool {
    pat.chars().enumerate().all(|(k, c)| line.get(i + k) == Some(&c))
}

/// Index just past the region closer, honoring backslash escapes in strings.
fn close_of(line: &[char], from: usize, close: &str, escapes: bool) -> Option<usize> {
    let mut i = from;
    while i < line.len() {
        if escapes && line[i] == '\\' {
            i += 2;
        } else if at(line, i, close) {
            return Some(i + close.len());
        } else {
            i += 1;
        }
    }
    None
}

/// One group per char of `line`, and the state the next line starts in.
pub fn highlight(line: &[char], lang: &Lang, state: State) -> (Vec<Hl>, State) {
    let mut hl = vec![Hl::Text; line.len()];
    let mut region = state.region;
    let mut i = 0;
    while i < line.len() || region.is_some() {
        if let Some(r) = region {
            let (_, close, group) = lang.regions[r];
            match close_of(line, i, close, group == Hl::Str) {
                Some(end) => {
                    hl[i..end].fill(group);
                    i = end;
                    region = None;
                }
                None => {
                    hl[i..].fill(group);
                    return (hl, State { region });
                }
            }
            continue;
        }
        if let Some(r) = lang.regions.iter().position(|(open, ..)| at(line, i, open)) {
            let (open, _, group) = lang.regions[r];
            hl[i..i + open.len()].fill(group);
            i += open.len();
            region = Some(r);
            continue;
        }
        let c = line[i];
        let start = i;
        if at(line, i, lang.comment) {
            hl[i..].fill(Hl::Comment);
            break;
        } else if lang.quotes.contains(c) {
            i = close_of(line, i + 1, &c.to_string(), true).unwrap_or(line.len());
            hl[start..i].fill(Hl::Str);
        } else if c.is_ascii_digit() {
            while i < line.len() && (line[i].is_alphanumeric() || line[i] == '_' || line[i] == '.') {
                i += 1;
            }
            hl[start..i].fill(Hl::Num);
        } else if c.is_alphabetic() || c == '_' {
            while i < line.len() && (line[i].is_alphanumeric() || line[i] == '_') {
                i += 1;
            }
            let word: String = line[start..i].iter().collect();
            let next = line[i..].iter().find(|c| **c != ' ').copied();
            hl[start..i].fill(classify(&word, next, start.checked_sub(1).map(|p| line[p]), lang));
        } else {
            i += 1;
        }
    }
    (hl, State::default())
}

fn classify(word: &str, next: Option<char>, prev: Option<char>, lang: &Lang) -> Hl {
    if lang.keywords.contains(&word) {
        Hl::Keyword
    } else if lang.consts.contains(&word) {
        Hl::Const
    } else if lang.builtins.contains(&word) {
        Hl::Builtin
    } else if next == Some('!') && lang.macros {
        Hl::Macro
    } else if next == Some('(') {
        Hl::Func
    } else if word.starts_with(char::is_uppercase) {
        if word.chars().any(char::is_lowercase) { Hl::Type } else { Hl::Const }
    } else if prev == Some('.') {
        Hl::Property
    } else {
        Hl::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn lang(file: &str) -> &'static Lang {
        lang_for(Path::new(file)).unwrap()
    }

    fn hl(s: &str, file: &str) -> Vec<Hl> {
        highlight(&s.chars().collect::<Vec<_>>(), lang(file), State::default()).0
    }

    #[test]
    fn tokens() {
        let h = hl(r#"fn main() { let s = "a\"b"; // hi"#, "x.rs");
        assert_eq!(h[0], Hl::Keyword);
        assert_eq!(h[3], Hl::Func);
        assert_eq!(h[20], Hl::Str);
        assert_eq!(h[25], Hl::Str); // escaped quote stays inside the string
        assert_eq!(h[27], Hl::Text);
        assert_eq!(h.last(), Some(&Hl::Comment));
        let h = hl("x = 42 -- c", "a.lua");
        assert_eq!((h[4], h[8]), (Hl::Num, Hl::Comment));
    }

    #[test]
    fn multi_line_regions() {
        let lines: Vec<String> = ["let a = 1; /* start", "still comment", "end */ let b", "--[[ lua"]
            .map(String::from)
            .into();
        let rust = lang("x.rs");
        let mut cache = Cache::default();
        assert!(cache.line(&lines, rust, 1).iter().all(|h| *h == Hl::Comment));
        let third = cache.line(&lines, rust, 2);
        assert_eq!((third[5], third[7]), (Hl::Comment, Hl::Keyword));
        cache.invalidate(0);
        assert_eq!(cache.line(&lines, rust, 2)[7], Hl::Keyword);
        let lua = lang("x.lua");
        assert!(highlight(&lines[3].chars().collect::<Vec<_>>(), lua, State::default()).1 != State::default());
    }
}
