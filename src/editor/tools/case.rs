//! Identifier and phrase cases (`<Leader>xk`, `:case`): snake, camel, kebab and the rest.

/// Identifier and phrase cases for `<Leader>xk` and `:case`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordCase {
    Snake,
    Camel,
    Pascal,
    Kebab,
    Constant,
    Title,
    Lower,
    Upper,
}

impl WordCase {
    pub const NAMES: [(&'static str, Self); 8] = [
        ("snake", Self::Snake),
        ("camel", Self::Camel),
        ("pascal", Self::Pascal),
        ("kebab", Self::Kebab),
        ("constant", Self::Constant),
        ("title", Self::Title),
        ("lower", Self::Lower),
        ("upper", Self::Upper),
    ];

    /// The case named `name`, or by a prefix of its name (`sn`, `k`).
    pub fn parse(name: &str) -> Option<Self> {
        let name: String = name.trim().to_ascii_lowercase();
        Self::NAMES.iter().find(|(n, _)| !name.is_empty() && n.starts_with(name.as_str())).map(|&(_, case)| case)
    }

    /// `text` split into words (at blanks, punctuation and camelCase humps), joined in this case.
    pub fn apply(self, text: &str) -> String {
        let words: Vec<String> = words(text);
        let capital = |w: &String| -> String {
            let mut chars = w.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect())
                .unwrap_or_default()
        };
        let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
        match self {
            Self::Snake => lower.join("_"),
            Self::Kebab => lower.join("-"),
            Self::Constant => words.iter().map(|w| w.to_uppercase()).collect::<Vec<String>>().join("_"),
            Self::Lower => lower.join(" "),
            Self::Upper => words.iter().map(|w| w.to_uppercase()).collect::<Vec<String>>().join(" "),
            Self::Title => words.iter().map(capital).collect::<Vec<String>>().join(" "),
            Self::Pascal => words.iter().map(capital).collect(),
            Self::Camel => {
                let mut out: String = lower.first().cloned().unwrap_or_default();
                out.extend(words.iter().skip(1).map(capital));
                out
            }
        }
    }
}

/// The words of `text`: runs of letters and digits, also split where a lowercase letter meets an uppercase one
/// (`fooBar`) and before the last capital of an acronym (`HTTPServer`).
fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word: String = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if !word.is_empty() && c.is_uppercase() {
            let prev: char = chars[i - 1];
            let hump: bool = prev.is_lowercase() || prev.is_numeric();
            let acronym_end: bool = prev.is_uppercase() && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if hump || acronym_end {
                words.push(std::mem::take(&mut word));
            }
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cases() {
        assert_eq!(words("parseHTTPServer_v2-x"), ["parse", "HTTP", "Server", "v2", "x"]);
        let all: Vec<String> = WordCase::NAMES.iter().map(|(_, c)| c.apply("fooBar baz")).collect();
        assert_eq!(
            all,
            [
                "foo_bar_baz",
                "fooBarBaz",
                "FooBarBaz",
                "foo-bar-baz",
                "FOO_BAR_BAZ",
                "Foo Bar Baz",
                "foo bar baz",
                "FOO BAR BAZ"
            ]
        );
        assert_eq!(WordCase::parse("k"), Some(WordCase::Kebab));
        assert_eq!(WordCase::parse(""), None);
    }
}
