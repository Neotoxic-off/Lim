//! JSON read through jsonc-parser: `//` and `/* */` comments and trailing commas are accepted, since config files
//! are written by hand, and nothing looser (quotes, numbers and commas stay strict JSON).

use std::fmt::{self, Write};

use jsonc_parser::{JsonValue, ParseOptions, parse_to_value};

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

/// The value in `text`; the parser bounds nesting itself, so deep input is an error, not a stack overflow.
pub fn parse(text: &str) -> Result<Json, ParseError> {
    let options: ParseOptions = ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        ..ParseOptions::default()
    };
    match parse_to_value(text, &options) {
        Ok(Some(value)) => Ok(convert(value)),
        Ok(None) => Err(ParseError { line: 1, col: 1, message: "unexpected end of file".into() }),
        Err(e) => Err(ParseError { line: e.line_display(), col: e.column_display(), message: e.kind().to_string() }),
    }
}

fn convert(value: JsonValue<'_>) -> Json {
    match value {
        JsonValue::Null => Json::Null,
        JsonValue::Boolean(b) => Json::Bool(b),
        // The parser has checked the syntax; f64 reads every JSON number (huge ones become infinite).
        JsonValue::Number(n) => Json::Number(n.parse().unwrap_or(f64::NAN)),
        JsonValue::String(s) => Json::String(s.into_owned()),
        JsonValue::Array(items) => Json::Array(items.into_iter().map(convert).collect()),
        JsonValue::Object(fields) => {
            Json::Object(fields.into_iter().map(|(k, v)| (k.into_owned(), convert(v))).collect())
        }
    }
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

/// `value` as indented JSON, two spaces per level.
pub fn pretty(value: &Json) -> String {
    let mut out: String = String::new();
    write_value(&mut out, value, 0);
    out
}

fn write_value(out: &mut String, value: &Json, depth: usize) {
    let indent = |out: &mut String, depth: usize| out.push_str(&"  ".repeat(depth));
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => out.push_str(&n.to_string()),
        Json::String(s) => write_string(out, s),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Object(fields) if fields.is_empty() => out.push_str("{}"),
        Json::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                indent(out, depth + 1);
                write_value(out, item, depth + 1);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            indent(out, depth);
            out.push(']');
        }
        Json::Object(fields) => {
            out.push_str("{\n");
            for (i, (key, item)) in fields.iter().enumerate() {
                indent(out, depth + 1);
                write_string(out, key);
                out.push_str(": ");
                write_value(out, item, depth + 1);
                out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
            }
            indent(out, depth);
            out.push('}');
        }
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _: fmt::Result = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_shaped_json() {
        let text: &str = r#"{
            // comment
            "a": [1, -2.5e1, true, null,],
            "s": "x\"\u00e9\ud83e\udd80", /* block */
        }"#;
        let Ok(Json::Object(fields)): Result<Json, ParseError> = parse(text) else { panic!("not an object") };
        assert_eq!(
            fields[0].1,
            Json::Array(vec![Json::Number(1.0), Json::Number(-25.0), Json::Bool(true), Json::Null])
        );
        assert_eq!(fields[1].1, Json::String("x\"é🦀".into()));
        assert_eq!(parse("{\"a\" 1}").map_err(|e| (e.line, e.col)), Err((1, 6)));
        assert!(parse("[".repeat(100).as_str()).is_err());
        assert!(parse("\"\\ud800\"").is_err());
    }

    #[test]
    fn pretty_reads_back() {
        let value: Json = Json::Object(vec![
            ("a".into(), Json::Array(vec![Json::Number(2.0), Json::Number(-0.5), Json::Null])),
            ("s".into(), Json::String("q\"\\\n\u{1}é".into())),
            ("o".into(), Json::Object(Vec::new())),
            ("b".into(), Json::Bool(false)),
        ]);
        assert_eq!(parse(&pretty(&value)), Ok(value));
        assert_eq!(pretty(&Json::Object(vec![("n".into(), Json::Number(4.0))])), "{\n  \"n\": 4\n}");
    }
}
