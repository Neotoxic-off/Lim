//! todo-comments: `KEYWORD:` markers inside comments, with their kind.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    Fix,
    Task,
    Hack,
    Warn,
    Perf,
    Note,
    Test,
}

/// todo-comments' default keywords and aliases.
const KEYWORDS: &[(&str, Todo)] = &[
    ("FIX", Todo::Fix),
    ("FIXME", Todo::Fix),
    ("BUG", Todo::Fix),
    ("FIXIT", Todo::Fix),
    ("ISSUE", Todo::Fix),
    ("TODO", Todo::Task),
    ("HACK", Todo::Hack),
    ("WARN", Todo::Warn),
    ("WARNING", Todo::Warn),
    ("XXX", Todo::Warn),
    ("PERF", Todo::Perf),
    ("OPTIM", Todo::Perf),
    ("PERFORMANCE", Todo::Perf),
    ("OPTIMIZE", Todo::Perf),
    ("NOTE", Todo::Note),
    ("INFO", Todo::Note),
    ("TEST", Todo::Test),
    ("TESTING", Todo::Test),
    ("PASSED", Todo::Test),
    ("FAILED", Todo::Test),
];

/// The first `KEYWORD:` on the line: (char column, keyword length, kind).
pub fn todo(line: &str) -> Option<(usize, usize, Todo)> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_uppercase() && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let end = i + chars[i..].iter().take_while(|c| c.is_ascii_uppercase()).count();
            if chars.get(end) == Some(&':') {
                let word: String = chars[i..end].iter().collect();
                if let Some(&(_, kind)) = KEYWORDS.iter().find(|(k, _)| *k == word) {
                    return Some((i, end - i, kind));
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_keywords() {
        assert_eq!(todo("    // TODO: later"), Some((7, 4, Todo::Task)));
        assert_eq!(todo("# FIXME: x"), Some((2, 5, Todo::Fix)));
        assert_eq!(todo("let TODO = 1;"), None);
        assert_eq!(todo("XTODO: no"), None);
    }
}
