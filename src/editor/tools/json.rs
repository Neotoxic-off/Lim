//! `:json` / `<Leader>xj`: JSON (and JSONC) laid out or minified.

use crate::config::json;

/// `text` re-laid out (two-space indent, or `minify`d), token by token, so numbers and strings stay exactly as
/// written. Invalid JSON is refused. `//` and `/* */` comments (JSONC, as config files) are kept: one after a
/// token stays on its line, one on its own line keeps it; minifying drops them, a `//` would swallow the rest.
pub fn json(text: &str, minify: bool) -> Result<String, String> {
    json::parse(text).map_err(|e| format!("invalid JSON: {e}"))?;
    let chars: Vec<char> = text.chars().collect();
    let mut layout: Layout = Layout::new(minify, text.len());
    let mut i: usize = 0;
    while let Some(&c) = chars.get(i) {
        i += 1;
        if layout.in_string {
            layout.string_char(c);
        } else if c == '/' && matches!(chars.get(i), Some('/' | '*')) {
            i = layout.comment(&chars, i - 1);
        } else {
            layout.token(c, &chars[i..]);
        }
    }
    Ok(layout.out)
}

/// The text being laid out, and where the layout stands in it.
#[expect(clippy::struct_excessive_bools, reason = "independent states of the scan, each read on its own")]
struct Layout {
    out: String,
    minify: bool,
    depth: usize,
    in_string: bool,
    /// The last char in a string was a backslash.
    escaped: bool,
    /// The bracket just opened closes right away: `{}` and `[]` stay on one line.
    empty: bool,
    /// A line break waits for the next token, so a comment after a token can still join its line.
    pending: bool,
    /// The source broke the line since the last token.
    broke: bool,
}

impl Layout {
    fn new(minify: bool, capacity: usize) -> Self {
        let out: String = String::with_capacity(capacity);
        Self { out, minify, depth: 0, in_string: false, escaped: false, empty: false, pending: false, broke: false }
    }

    /// Writes the waiting line break, indented to the current depth.
    fn newline(&mut self) {
        if std::mem::take(&mut self.pending) && !self.minify {
            self.out.push('\n');
            self.out.push_str(&"  ".repeat(self.depth));
        }
    }

    /// A char inside a string, copied as it is.
    fn string_char(&mut self, c: char) {
        self.out.push(c);
        if self.escaped {
            self.escaped = false;
        } else if c == '\\' {
            self.escaped = true;
        } else if c == '"' {
            self.in_string = false;
        }
    }

    /// The comment starting at `start` (`//` or `/*`); returns where the text goes on after it.
    fn comment(&mut self, chars: &[char], start: usize) -> usize {
        let line: bool = chars.get(start + 1) == Some(&'/');
        // The parser accepted the text, so a block comment is closed.
        let end: usize = if line {
            chars[start..].iter().position(|&c| c == '\n').map_or(chars.len(), |p| start + p)
        } else {
            chars[start + 2..].windows(2).position(|w| w == ['*', '/']).map_or(chars.len(), |p| start + 2 + p + 2)
        };
        if !self.minify {
            let comment: String = chars[start..end].iter().collect::<String>().trim_end().to_owned();
            self.place_comment(&comment, line);
        }
        end
    }

    /// On its own line in the source, a comment keeps one; otherwise it joins the token before it.
    fn place_comment(&mut self, comment: &str, line: bool) {
        let own_line: bool = self.broke || self.out.is_empty();
        if own_line {
            self.pending |= !self.out.is_empty();
            self.newline();
        } else if !self.out.ends_with(' ') {
            self.out.push(' ');
        }
        self.out.push_str(comment);
        if line || own_line {
            self.pending = true;
        } else if !self.pending {
            // Inside a line (`"a": /* x */ 1`): stays there.
            self.out.push(' ');
        }
    }

    /// A char outside strings and comments; `rest` is the text after it.
    fn token(&mut self, c: char, rest: &[char]) {
        match c {
            '\n' => self.broke = true,
            c if c.is_whitespace() => {}
            '{' | '[' => self.open(c, rest),
            '}' | ']' => self.close(c),
            ',' => {
                self.newline();
                self.out.push(c);
                self.pending = true;
            }
            ':' => {
                self.newline();
                self.out.push_str(if self.minify { ":" } else { ": " });
            }
            c => {
                self.newline();
                self.in_string = c == '"';
                self.out.push(c);
            }
        }
        if !c.is_whitespace() {
            self.broke = false;
        }
    }

    fn open(&mut self, bracket: char, rest: &[char]) {
        self.newline();
        self.out.push(bracket);
        self.depth += 1;
        self.empty = rest.iter().find(|c| !c.is_whitespace()).is_some_and(|n| matches!(n, '}' | ']'));
        self.pending = !self.empty;
    }

    fn close(&mut self, bracket: char) {
        self.depth = self.depth.saturating_sub(1);
        self.pending |= !std::mem::take(&mut self.empty);
        self.newline();
        self.out.push(bracket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        let text: &str = r#"{"a": [1, 2.50, {}], "b": {"c": "x,{y}"}, "d": []}"#;
        assert_eq!(json(text, true).unwrap(), r#"{"a":[1,2.50,{}],"b":{"c":"x,{y}"},"d":[]}"#);
        assert_eq!(
            json(text, false).unwrap(),
            "{\n  \"a\": [\n    1,\n    2.50,\n    {}\n  ],\n  \"b\": {\n    \"c\": \"x,{y}\"\n  },\n  \"d\": []\n}"
        );
        assert!(json("{\"a\": }", false).is_err());
        // JSONC: comments keep their place, trailing or on their own line; minifying drops them.
        let commented: &str =
            "// top\n{\"a\": 1, // one\n  /* own line */\n\"b\": /* inline */ [2,\n3], \"c\": 4 // last\n}";
        assert_eq!(
            json(commented, false).unwrap(),
            "// top\n{\n  \"a\": 1, // one\n  /* own line */\n  \"b\": /* inline */ [\n    2,\n    3\n  ],\n  \"c\": 4 // last\n}"
        );
        assert_eq!(json(commented, true).unwrap(), r#"{"a":1,"b":[2,3],"c":4}"#);
        assert_eq!(
            json("{\"url\": \"http://x/*y*/\"}", true).unwrap(),
            "{\"url\":\"http://x/*y*/\"}",
            "not in strings"
        );
        let formatted: String = json(commented, false).unwrap();
        assert_eq!(json(&formatted, false).unwrap(), formatted, "formatting twice changes nothing");
        let config: String = json(crate::config::EXAMPLE, false).unwrap();
        assert_eq!(config.matches("//").count(), crate::config::EXAMPLE.matches("//").count(), "{config}");
        assert_eq!(json(&config, false).unwrap(), config);
        assert_eq!(json(r#"["a\"b", "c\\"]"#, true).unwrap(), r#"["a\"b","c\\"]"#, "escapes inside strings");
    }
}
