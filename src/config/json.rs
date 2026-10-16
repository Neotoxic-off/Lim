//! A small JSON reader (no crates). Accepts `//` and `/* */` comments and trailing commas,
//! since config files are written by hand.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    /// Keys keep file order, so errors and docs read naturally.
    Object(Vec<(String, Json)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

/// Nesting deeper than this is rejected instead of risking the stack.
const MAX_DEPTH: usize = 64;

pub fn parse(text: &str) -> Result<Json, ParseError> {
    let mut p = Parser { chars: text.chars().collect(), pos: 0 };
    let value = p.value(0)?;
    p.skip_blank()?;
    if p.pos < p.chars.len() {
        return Err(p.error("unexpected text after the value"));
    }
    Ok(value)
}

impl Json {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::String(_) => "a string",
            Self::Array(_) => "an array",
            Self::Object(_) => "an object",
        }
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn error(&self, message: &str) -> ParseError {
        let before = &self.chars[..self.pos.min(self.chars.len())];
        let line = 1 + before.iter().filter(|&&c| c == '\n').count();
        let col = 1 + before.iter().rev().take_while(|&&c| c != '\n').count();
        ParseError { line, col, message: message.to_owned() }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_blank(&mut self) -> Result<(), ParseError> {
        loop {
            match (self.peek(), self.chars.get(self.pos + 1).copied()) {
                (Some(c), _) if c.is_whitespace() => self.pos += 1,
                (Some('/'), Some('/')) => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.pos += 1;
                    }
                }
                (Some('/'), Some('*')) => {
                    self.pos += 2;
                    loop {
                        match (self.peek(), self.chars.get(self.pos + 1).copied()) {
                            (Some('*'), Some('/')) => {
                                self.pos += 2;
                                break;
                            }
                            (Some(_), _) => self.pos += 1,
                            (None, _) => return Err(self.error("unterminated comment")),
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn expect(&mut self, c: char) -> Result<(), ParseError> {
        self.skip_blank()?;
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{c}'")))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        self.skip_blank()?;
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => self.string().map(Json::String),
            Some('t') => self.word("true", Json::Bool(true)),
            Some('f') => self.word("false", Json::Bool(false)),
            Some('n') => self.word("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.error("expected a value")),
            None => Err(self.error("unexpected end of file")),
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, ParseError> {
        let matches = word.chars().enumerate().all(|(i, c)| self.chars.get(self.pos + i) == Some(&c));
        if !matches {
            return Err(self.error("expected a value"));
        }
        self.pos += word.len();
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || "+-.eE".contains(c)) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse().map(Json::Number).map_err(|_| self.error("invalid number"))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else { return Err(self.error("unterminated string")) };
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let Some(esc) = self.peek() else { return Err(self.error("unterminated string")) };
                    self.pos += 1;
                    out.push(match esc {
                        '"' => '"',
                        '\\' => '\\',
                        '/' => '/',
                        'b' => '\u{8}',
                        'f' => '\u{c}',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'u' => self.unicode_escape()?,
                        _ => return Err(self.error("invalid escape")),
                    });
                }
                '\n' => return Err(self.error("newline inside a string")),
                c => out.push(c),
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, ParseError> {
        let hex = |p: &mut Self| -> Result<u32, ParseError> {
            let digits: String = p.chars.get(p.pos..p.pos + 4).map(|d| d.iter().collect()).unwrap_or_default();
            p.pos += 4;
            u32::from_str_radix(&digits, 16).map_err(|_| p.error("invalid \\u escape"))
        };
        let high = hex(self)?;
        let code = if (0xD800..0xDC00).contains(&high) {
            if self.peek() != Some('\\') || self.chars.get(self.pos + 1) != Some(&'u') {
                return Err(self.error("unpaired surrogate"));
            }
            self.pos += 2;
            let low = hex(self)?;
            0x10000 + ((high - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF)
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| self.error("invalid code point"))
    }

    fn array(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_blank()?;
            if self.peek() == Some(']') {
                self.pos += 1;
                return Ok(Json::Array(items));
            }
            items.push(self.value(depth + 1)?);
            self.skip_blank()?;
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(']') => {}
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.expect('{')?;
        let mut fields = Vec::new();
        loop {
            self.skip_blank()?;
            if self.peek() == Some('}') {
                self.pos += 1;
                return Ok(Json::Object(fields));
            }
            let key = self.string()?;
            self.expect(':')?;
            fields.push((key, self.value(depth + 1)?));
            self.skip_blank()?;
            match self.peek() {
                Some(',') => self.pos += 1,
                Some('}') => {}
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_shaped_json() {
        let text = r#"{
            // comment
            "a": [1, -2.5e1, true, null,],
            "s": "x\"\u00e9\ud83e\udd80", /* block */
        }"#;
        let Ok(Json::Object(fields)) = parse(text) else { panic!("not an object") };
        assert_eq!(fields[0].1, Json::Array(vec![Json::Number(1.0), Json::Number(-25.0), Json::Bool(true), Json::Null]));
        assert_eq!(fields[1].1, Json::String("x\"é🦀".into()));
        assert_eq!(parse("{\"a\" 1}").map_err(|e| (e.line, e.col)), Err((1, 6)));
        assert!(parse("[".repeat(100).as_str()).is_err());
        assert!(parse("\"\\ud800\"").is_err());
    }
}
