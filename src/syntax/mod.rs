//! Syntax highlighting through syntect with bat's grammars (two-face). Each line's start state is cached, so an
//! edit re-parses from the edited line on; scopes map to lim's highlight groups, so the themes stay as they are.

mod lang;
mod todo;

use std::path::Path;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeError, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

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

/// A line further than this past the parsed ones restarts the parse this many lines above it, as Vim's `syntax
/// sync minlines` does, so a jump into a huge file stays fast.
/// shortcut: a comment or string opened more than `SYNC` lines above a far jump is missed until scrolled through.
const SYNC: usize = 300;
/// Longer lines are drawn plain and not parsed (the state carries over), as editors do, so they stay fast.
const MAX_LINE: usize = 20_000;

/// Extensions bat has no grammar for, and the one that fits them.
const ALIASES: &[(&str, &str)] = &[
    ("h", "c"),
    ("ino", "cpp"),
    ("cu", "cpp"),
    ("jsx", "js"),
    ("mjs", "js"),
    ("cjs", "js"),
    ("mts", "ts"),
    ("cts", "ts"),
    ("astro", "html"),
    ("plist", "xml"),
    ("mdx", "md"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("s", "asm"),
    ("conf", "ini"),
];

/// Scope prefixes and their group, the first matching anywhere in the scope stack winning: a JSON key is a
/// string inside `meta.mapping.key`, and must read as a property.
const RULES: &[(&str, Hl)] = &[
    ("comment", Hl::Comment),
    ("meta.mapping.key", Hl::Property),
    ("support.type.property-name", Hl::Property),
    ("entity.name.tag.yaml", Hl::Property),
    ("entity.other.attribute-name", Hl::Property),
    ("variable.other.member", Hl::Property),
    ("variable.other.property", Hl::Property),
    ("string", Hl::Str),
    ("markup.raw", Hl::Str),
    ("markup.heading", Hl::Keyword),
    ("constant.numeric", Hl::Num),
    ("constant", Hl::Const),
    ("support.constant", Hl::Const),
    ("meta.preprocessor", Hl::Macro),
    ("support.macro", Hl::Macro),
    ("entity.name.function.macro", Hl::Macro),
    ("entity.name.function", Hl::Func),
    ("support.function", Hl::Func),
    ("variable.function", Hl::Func),
    ("entity.name.type", Hl::Type),
    ("entity.name.class", Hl::Type),
    ("entity.name.struct", Hl::Type),
    ("entity.name.enum", Hl::Type),
    ("entity.name.section", Hl::Type),
    ("support.type", Hl::Type),
    ("support.class", Hl::Type),
    ("variable.language", Hl::Builtin),
    ("support.variable", Hl::Builtin),
    // Operators stay plain, as before; `storage` (Rust's `fn`, `let`, C's `int`) reads as a keyword.
    ("keyword.operator", Hl::Text),
    ("keyword", Hl::Keyword),
    ("storage", Hl::Keyword),
    ("entity.name.tag", Hl::Keyword),
];

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn rules() -> &'static [(Scope, Hl)] {
    static PARSED: OnceLock<Vec<(Scope, Hl)>> = OnceLock::new();
    PARSED.get_or_init(|| RULES.iter().filter_map(|&(name, hl)| Some((Scope::new(name).ok()?, hl))).collect())
}

/// The grammar of a file: by its name (`Makefile`, `Dockerfile`, `.gitignore`), else its extension.
pub fn grammar_for(path: &Path) -> Option<&'static SyntaxReference> {
    let set: &SyntaxSet = syntaxes();
    let name: &str = path.file_name()?.to_str()?;
    if let Some(grammar) = set.find_syntax_by_extension(name) {
        return Some(grammar);
    }
    let ext: String = path.extension()?.to_str()?.to_ascii_lowercase();
    let ext: &str = ALIASES.iter().find(|(from, _)| *from == ext).map_or(ext.as_str(), |(_, to)| to);
    set.find_syntax_by_extension(ext)
}

/// The group of the text under `stack`.
fn group(stack: &ScopeStack) -> Hl {
    let scopes: &[Scope] = stack.as_slice();
    rules().iter().find(|(prefix, _)| scopes.iter().any(|s| prefix.is_prefix_of(*s))).map_or(Hl::Text, |&(_, hl)| hl)
}

/// The parser and scope stack at a line start.
type State = (ParseState, ScopeStack);

/// Parses `line` from `state`, leaving it at the next line's start; with `out`, pushes one group per char for the
/// first `limit` chars.
fn parse(line: &str, state: &mut State, out: Option<(&mut Vec<Hl>, usize)>) {
    let ops: Vec<(usize, ScopeStackOp)> = if line.len() > MAX_LINE {
        Vec::new()
    } else {
        state.0.parse_line(&format!("{line}\n"), syntaxes()).unwrap_or_default()
    };
    let stack: &mut ScopeStack = &mut state.1;
    let mut ops = ops.iter().peekable();
    if let Some((groups, limit)) = out {
        let mut current: Hl = group(stack);
        for (byte, _) in line.char_indices().take(limit) {
            let mut moved: bool = false;
            while let Some((_, op)) = ops.next_if(|(at, _)| *at <= byte) {
                let _: Result<(), ScopeError> = stack.apply(op);
                moved = true;
            }
            if moved {
                current = group(stack);
            }
            groups.push(current);
        }
    }
    for (_, op) in ops {
        let _: Result<(), ScopeError> = stack.apply(op);
    }
}

/// Start states of a window of lines, from `base` on, and the groups of the lines drawn (with the char limit
/// they were drawn to), so each line is parsed once until an edit truncates both.
#[derive(Default)]
pub struct Cache {
    base: usize,
    starts: Vec<State>,
    groups: Vec<Option<(usize, Vec<Hl>)>>,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cache {{ base: {}, states: {} }}", self.base, self.starts.len())
    }
}

impl Cache {
    /// Forgets what follows from `line` on: its own start state only depends on earlier lines.
    pub fn invalidate(&mut self, line: usize) {
        if line < self.base {
            (self.base, self.starts, self.groups) = (0, Vec::new(), Vec::new());
        } else {
            self.starts.truncate(line - self.base + 1);
            self.groups.truncate(line - self.base);
        }
    }

    /// Groups for the first `limit` chars of line `y` (a long line is drawn only in part).
    pub fn line(&mut self, lines: &[String], grammar: &SyntaxReference, y: usize, limit: usize) -> Vec<Hl> {
        let known: usize = self.base + self.starts.len().saturating_sub(1);
        if self.starts.is_empty() || y < self.base || y > known + SYNC {
            self.base = y.saturating_sub(SYNC);
            (self.starts, self.groups) = (vec![(ParseState::new(grammar), ScopeStack::new())], Vec::new());
        }
        while self.base + self.starts.len() <= y {
            let i: usize = self.starts.len() - 1;
            let mut next: State = self.starts[i].clone();
            parse(&lines[self.base + i], &mut next, None);
            self.starts.push(next);
        }
        let i: usize = y - self.base;
        if let Some(Some((drawn, groups))) = self.groups.get(i)
            && *drawn == limit
        {
            return groups.clone();
        }
        let mut state: State = self.starts[i].clone();
        let mut groups: Vec<Hl> = Vec::new();
        parse(&lines[y], &mut state, Some((&mut groups, limit)));
        // Drawing the last known line also gives the next line's start state.
        if self.starts.len() == i + 1 {
            self.starts.push(state);
        }
        if self.groups.len() <= i {
            self.groups.resize(i + 1, None);
        }
        self.groups[i] = Some((limit, groups.clone()));
        groups
    }
}

/// Loads the grammars on a background thread, so the first file opened does not wait for them.
pub fn preload() {
    std::thread::spawn(syntaxes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Groups of each line of `text` in the file type of `file`.
    fn hl(text: &str, file: &str) -> Vec<Vec<Hl>> {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let grammar: &SyntaxReference = grammar_for(Path::new(file)).unwrap();
        let mut cache: Cache = Cache::default();
        (0..lines.len()).map(|y| cache.line(&lines, grammar, y, usize::MAX)).collect()
    }

    #[test]
    fn tokens() {
        let h: Vec<Hl> = hl(r#"fn main() { let s = "a\"b"; // hi"#, "x.rs").remove(0);
        assert_eq!(h[0], Hl::Keyword, "fn");
        assert_eq!(h[3], Hl::Func, "main");
        assert_eq!(h[12], Hl::Keyword, "let");
        assert_eq!((h[20], h[23]), (Hl::Str, Hl::Str), "the escaped quote stays inside the string");
        assert_eq!(h.last(), Some(&Hl::Comment));
        let h: Vec<Hl> = hl("x = 42 -- c", "a.lua").remove(0);
        assert_eq!((h[4], h[8]), (Hl::Num, Hl::Comment));
    }

    #[test]
    fn multi_line_regions() {
        let text: &str = "let a = 1; /* start\nstill comment\nend */ let b";
        let lines: Vec<Vec<Hl>> = hl(text, "x.rs");
        assert!(lines[1].iter().all(|h| *h == Hl::Comment));
        assert_eq!((lines[2][5], lines[2][7]), (Hl::Comment, Hl::Keyword));
        let mut source: Vec<String> = text.lines().map(str::to_owned).collect();
        let grammar: &SyntaxReference = grammar_for(Path::new("x.rs")).unwrap();
        let mut cache: Cache = Cache::default();
        assert_eq!(cache.line(&source, grammar, 2, usize::MAX)[0], Hl::Comment);
        source[0] = "let a = 1;".into();
        cache.invalidate(0);
        assert_eq!(cache.line(&source, grammar, 1, usize::MAX)[0], Hl::Text, "the comment is gone once re-parsed");
    }

    /// A far jump parses only `SYNC` lines above it; a huge line is drawn plain without being parsed.
    #[test]
    fn far_jumps_and_huge_lines_stay_bounded() {
        let mut lines: Vec<String> = (0..10 * SYNC).map(|_| "let x = 1;".to_owned()).collect();
        lines.push("x".repeat(MAX_LINE + 1));
        let grammar: &SyntaxReference = grammar_for(Path::new("x.rs")).unwrap();
        let mut cache: Cache = Cache::default();
        assert_eq!(cache.line(&lines, grammar, 9 * SYNC, usize::MAX)[0], Hl::Keyword);
        assert_eq!((cache.base, cache.starts.len()), (8 * SYNC, SYNC + 2), "and the next line's start state");
        assert!(cache.line(&lines, grammar, 10 * SYNC, 100).iter().all(|h| *h == Hl::Text));
    }

    #[test]
    fn grammars_and_non_code() {
        for file in ["Makefile", "Dockerfile", ".gitignore", "x.toml", "x.yaml", "x.ts", "x.h", "x.jsonc", "x.go"] {
            assert!(grammar_for(Path::new(file)).is_some(), "{file}");
        }
        let h: Vec<Hl> = hl(r#"<a href="x">it's</a>"#, "x.html").remove(0);
        assert_eq!((h[1], h[3], h[9]), (Hl::Keyword, Hl::Property, Hl::Str));
        let h: Vec<Hl> = hl("name: true # c", "x.yaml").remove(0);
        assert_eq!((h[0], h[6], h[13]), (Hl::Property, Hl::Const, Hl::Comment));
        let h: Vec<Hl> = hl(r#"{"k": "v"}"#, "x.json").remove(0);
        assert_eq!((h[2], h[7]), (Hl::Property, Hl::Str));
        assert!(hl("# Title", "a.md").remove(0).iter().skip(2).all(|h| *h == Hl::Keyword));
    }
}
