//! Reverse-engineering helpers over the buffer's bytes: `:detect` (file type, architecture, entropy, packer),
//! `:strings`, and in a hex buffer (`:hexedit`, or any binary file) `:inspect`, `:pack` and `:xor`.

use std::fmt::Write;

use super::Editor;
use super::vim::LineRange;
use crate::buffer::{Buffer, hex};
use crate::error::Error;

/// `:pack`'s value types: `u8`, `i8`, then `u16` … `i64`, `f32`, `f64` with `le` or `be`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackType {
    pub bytes: usize,
    pub kind: NumKind,
    pub big_endian: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumKind {
    Unsigned,
    Signed,
    Float,
}

impl PackType {
    pub fn parse(name: &str) -> Option<Self> {
        let (body, big_endian): (&str, bool) = match name.strip_suffix("le") {
            Some(body) => (body, false),
            None => name.strip_suffix("be").map_or((name, false), |body| (body, true)),
        };
        let kind: NumKind = match body.chars().next()? {
            'u' => NumKind::Unsigned,
            'i' => NumKind::Signed,
            'f' => NumKind::Float,
            _ => return None,
        };
        let bits: usize = body[1..].parse().ok()?;
        let fits: bool = match kind {
            NumKind::Float => matches!(bits, 32 | 64),
            _ => matches!(bits, 8 | 16 | 32 | 64),
        };
        // `u8le` is meaningless but harmless; `u16` alone must say its byte order.
        (fits && (bits == 8 || body.len() < name.len())).then_some(Self { bytes: bits / 8, kind, big_endian })
    }

    /// `value` (decimal, `0x` hex, or a float for `f32`/`f64`) as this type's bytes.
    fn encode(self, value: &str) -> Option<Vec<u8>> {
        let mut bytes: Vec<u8> = if self.kind == NumKind::Float {
            if self.bytes == 4 {
                value.parse::<f32>().ok()?.to_le_bytes().to_vec()
            } else {
                value.parse::<f64>().ok()?.to_le_bytes().to_vec()
            }
        } else {
            let (negative, digits): (bool, &str) = value.strip_prefix('-').map_or((false, value), |d| (true, d));
            let magnitude: i128 = match digits.strip_prefix("0x") {
                Some(h) => i128::from_str_radix(h, 16).ok()?,
                None => digits.parse().ok()?,
            };
            let v: i128 = if negative { -magnitude } else { magnitude };
            let bits: u32 = u32::try_from(self.bytes * 8).ok()?;
            let (min, max): (i128, i128) = if self.kind == NumKind::Signed {
                (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
            } else {
                (0, (1 << bits) - 1)
            };
            if !(min..=max).contains(&v) {
                return None;
            }
            v.to_le_bytes()[..self.bytes].to_vec()
        };
        if self.big_endian {
            bytes.reverse();
        }
        Some(bytes)
    }
}

impl Editor {
    /// The byte offset under the cursor of a hex buffer.
    fn hex_cursor(&self) -> Result<usize, Error> {
        let buf: &Buffer = self.buf();
        if !buf.hex {
            return Err(Error::Vim("E21: not a hex buffer, :hexedit first".into()));
        }
        let before: Vec<u8> = hex::undump(&buf.lines()[..buf.cursor.line]).map_err(Error::Vim)?;
        Ok(before.len() + hex::byte_in_line(buf.line(buf.cursor.line), buf.cursor.col))
    }

    /// Redraws the dump of a hex buffer from `bytes`, replacing only the lines that differ (one undo step).
    fn set_hex_bytes(&mut self, bytes: &[u8]) {
        let new: Vec<String> = hex::dump(bytes, 0);
        let buf: &mut Buffer = self.buf_mut();
        let old: &[String] = buf.lines();
        let head: usize = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let tail: usize = old[head..].iter().rev().zip(new[head..].iter().rev()).take_while(|(a, b)| a == b).count();
        let end: usize = old.len() - tail;
        let changed: Vec<String> = new[head..new.len() - tail].to_vec();
        if head < end || !changed.is_empty() {
            buf.seal();
            buf.splice(head..end, changed);
        }
        buf.clamp(false);
    }

    /// `:inspect`: the bytes under the cursor read as every integer and float type.
    pub(super) fn inspect(&mut self) -> Result<(), Error> {
        let at: usize = self.hex_cursor()?;
        let bytes: Vec<u8> = self.buf().bytes()?;
        let rest: &[u8] = bytes.get(at..).unwrap_or_default();
        let take = |n: usize| -> Option<[u8; 8]> {
            let mut le: [u8; 8] = [0; 8];
            le[..n].copy_from_slice(rest.get(..n)?);
            Some(le)
        };
        let mut out: String = format!("0x{at:x}:");
        for n in [1, 2, 4, 8] {
            let Some(le): Option<[u8; 8]> = take(n) else { break };
            let mut be: [u8; 8] = [0; 8];
            be[..n].copy_from_slice(&le[..n]);
            be[..n].reverse();
            let (little, big): (u64, u64) = (u64::from_le_bytes(le), u64::from_le_bytes(be));
            let shift: u32 = 64 - 8 * u32::try_from(n).unwrap_or(8);
            // Shifting up then back sign-extends the n-byte value.
            let signed = |u: u64| i64::from_le_bytes(u.to_le_bytes()) << shift >> shift;
            if n == 1 {
                let _: std::fmt::Result = write!(out, " u8 {little} i8 {}", signed(little));
            } else {
                let bits: usize = n * 8;
                let _: std::fmt::Result =
                    write!(out, " · u{bits} {little}/{big} i{bits} {}/{}", signed(little), signed(big));
            }
            if n == 4 {
                let f: [u8; 4] = [le[0], le[1], le[2], le[3]];
                let _: std::fmt::Result = write!(out, " f32 {}", f32::from_le_bytes(f));
            }
            if n == 8 {
                let _: std::fmt::Result = write!(out, " f64 {}", f64::from_le_bytes(le));
            }
        }
        self.info(out + "  (le/be)");
        Ok(())
    }

    /// `:pack {type} {value}`: writes the value's bytes over the ones at the cursor.
    pub(super) fn pack(&mut self, ty: PackType, value: &str) -> Result<(), Error> {
        let at: usize = self.hex_cursor()?;
        let new: Vec<u8> = ty.encode(value).ok_or_else(|| Error::Vim(format!("E475: Invalid argument: {value}")))?;
        let mut bytes: Vec<u8> = self.buf().bytes()?;
        if bytes.len() < at + new.len() {
            bytes.resize(at + new.len(), 0);
        }
        bytes[at..at + new.len()].copy_from_slice(&new);
        self.set_hex_bytes(&bytes);
        self.info(format!("{} bytes written at 0x{at:x}", new.len()));
        Ok(())
    }

    /// `:[range]xor {key}`: the bytes of the dump lines, `XORed` with the key (`0x41ff` hex, else its text).
    pub(super) fn xor(&mut self, range: LineRange, key: &str) -> Result<(), Error> {
        let key: Vec<u8> = match key.strip_prefix("0x") {
            Some(h) => hex_bytes(h),
            None => Some(key.as_bytes().to_vec()),
        }
        .filter(|k| !k.is_empty())
        .ok_or_else(|| Error::Vim(format!("E475: Invalid argument: {key}")))?;
        let buf: &Buffer = self.buf();
        if !buf.hex {
            return Err(Error::Vim("E21: not a hex buffer, :hexedit first".into()));
        }
        let start: usize = hex::undump(&buf.lines()[..range.first]).map_err(Error::Vim)?.len();
        let len: usize = hex::undump(&buf.lines()[range.first..=range.last]).map_err(Error::Vim)?.len();
        let mut bytes: Vec<u8> = buf.bytes()?;
        for (b, k) in bytes[start..start + len].iter_mut().zip(key.iter().cycle()) {
            *b ^= k;
        }
        self.set_hex_bytes(&bytes);
        Ok(())
    }

    /// `:detect`: what the bytes are, by their magic numbers, with their size and entropy.
    pub(super) fn detect(&mut self) -> Result<(), Error> {
        let bytes: Vec<u8> = self.buf().bytes()?;
        let entropy: f64 = entropy(&bytes);
        let mut out: String = format!("{} · {} bytes · entropy {entropy:.2}", detect(&bytes), bytes.len());
        if find(&bytes, b"UPX!").is_some() || find(&bytes, b"UPX0").is_some() {
            out.push_str(" · UPX packed");
        } else if entropy > 7.2 {
            out.push_str(" · packed, compressed or encrypted");
        }
        self.info(out);
        Ok(())
    }

    /// `:strings`: printable runs of 4+ chars, ASCII and UTF-16LE, with their offsets, in a new buffer.
    pub(super) fn strings(&mut self) -> Result<(), Error> {
        let bytes: Vec<u8> = self.buf().bytes()?;
        let name: String = self.buf().name();
        let mut found: Vec<(usize, &str, String)> = strings(&bytes, 1).into_iter().map(|(at, s)| (at, "", s)).collect();
        found.extend(strings(&bytes, 2).into_iter().map(|(at, s)| (at, "u16 ", s)));
        found.sort_by_key(|(at, _, _)| *at);
        let text: String = found.iter().fold(String::new(), |mut text, (at, kind, s)| {
            let _: std::fmt::Result = writeln!(text, "{at:08x}: {kind}{s}");
            text
        });
        self.show_scratch(&format!("strings: {name}"), Some(text));
        Ok(())
    }
}

/// Hex digits two by two; None when odd or not hex.
pub fn hex_bytes(digits: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> =
        digits.chars().map(|c| c.to_digit(16).and_then(|d| u8::try_from(d).ok())).collect::<Option<Vec<u8>>>()?;
    digits.len().is_multiple_of(2).then(|| digits.chunks(2).map(|p| p[0] << 4 | p[1]).collect())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Shannon entropy in bits per byte: 0 for one repeated byte, 8 for random data.
fn entropy(bytes: &[u8]) -> f64 {
    let mut counts: [u32; 256] = [0; 256];
    for b in bytes {
        counts[usize::from(*b)] = counts[usize::from(*b)].saturating_add(1);
    }
    let total: f64 = f64::from(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    counts.iter().filter(|c| **c > 0).map(|c| f64::from(*c) / total).map(|p| -p * p.log2()).sum()
}

fn u16_at(bytes: &[u8], at: usize, big: bool) -> Option<u16> {
    let pair: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
    Some(if big { u16::from_be_bytes(pair) } else { u16::from_le_bytes(pair) })
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The file type from its first bytes, with the architecture of executables.
fn detect(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x7fELF") {
        let bits: &str = if bytes.get(4) == Some(&2) { "64" } else { "32" };
        let big: bool = bytes.get(5) == Some(&2);
        let machine: &str = match u16_at(bytes, 18, big) {
            Some(3) => "x86",
            Some(0x3e) => "x86-64",
            Some(0x28) => "ARM",
            Some(0xb7) => "AArch64",
            Some(0xf3) => "RISC-V",
            Some(8) => "MIPS",
            Some(0x14 | 0x15) => "PowerPC",
            _ => "unknown machine",
        };
        let kind: &str = match u16_at(bytes, 16, big) {
            Some(1) => "relocatable",
            Some(2) => "executable",
            Some(3) => "shared object",
            Some(4) => "core",
            _ => "",
        };
        return format!("ELF {bits}-bit {} {kind} {machine}", if big { "MSB" } else { "LSB" });
    }
    if bytes.starts_with(b"MZ") {
        let pe: Option<usize> = u32_at(bytes, 0x3c).and_then(|o| usize::try_from(o).ok());
        let Some(pe): Option<usize> = pe.filter(|o| bytes.get(*o..).is_some_and(|r| r.starts_with(b"PE\0\0"))) else {
            return "MS-DOS executable".into();
        };
        let machine: &str = match u16_at(bytes, pe + 4, false) {
            Some(0x14c) => "x86",
            Some(0x8664) => "x86-64",
            Some(0xaa64) => "ARM64",
            Some(0x1c0 | 0x1c4) => "ARM",
            _ => "unknown machine",
        };
        let dll: bool = u16_at(bytes, pe + 22, false).is_some_and(|c| c & 0x2000 != 0);
        let plus: bool = u16_at(bytes, pe + 24, false) == Some(0x20b);
        let dotnet: bool = find(bytes, b"mscoree.dll").is_some() || find(bytes, b"#Strings").is_some();
        return format!(
            "PE32{} {} {machine}{}",
            if plus { "+" } else { "" },
            if dll { "DLL" } else { "executable" },
            if dotnet { " .NET" } else { "" }
        );
    }
    let magics: &[(&[u8], &str)] = &[
        (b"\xcf\xfa\xed\xfe", "Mach-O 64-bit"),
        (b"\xce\xfa\xed\xfe", "Mach-O 32-bit"),
        (b"\xca\xfe\xba\xbe", "Mach-O universal or Java class"),
        (b"dex\n", "Android DEX"),
        (b"\0asm", "WebAssembly"),
        (b"PK\x03\x04", "ZIP (also jar, apk, docx)"),
        (b"\x1f\x8b", "gzip"),
        (b"BZh", "bzip2"),
        (b"\xfd7zXZ\0", "xz"),
        (b"\x28\xb5\x2f\xfd", "zstd"),
        (b"7z\xbc\xaf\x27\x1c", "7-Zip"),
        (b"Rar!", "RAR"),
        (b"MSCF", "Microsoft Cabinet"),
        (b"\xd0\xcf\x11\xe0", "OLE compound file (doc, xls, msi)"),
        (b"%PDF", "PDF"),
        (b"\x89PNG", "PNG"),
        (b"\xff\xd8\xff", "JPEG"),
        (b"GIF8", "GIF"),
        (b"SQLite format 3\0", "SQLite database"),
        (b"#!", "script"),
    ];
    magics.iter().find(|(magic, _)| bytes.starts_with(magic)).map_or_else(
        || if std::str::from_utf8(bytes).is_ok() { "text".into() } else { "data".into() },
        |(_, name)| (*name).to_owned(),
    )
}

/// Runs of 4+ printable ASCII chars, each `width` bytes (2: UTF-16LE, the high byte zero), with their offsets.
fn strings(bytes: &[u8], width: usize) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut run: String = String::new();
    let mut start: usize = 0;
    for (i, unit) in bytes.chunks_exact(width).enumerate() {
        let printable: bool = (unit[0].is_ascii_graphic() || unit[0] == b' ') && unit[1..].iter().all(|b| *b == 0);
        if printable {
            if run.is_empty() {
                start = i * width;
            }
            run.push(char::from(unit[0]));
        }
        if !printable || i + 1 == bytes.len() / width {
            if run.len() >= 4 {
                found.push((start, std::mem::take(&mut run)));
            }
            run.clear();
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        let mut elf: Vec<u8> = b"\x7fELF\x02\x01".to_vec();
        elf.resize(20, 0);
        elf[16] = 2;
        elf[18] = 0x3e;
        assert_eq!(detect(&elf), "ELF 64-bit LSB executable x86-64");
        let mut pe: Vec<u8> = vec![0; 0x100];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c] = 0x80;
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        pe[0x96..0x98].copy_from_slice(&0x2022_u16.to_le_bytes());
        pe[0x98..0x9a].copy_from_slice(&0x20b_u16.to_le_bytes());
        assert_eq!(detect(&pe), "PE32+ DLL x86-64");
        pe[0x3c] = 0xff;
        assert_eq!(detect(&pe), "MS-DOS executable", "PE offset past the end");
        assert_eq!(detect(b"\x89PNG\r\n"), "PNG");
        assert_eq!(detect(b"hello"), "text");
        assert_eq!(detect(b"\xff\x00"), "data");
        assert_eq!(detect(b""), "text");
        assert!(entropy(&[7; 100]).abs() < 1e-9);
        assert!((entropy(&(0..=255).collect::<Vec<u8>>()) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn finds_strings() {
        assert_eq!(strings(b"\x00abc\x01hello\x02world", 1), [(5, String::from("hello")), (11, "world".into())]);
        assert_eq!(strings(b"x\0h\0e\0y\0!\0\0\0", 2), [(0, String::from("xhey!"))]);
        assert_eq!(strings(b"abc", 2), Vec::<(usize, String)>::new());
    }

    #[test]
    fn packs_values() {
        let ty = |name: &str| PackType::parse(name).unwrap();
        assert_eq!(ty("u32le").encode("0x11223344").unwrap(), [0x44, 0x33, 0x22, 0x11]);
        assert_eq!(ty("u16be").encode("258").unwrap(), [1, 2]);
        assert_eq!(ty("i8").encode("-1").unwrap(), [0xff]);
        assert_eq!(ty("i64le").encode("-2").unwrap(), [0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        assert_eq!(ty("f32le").encode("1.5").unwrap(), 1.5_f32.to_le_bytes());
        assert_eq!(ty("u64be").encode("18446744073709551615").unwrap(), [0xff; 8]);
        assert_eq!(ty("u8").encode("256"), None);
        assert_eq!(ty("i16le").encode("-32769"), None);
        assert_eq!(ty("u8").encode("-1"), None);
        assert_eq!(ty("u8").encode("x"), None);
        for bad in ["u16", "u24le", "f16le", "x32le", "", "le", "u"] {
            assert_eq!(PackType::parse(bad), None, "{bad}");
        }
        assert_eq!(hex_bytes("41ff").unwrap(), [0x41, 0xff]);
        assert_eq!(hex_bytes("4"), None);
        assert_eq!(hex_bytes("zz"), None);
    }
}
