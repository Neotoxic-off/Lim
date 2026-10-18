//! The hex view of a binary file: `00000000: 48 65 6c 6c 6f 0a 00 00  00 … |Hello...|`, 16 bytes a line.
//! Only the hex digits between `:` and `|` count when it is written back, so bytes can be changed, inserted or
//! deleted there; the offsets and the ASCII column are just for reading and are redrawn by the next dump.

use std::fmt::Write;

pub const WIDTH: usize = 16;

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// `bytes` as dump lines, the first one labelled `start`.
pub fn dump(bytes: &[u8], start: usize) -> Vec<String> {
    if bytes.is_empty() {
        return vec![String::new()];
    }
    bytes
        .chunks(WIDTH)
        .enumerate()
        .map(|(n, chunk)| {
            let mut line: String = String::with_capacity(78);
            let _: std::fmt::Result = write!(line, "{:08x}:", start + n * WIDTH);
            for i in 0..WIDTH {
                if i == WIDTH / 2 {
                    line.push(' ');
                }
                match chunk.get(i) {
                    Some(b) => {
                        line.push(' ');
                        line.push(char::from(DIGITS[usize::from(b >> 4)]));
                        line.push(char::from(DIGITS[usize::from(b & 15)]));
                    }
                    None => line.push_str("   "),
                }
            }
            line.push_str("  |");
            line.extend(chunk.iter().map(|b| if b.is_ascii_graphic() || *b == b' ' { char::from(*b) } else { '.' }));
            line.push('|');
            line
        })
        .collect()
}

/// Where the hex digits of a dump line are: after the offset's `:`, up to the `|` of the ASCII column.
fn hex_area(line: &str) -> (usize, usize) {
    let end: usize = line.find('|').unwrap_or(line.len());
    let start: usize = line[..end].find(':').map_or(0, |i| i + 1);
    (start, end)
}

/// The bytes of dump `lines`, or the first line (0-based) that is not hex and why.
pub fn undump<S: AsRef<str>>(lines: &[S]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(lines.len() * WIDTH);
    for (n, line) in lines.iter().enumerate() {
        let line: &str = line.as_ref();
        let (start, end): (usize, usize) = hex_area(line);
        let mut high: Option<u8> = None;
        for c in line[start..end].chars().filter(|c| !c.is_whitespace()) {
            let digit: u8 = c
                .to_digit(16)
                .and_then(|d| u8::try_from(d).ok())
                .ok_or_else(|| format!("line {}: '{c}' is not a hex digit", n + 1))?;
            match high.take() {
                Some(h) => out.push(h << 4 | digit),
                None => high = Some(digit),
            }
        }
        if high.is_some() {
            return Err(format!("line {}: odd number of hex digits", n + 1));
        }
    }
    Ok(out)
}

/// Which byte of a dump line the char column `col` is on: the hex pair it touches, or its ASCII column cell.
pub fn byte_in_line(line: &str, col: usize) -> usize {
    let (start, end): (usize, usize) = hex_area(line);
    let byte_col: usize = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    if byte_col > end {
        return line[end + 1..byte_col].chars().count();
    }
    line[start.min(byte_col)..byte_col].chars().filter(char::is_ascii_hexdigit).count() / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_and_reads_back() {
        let bytes: Vec<u8> = (0..=255).chain([0, 1, 2]).collect();
        let lines: Vec<String> = dump(&bytes, 0);
        assert_eq!(lines.len(), 17);
        assert_eq!(lines[4], "00000040: 40 41 42 43 44 45 46 47  48 49 4a 4b 4c 4d 4e 4f  |@ABCDEFGHIJKLMNO|");
        assert_eq!(lines[16], "00000100: 00 01 02                                          |...|");
        assert_eq!(undump(&lines).unwrap(), bytes);
        assert_eq!(dump(&[], 0), [""]);
        assert_eq!(undump(&[""]).unwrap(), b"");
        assert_eq!(dump(b"a", 32)[0], "00000020: 61                                                |a|");
    }

    /// The ASCII column and the offsets are ignored, so `|` and `:` inside it are harmless; spacing is free.
    #[test]
    fn reads_edited_lines() {
        assert_eq!(undump(&["00000000: 4142 43  |x:|y|", "zz: 44", "45 46"]).unwrap(), b"ABCDEF");
        assert_eq!(undump(&["00: 4g"]).unwrap_err(), "line 1: 'g' is not a hex digit");
        assert_eq!(undump(&["", "00: 414"]).unwrap_err(), "line 2: odd number of hex digits");
    }

    #[test]
    fn finds_the_byte_under_a_column() {
        let line: &str = &dump(b"0123456789abcdef", 0)[0];
        assert_eq!(byte_in_line(line, 0), 0);
        assert_eq!(byte_in_line(line, 10), 0);
        assert_eq!(byte_in_line(line, 11), 0);
        assert_eq!(byte_in_line(line, 13), 1);
        assert_eq!(byte_in_line(line, 35), 8, "after the middle gap");
        assert_eq!(byte_in_line(line, 61), 0, "the ASCII column");
        assert_eq!(byte_in_line(line, 63), 2);
        assert_eq!(byte_in_line("", 5), 0);
    }
}
