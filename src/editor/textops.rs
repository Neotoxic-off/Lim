//! Ex commands that compute over lines, built in so they never wait on a process: `:uniq`, the encodings
//! (`:base64`, `:base32`, `:hex`, `:url`, `:rot13`), `:sha256`, `:wc`, `:calc`, `:case` and `:json`. Sort, case, trim and reverse are
//! already `:sort`, `gU`/`gu`, `:s` and `:g/^/m0`.

use data_encoding::{BASE32, BASE64, Encoding, HEXLOWER, Specification};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use sha2::{Digest, Sha256};

use super::Editor;
use super::re::hex_bytes;
use super::tools::{WordCase, evaluate, format_number, json};
use super::vim::LineRange;
use crate::buffer::{Buffer, Pos};
use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOp {
    /// `:uniq`: drops repeated adjacent lines, as Vim 9.1.
    Uniq,
    /// `:base64` encodes the lines (newlines included) into one line, `:base64!` decodes them.
    Base64 { decode: bool },
    /// `:base32` / `:base32!`, as `:base64`.
    Base32 { decode: bool },
    /// `:hex` encodes the lines (newlines included) into hex digits, `:hex!` decodes them; `0x`, `\x`, commas and
    /// blanks are skipped, so C arrays decode too.
    Hex { decode: bool },
    /// `:url` percent-encodes each line, `:url!` decodes it.
    Url { decode: bool },
    /// `:rot13` on each line.
    Rot13,
    /// `:sha256`: the hash of the lines as a file would hold them.
    Sha256,
    /// `:wc`: lines, words and chars.
    Wc,
    /// `:calc`: the arithmetic of the lines (the calculator's syntax, `tools::calc`).
    Calc,
    /// `:case snake` (camel, pascal, kebab, constant, title, lower, upper) on each line.
    Case(WordCase),
    /// `:json` lays the lines out as indented JSON, `:json!` minifies them.
    Json { minify: bool },
}

impl Editor {
    pub(super) fn text_op(&mut self, range: LineRange, op: TextOp) -> Result<(), Error> {
        let last: usize = range.last.min(self.buf().last_line());
        let lines: &[String] = &self.buf().lines()[range.first..=last];
        let text: String = lines.join("\n") + "\n";
        let new: Vec<String> = match op {
            TextOp::Uniq => {
                let mut new: Vec<String> = lines.to_vec();
                new.dedup();
                new
            }
            TextOp::Base64 { decode: false } => vec![BASE64.encode(text.as_bytes())],
            TextOp::Base32 { decode: false } => vec![BASE32.encode(text.as_bytes())],
            TextOp::Hex { decode: false } => vec![HEXLOWER.encode(text.as_bytes())],
            TextOp::Base64 { decode: true } | TextOp::Base32 { decode: true } | TextOp::Hex { decode: true } => {
                let bytes: Option<Vec<u8>> = match op {
                    TextOp::Base64 { .. } => decode(&text, &BASE64),
                    TextOp::Base32 { .. } => decode(&text, &BASE32),
                    _ => {
                        let digits: String = text.replace("0x", "").replace("\\x", "");
                        hex_bytes(&digits.replace(|c: char| c == ',' || c.is_whitespace(), ""))
                    }
                };
                let decoded: String = bytes
                    .and_then(|b| String::from_utf8(b).ok())
                    .ok_or_else(|| Error::Vim("E475: Invalid argument: not UTF-8 once decoded".into()))?;
                let new: Vec<String> = decoded.lines().map(str::to_owned).collect();
                if new.is_empty() { vec![String::new()] } else { new }
            }
            TextOp::Url { decode: false } => lines.iter().map(|l| url_encode(l)).collect(),
            TextOp::Url { decode: true } => lines
                .iter()
                .map(|l| url_decode(l))
                .collect::<Option<Vec<String>>>()
                .ok_or_else(|| Error::Vim("E475: Invalid argument: bad %XX or not UTF-8".into()))?,
            TextOp::Rot13 => lines.iter().map(|l| l.chars().map(rot13).collect()).collect(),
            TextOp::Sha256 => {
                let hash: String = HEXLOWER.encode(&Sha256::digest(text.as_bytes()));
                self.info(hash);
                return Ok(());
            }
            TextOp::Wc => {
                let words: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
                let chars: usize = lines.iter().map(|l| l.chars().count() + 1).sum();
                let count: usize = lines.len();
                self.info(format!("{count} lines, {words} words, {chars} chars"));
                return Ok(());
            }
            TextOp::Calc => {
                let expr: String = lines.join(" ");
                let value: f64 =
                    evaluate(&expr, self.calc_ans).map_err(|e| Error::Vim(format!("E15: {e}: \"{expr}\"")))?;
                self.calc_ans = Some(value);
                self.info(format_number(value));
                return Ok(());
            }
            TextOp::Case(case) => lines.iter().map(|l| case.apply(l)).collect(),
            TextOp::Json { minify } => {
                let laid_out: String = json(&text, minify).map_err(|e| Error::Vim(format!("E474: {e}")))?;
                laid_out.lines().map(str::to_owned).collect()
            }
        };
        let buf: &mut Buffer = self.buf_mut();
        if buf.lines()[range.first..=last] != new[..] {
            buf.seal();
            buf.splice(range.first..last + 1, new);
        }
        buf.cursor = Pos::new(range.first, 0);
        buf.clamp(false);
        Ok(())
    }
}

/// Decodes leniently, as people paste: blanks and `=` padding are skipped (base64 stops at the first `=`), and a
/// lowercase base32 is accepted; None on any other byte outside the alphabet.
fn decode(text: &str, base: &Encoding) -> Option<Vec<u8>> {
    let mut spec: Specification = base.specification();
    spec.padding = None;
    spec.check_trailing_bits = false;
    let digits: String = text
        .split('=')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| if spec.symbols.len() == 32 { c.to_ascii_uppercase() } else { c })
        .collect();
    spec.encoding().ok()?.decode(digits.as_bytes()).ok()
}

/// Everything but letters, digits and `-_.~` is %-escaped.
const URL_ESCAPED: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

fn url_encode(line: &str) -> String {
    utf8_percent_encode(line, URL_ESCAPED).to_string()
}

/// None on a `%` without two hex digits (the crate would keep it as is), or bytes that are not UTF-8.
fn url_decode(line: &str) -> Option<String> {
    let escapes_ok: bool = line
        .match_indices('%')
        .all(|(i, _)| line.as_bytes().get(i + 1..i + 3).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)));
    escapes_ok.then(|| percent_decode_str(line).decode_utf8().ok().map(String::from)).flatten()
}

fn rot13(c: char) -> char {
    let rotate = |base: u8| char::from((u8::try_from(c).unwrap_or(base) - base + 13) % 26 + base);
    match c {
        'a'..='z' => rotate(b'a'),
        'A'..='Z' => rotate(b'A'),
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes() {
        assert_eq!(decode("aGkK\n", &BASE64).unwrap(), b"hi\n");
        assert_eq!(decode("YW\nI=", &BASE64).unwrap(), b"ab", "blanks and padding are skipped");
        assert_eq!(decode("YWI", &BASE64).unwrap(), b"ab", "padding may be missing");
        assert_eq!(decode("a*", &BASE64), None);
        assert_eq!(decode("MZXW6YTBOI======", &BASE32).unwrap(), b"foobar");
        assert_eq!(decode("mzxw6\n", &BASE32).unwrap(), b"foo", "lowercase base32");
        assert_eq!(decode("M1", &BASE32), None);
        assert_eq!(url_encode("a b/é~"), "a%20b%2F%C3%A9~");
        assert_eq!(url_decode("a%20b%2F%C3%A9~").unwrap(), "a b/é~");
        for bad in ["%2", "%zz", "%ff", "%é"] {
            assert_eq!(url_decode(bad), None, "{bad}");
        }
        assert_eq!("Hello, World".chars().map(rot13).collect::<String>(), "Uryyb, Jbeyq");
        assert_eq!(evaluate("2*(3+4)/7", None).ok(), Some(2.0));
        assert_eq!(evaluate("-2^2 + 10 % 4 - .5", None).ok(), Some(-2.5));
        assert_eq!(evaluate("2^3^2", None).ok(), Some(512.0));
        assert_eq!(evaluate("(1", None).ok(), Some(1.0), "a parenthesis left open is closed");
        for bad in ["", "1/0", "1)", "2..3", "x", &"(".repeat(10_000), &"-".repeat(10_000)] {
            assert_eq!(evaluate(bad, None).ok(), None, "{bad:.20}");
        }
    }
}
