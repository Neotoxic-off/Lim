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

/// A jump further than this past the window of known states moves the window instead of growing it.
const SYNC: usize = 2000;
/// Lines between the saved start states a far jump scans from.
const CHECKPOINT: usize = 4096;

/// Start states of a window of lines (from `base` on), and of every `CHECKPOINT`-th line scanned so far; edits
/// truncate both. States between are found with `end_state`, which only tracks regions, so a jump to the end of
/// a 3M-line file stays exact (a comment opened on line 1 still colors it) and fast.
#[derive(Debug, Default)]
pub struct Cache {
    base: usize,
    starts: Vec<State>,
    checkpoints: Vec<State>,
}

impl Cache {
    /// Forgets states after `line`: its own start state only depends on earlier lines.
    pub fn invalidate(&mut self, line: usize) {
        if line < self.base {
            (self.base, self.starts) = (0, Vec::new());
        } else {
            self.starts.truncate(line - self.base + 1);
        }
        self.checkpoints.truncate(line / CHECKPOINT + 1);
    }

    /// Groups for the first `limit` chars of line `y` (a long line is drawn only in part).
    pub fn line(&mut self, lines: &[String], lang: &Lang, y: usize, limit: usize) -> Vec<Hl> {
        let known: usize = self.base + self.starts.len().saturating_sub(1);
        if self.starts.is_empty() || y < self.base || y > known + SYNC {
            let base: usize = y.saturating_sub(SYNC);
            let start: State = self.state_at(lines, lang, base);
            (self.base, self.starts) = (base, vec![start]);
        }
        let firsts: Firsts = firsts(lang);
        while self.base + self.starts.len() <= y {
            let i: usize = self.starts.len() - 1;
            let end: State = end_state(lines[self.base + i].as_bytes(), lang, &firsts, self.starts[i]);
            self.starts.push(end);
        }
        let chars: Vec<char> = lines[y].chars().take(limit).collect();
        highlight(&chars, lang, self.starts[y - self.base]).0
    }

    /// The start state of line `y`, scanned from the nearest state known above it.
    fn state_at(&mut self, lines: &[String], lang: &Lang, y: usize) -> State {
        if self.checkpoints.is_empty() {
            self.checkpoints.push(State::default());
        }
        let n: usize = (y / CHECKPOINT).min(self.checkpoints.len() - 1);
        let (mut line, mut state): (usize, State) = (n * CHECKPOINT, self.checkpoints[n]);
        let known: usize = self.base + self.starts.len().saturating_sub(1);
        if !self.starts.is_empty() && (line..=y).contains(&known) {
            (line, state) = (known, self.starts[known - self.base]);
        }
        let firsts: Firsts = firsts(lang);
        while line < y {
            state = end_state(lines[line].as_bytes(), lang, &firsts, state);
            line += 1;
            if line.is_multiple_of(CHECKPOINT) && line / CHECKPOINT == self.checkpoints.len() {
                self.checkpoints.push(state);
            }
        }
        state
    }
}

/// `end_state` for the fuzzer, which checks it against `highlight`.
#[cfg(test)]
pub fn fast_end_state(line: &str, lang: &Lang, state: State) -> State {
    end_state(line.as_bytes(), lang, &firsts(lang), state)
}

/// Which bytes can start a region, a comment or a quote of a language.
type Firsts = [bool; 256];

fn firsts(lang: &Lang) -> Firsts {
    let mut firsts: Firsts = [false; 256];
    let openers = lang.regions.iter().map(|(open, ..)| *open).chain([lang.comment]).filter_map(|p| p.bytes().next());
    for b in openers.chain(lang.quotes.bytes()) {
        firsts[usize::from(b)] = true;
    }
    firsts
}

/// The state after `line`, like `highlight(..).1` without coloring anything. Every region, comment and quote
/// delimiter is ASCII and none starts with a word char, so bytes can be skipped up to the next possible one.
fn end_state(line: &[u8], lang: &Lang, firsts: &Firsts, state: State) -> State {
    if lang.kind == Kind::Markdown && state.region.is_none() && line.first() == Some(&b'#') {
        return state;
    }
    let at = |i: usize, pat: &str| line[i..].starts_with(pat.as_bytes());
    let close = |from: usize, close: &str, escapes: bool| {
        let mut i: usize = from;
        while i < line.len() {
            if escapes && line[i] == b'\\' {
                i += 2;
            } else if at(i, close) {
                return Some(i + close.len());
            } else {
                i += 1;
            }
        }
        None
    };
    let mut region: Option<usize> = state.region;
    let mut i: usize = 0;
    loop {
        if let Some(r) = region {
            let (_, closer, group): (&str, &str, Hl) = lang.regions[r];
            let Some(end): Option<usize> = close(i, closer, group == Hl::Str) else { return State { region } };
            (i, region) = (end, None);
            continue;
        }
        let Some(skip): Option<usize> =
            line.get(i..).and_then(|rest| rest.iter().position(|&b| firsts[usize::from(b)]))
        else {
            return State::default();
        };
        i += skip;
        if let Some(r) = lang.regions.iter().position(|(open, ..)| at(i, open)) {
            (i, region) = (i + lang.regions[r].0.len(), Some(r));
        } else if !lang.comment.is_empty() && at(i, lang.comment) {
            return State::default();
        } else if lang.quotes.as_bytes().contains(&line[i]) {
            i = close(i + 1, &char::from(line[i]).to_string(), true).unwrap_or(line.len());
        } else {
            i += 1;
        }
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
        assert!(cache.line(&lines, rust, 1, usize::MAX).iter().all(|h| *h == Hl::Comment));
        let third: Vec<Hl> = cache.line(&lines, rust, 2, usize::MAX);
        assert_eq!((third[5], third[7]), (Hl::Comment, Hl::Keyword));
        cache.invalidate(0);
        assert_eq!(cache.line(&lines, rust, 2, usize::MAX)[7], Hl::Keyword);
        let lua: &Lang = lang("x.lua");
        assert!(highlight(&lines[3].chars().collect::<Vec<_>>(), lua, State::default()).1 != State::default());
    }

    /// A far jump keeps only a window of states, yet stays exact: a comment opened on line 1 still colors it.
    #[test]
    fn far_lines_stay_exact() {
        let mut lines: Vec<String> = vec!["/* open".into()];
        lines.extend((0..3 * CHECKPOINT).map(|_| "let x".to_owned()));
        let rust: &Lang = lang("x.rs");
        let mut cache: Cache = Cache::default();
        let far: usize = 3 * CHECKPOINT;
        assert_eq!(cache.line(&lines, rust, far, usize::MAX)[0], Hl::Comment);
        assert_eq!((cache.base, cache.starts.len(), cache.checkpoints.len()), (far - SYNC, SYNC + 1, 3));
        lines[0] = "// closed".into();
        cache.invalidate(0);
        assert_eq!(cache.checkpoints.len(), 1, "checkpoints below the edit are dropped");
        assert_eq!(cache.line(&lines, rust, far, usize::MAX)[0], Hl::Keyword);
        lines[2 * CHECKPOINT + 5] = "/*".into();
        cache.invalidate(2 * CHECKPOINT + 5);
        assert_eq!(cache.line(&lines, rust, 5, usize::MAX)[0], Hl::Keyword, "back to the top, from a checkpoint");
        assert_eq!(cache.line(&lines, rust, far, usize::MAX)[0], Hl::Comment);
        lines[0] = "/* open".into();
        lines[2 * CHECKPOINT + 5] = "let x".into();
        let mut cache: Cache = Cache::default();
        assert_eq!(cache.line(&lines, rust, SYNC + 500, usize::MAX)[0], Hl::Comment);
        assert_eq!(cache.line(&lines, rust, 2 * SYNC + 1000, usize::MAX)[0], Hl::Comment, "from the window");
        assert_eq!(cache.checkpoints.len(), 1);
    }

    /// `end_state` agrees with the full highlighter on every kind of delimiter.
    #[test]
    fn end_state_matches_highlight() {
        let files: [&str; 16] = [
            "x.rs", "x.lua", "x.py", "x.rb", "x.pl", "x.ps1", "x.md", "x.hs", "x.js", "x.c", "x.html", "x.go", "x.sql",
            "x.sh", "x.json", "x.toml",
        ];
        let samples: [&str; 18] = [
            "",
            "plain é words 1.5",
            "a /* b */ c /* d",
            "*/ x \"str /* not\" y",
            "s = \"esc \\\" /*\" // c /*",
            "--[[ lua ]] [[ str",
            "]] x -- c [[",
            "\"\"\" py ''' x",
            "x ''' y \"\"\" z",
            "=begin ruby",
            "=end =pod",
            "=cut <# ps #> <#",
            "# title ```",
            "``` `tick` <!-- html",
            "--> {- hs -} {-",
            "-} 'a\\'b' \"\\",
            "日本 /* 語 */ \"é",
            "x \\",
        ];
        for (file, lang) in files.iter().filter_map(|f| Some((f, lang_for(Path::new(f))?))) {
            let firsts: Firsts = firsts(lang);
            let mut state: State = State::default();
            for line in samples {
                let chars: Vec<char> = line.chars().collect();
                let want: State = highlight(&chars, lang, state).1;
                assert_eq!(end_state(line.as_bytes(), lang, &firsts, state), want, "{line:?} in {file}");
                state = want;
            }
        }
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
