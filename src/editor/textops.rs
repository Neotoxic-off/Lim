//! Ex commands that compute over lines, built in so they never wait on a process: `:uniq`, the encodings
//! (`:base64`, `:base32`, `:hex`, `:url`, `:rot13`), `:sha256`, `:wc`, `:calc`, `:case` and `:json`. Sort, case, trim and reverse are
//! already `:sort`, `gU`/`gu`, `:s` and `:g/^/m0`.

use std::fmt::{self, Write};

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
            TextOp::Base64 { decode: false } => vec![base64(text.as_bytes())],
            TextOp::Base32 { decode: false } => vec![base32(text.as_bytes())],
            TextOp::Hex { decode: false } => vec![text.bytes().fold(String::new(), |mut hex, b| {
                let _: fmt::Result = write!(hex, "{b:02x}");
                hex
            })],
            TextOp::Base64 { decode: true } | TextOp::Base32 { decode: true } | TextOp::Hex { decode: true } => {
                let bytes: Option<Vec<u8>> = match op {
                    TextOp::Base64 { .. } => unbase64(&text),
                    TextOp::Base32 { .. } => unbase32(&text),
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
                let hash: String = sha256(text.as_bytes());
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

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32, `=` padded.
fn base32(data: &[u8]) -> String {
    let mut out: String = String::with_capacity(data.len().div_ceil(5) * 8);
    for chunk in data.chunks(5) {
        let mut block: [u8; 8] = [0; 8];
        block[3..3 + chunk.len()].copy_from_slice(chunk);
        let n: u64 = u64::from_be_bytes(block);
        // 1 to 5 input bytes give 2, 4, 5, 7 or 8 significant chars.
        let used: usize = [0, 2, 4, 5, 7, 8][chunk.len()];
        for i in 0..8 {
            let index: u64 = n >> (35 - 5 * i) & 31;
            out.push(if i < used { char::from(BASE32[usize::try_from(index).unwrap_or(0)]) } else { '=' });
        }
    }
    out
}

/// Blanks and `=` are skipped, lowercase is accepted; None on a char outside the alphabet.
fn unbase32(text: &str) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let (mut acc, mut bits): (u32, u32) = (0, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let value: u32 = u32::try_from(BASE32.iter().position(|b| *b == c.to_ascii_uppercase())?).ok()?;
        acc = (acc << 5 | value) & 0xffff;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(acc >> bits & 0xff).ok()?);
        }
    }
    Some(out)
}

fn url_encode(line: &str) -> String {
    line.bytes().fold(String::with_capacity(line.len()), |mut out, b| {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            let _: fmt::Result = write!(out, "%{b:02X}");
        }
        out
    })
}

/// None on a `%` without two hex digits, or bytes that are not UTF-8.
fn url_decode(line: &str) -> Option<String> {
    let mut out: Vec<u8> = Vec::with_capacity(line.len());
    let mut bytes = line.bytes();
    while let Some(b) = bytes.next() {
        out.push(if b == b'%' {
            *hex_bytes(&String::from_utf8(vec![bytes.next()?, bytes.next()?]).ok()?)?.first()?
        } else {
            b
        });
    }
    String::from_utf8(out).ok()
}

fn rot13(c: char) -> char {
    let rotate = |base: u8| char::from((u8::try_from(c).unwrap_or(base) - base + 13) % 26 + base);
    match c {
        'a'..='z' => rotate(b'a'),
        'A'..='Z' => rotate(b'A'),
        _ => c,
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out: String = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b: [u8; 3] = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let sextets: [u8; 4] = [b[0] >> 2, (b[0] & 3) << 4 | b[1] >> 4, (b[1] & 15) << 2 | b[2] >> 6, b[2] & 63];
        for (i, s) in sextets.into_iter().enumerate() {
            out.push(if i <= chunk.len() { char::from(BASE64[usize::from(s)]) } else { '=' });
        }
    }
    out
}

/// Blanks are skipped and decoding stops at the first `=`; None on any other byte outside the alphabet.
fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let (mut acc, mut bits): (u32, u32) = (0, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()).take_while(|c| *c != b'=') {
        let value: u32 = u32::try_from(BASE64.iter().position(|b| *b == c)?).ok()?;
        acc = (acc << 6 | value) & 0xff_ffff;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(acc >> bits & 0xff).ok()?);
        }
    }
    Some(out)
}

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// SHA-256 (FIPS 180-4) as lowercase hex.
#[expect(clippy::many_single_char_names, reason = "the working variables keep their names from the standard")]
fn sha256(data: &[u8]) -> String {
    let mut h: [u32; 8] =
        [0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19];
    let bits: u64 = u64::try_from(data.len()).unwrap_or_default().wrapping_mul(8);
    let mut message: Vec<u8> = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w: [u32; 64] = [0; 64];
        for (slot, word) in w.iter_mut().zip(block.chunks_exact(4)) {
            *slot = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0: u32 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1: u32 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v: [u32; 8] = h;
        for (k, wi) in K.iter().zip(w) {
            let [a, b, c, d, e, f, g, hh]: [u32; 8] = v;
            let t1: u32 = hh
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(*k)
                .wrapping_add(wi);
            let t2: u32 =
                (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
            v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().fold(String::with_capacity(64), |mut hex, x| {
        let _: fmt::Result = write!(hex, "{x:08x}");
        hex
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes() {
        assert_eq!(base64(b"hi\n"), "aGkK");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(unbase64("aGkK\n").unwrap(), b"hi\n");
        assert_eq!(unbase64("YW\nI=").unwrap(), b"ab");
        assert_eq!(unbase64("a*"), None);
        assert_eq!(sha256(b"hi\n"), "98ea6e4f216f2fb4b69fff9b3a44842c38686ca685f3f55dc48c5d3fb1107be4");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        // 56 bytes: the padding spills into a second block.
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI======");
        assert_eq!(base32(b"f"), "MY======");
        assert_eq!(unbase32("MZXW6YTBOI======").unwrap(), b"foobar");
        assert_eq!(unbase32("mzxw6\n").unwrap(), b"foo");
        assert_eq!(unbase32("M1"), None);
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
