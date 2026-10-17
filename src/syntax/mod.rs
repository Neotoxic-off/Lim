//! Keyword-based highlighting standing in for treesitter, with multi-line regions
//! (block comments, long strings) carried from line to line.

mod lang;
mod todo;

pub use lang::Kind;
pub use lang::{Lang, lang_for};
pub use todo::{Todo, todo};

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
            let i: usize = self.starts.len() - 1;
            let chars: Vec<char> = lines[i].chars().collect();
            let end: State = highlight(&chars, lang, self.starts[i]).1;
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
    let mut i: usize = from;
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
    if lang.kind == Kind::Markdown && state.region.is_none() && line.first() == Some(&'#') {
        return (vec![Hl::Keyword; line.len()], state);
    }
    let mut hl: Vec<Hl> = vec![Hl::Text; line.len()];
    let mut region: Option<usize> = state.region;
    let mut i: usize = 0;
    while i < line.len() || region.is_some() {
        if let Some(r) = region {
            let (_, close, group): (&str, &str, Hl) = lang.regions[r];
            if let Some(end) = close_of(line, i, close, group == Hl::Str) {
                hl[i..end].fill(group);
                i = end;
                region = None;
            } else {
                hl[i..].fill(group);
                return (hl, State { region });
            }
            continue;
        }
        if let Some(r) = lang.regions.iter().position(|(open, ..)| at(line, i, open)) {
            let (open, _, group): (&str, &str, Hl) = lang.regions[r];
            hl[i..i + open.len()].fill(group);
            i += open.len();
            region = Some(r);
            continue;
        }
        let c: char = line[i];
        let start: usize = i;
        if !lang.comment.is_empty() && at(line, i, lang.comment) {
            hl[i..].fill(Hl::Comment);
            break;
        } else if lang.quotes.contains(c) {
            i = close_of(line, i + 1, &c.to_string(), true).unwrap_or(line.len());
            let key: bool = lang.kind == Kind::Config && line[i..].iter().find(|c| **c != ' ') == Some(&':');
            hl[start..i].fill(if key { Hl::Property } else { Hl::Str });
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
            let next: Option<char> = line[i..].iter().find(|c| **c != ' ').copied();
            hl[start..i].fill(classify(&word, next, &line[..start], lang));
        } else {
            i += 1;
        }
    }
    (hl, State::default())
}

/// The group of `word`, given the first non-blank char after it and the text before it.
fn classify(word: &str, next: Option<char>, before: &[char], lang: &Lang) -> Hl {
    let prev: Option<char> = before.last().copied();
    match lang.kind {
        Kind::Markup if prev == Some('<') || before.ends_with(&['<', '/']) => return Hl::Keyword,
        Kind::Markup if next == Some('=') => return Hl::Property,
        Kind::Config if matches!(next, Some(':' | '=')) => return Hl::Property,
        Kind::Config if prev == Some('[') => return Hl::Type,
        _ => {}
    }
    let key: String = if lang.ignore_case { word.to_ascii_lowercase() } else { word.to_owned() };
    let key: &str = &key;
    if lang.keywords.contains(&key) {
        Hl::Keyword
    } else if lang.consts.contains(&key) {
        Hl::Const
    } else if lang.builtins.contains(&key) {
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
        let h: Vec<Hl> = hl(r#"fn main() { let s = "a\"b"; // hi"#, "x.rs");
        assert_eq!(h[0], Hl::Keyword);
        assert_eq!(h[3], Hl::Func);
        assert_eq!(h[20], Hl::Str);
        assert_eq!(h[25], Hl::Str); // escaped quote stays inside the string
        assert_eq!(h[27], Hl::Text);
        assert_eq!(h.last(), Some(&Hl::Comment));
        let h: Vec<Hl> = hl("x = 42 -- c", "a.lua");
        assert_eq!((h[4], h[8]), (Hl::Num, Hl::Comment));
    }

    #[test]
    fn multi_line_regions() {
        let lines: Vec<String> =
            ["let a = 1; /* start", "still comment", "end */ let b", "--[[ lua"].map(String::from).into();
        let rust: &Lang = lang("x.rs");
        let mut cache: Cache = Cache::default();
        assert!(cache.line(&lines, rust, 1).iter().all(|h| *h == Hl::Comment));
        let third: Vec<Hl> = cache.line(&lines, rust, 2);
        assert_eq!((third[5], third[7]), (Hl::Comment, Hl::Keyword));
        cache.invalidate(0);
        assert_eq!(cache.line(&lines, rust, 2)[7], Hl::Keyword);
        let lua: &Lang = lang("x.lua");
        assert!(highlight(&lines[3].chars().collect::<Vec<_>>(), lua, State::default()).1 != State::default());
    }

    #[test]
    fn non_code_rules() {
        let h: Vec<Hl> = hl(r#"<a href="x">it's</a>"#, "x.html");
        assert_eq!((h[1], h[3], h[8], h[14], h[18]), (Hl::Keyword, Hl::Property, Hl::Str, Hl::Text, Hl::Keyword));
        let h: Vec<Hl> = hl("name: true # c", "x.yaml");
        assert_eq!((h[0], h[6], h[13]), (Hl::Property, Hl::Const, Hl::Comment));
        let h: Vec<Hl> = hl(r#"{"k": "v"}"#, "x.json");
        assert_eq!((h[1], h[7]), (Hl::Property, Hl::Str));
        assert_eq!(hl("[server]", "x.toml")[1], Hl::Type);
        let h: Vec<Hl> = hl("SELECT x FROM t", "q.sql");
        assert_eq!((h[0], h[7], h[9]), (Hl::Keyword, Hl::Text, Hl::Keyword));
        assert!(hl("# Title", "a.md").iter().all(|h| *h == Hl::Keyword));
        assert!(hl("a: b", "x.css").iter().all(|h| *h != Hl::Comment), "no line comment in CSS");
    }
}
