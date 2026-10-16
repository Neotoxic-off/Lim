//! Char-indexed helpers over `str`: columns are char counts, never byte offsets.

use std::cmp::Ordering;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

// Text-level options (`tabstop`, `ignorecase`, `smartcase`), shared by buffers and the renderer.
static TAB_STOP: AtomicUsize = AtomicUsize::new(2);
static IGNORE_CASE: AtomicBool = AtomicBool::new(true);
static SMART_CASE: AtomicBool = AtomicBool::new(true);

pub fn configure(tabstop: usize, ignorecase: bool, smartcase: bool) {
    TAB_STOP.store(tabstop.max(1), Relaxed);
    IGNORE_CASE.store(ignorecase, Relaxed);
    SMART_CASE.store(smartcase, Relaxed);
}

/// Cells a tab spans at screen column `col`.
pub fn tab_cells(col: usize) -> usize {
    let stop: usize = TAB_STOP.load(Relaxed).max(1);
    stop - col % stop
}

pub fn char_len(s: &str) -> usize {
    if s.is_ascii() { s.len() } else { s.chars().count() }
}

pub fn byte_index(s: &str, col: usize) -> usize {
    if s.is_ascii() {
        return col.min(s.len());
    }
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

pub fn indent(s: &str) -> &str {
    &s[..s.len() - s.trim_start().len()]
}

/// Screen column of char `col`, tabs expanded.
pub fn display_col(s: &str, col: usize) -> usize {
    s.chars().take(col).fold(0, |w, c| if c == '\t' { w + tab_cells(w) } else { w + char_width(c) })
}

pub fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Terminal cells taken by `c`: 0 for combining marks, 2 for East Asian wide chars and emoji, else 1.
// shortcut: compact range table (wcwidth + Unicode wide emoji), not the full EastAsianWidth data.
pub fn char_width(c: char) -> usize {
    const ZERO: &[(u32, u32)] = &[
        (0x0300, 0x036F),
        (0x0483, 0x0489),
        (0x0591, 0x05BD),
        (0x0610, 0x061A),
        (0x064B, 0x065F),
        (0x1AB0, 0x1AFF),
        (0x1DC0, 0x1DFF),
        (0x200B, 0x200F),
        (0x20D0, 0x20FF),
        (0xFE00, 0xFE0F),
        (0xFE20, 0xFE2F),
        (0xE0100, 0xE01EF),
    ];
    const WIDE: &[(u32, u32)] = &[
        (0x1100, 0x115F),
        (0x231A, 0x231B),
        (0x2329, 0x232A),
        (0x23E9, 0x23EC),
        (0x23F0, 0x23F0),
        (0x23F3, 0x23F3),
        (0x25FD, 0x25FE),
        (0x2614, 0x2615),
        (0x2648, 0x2653),
        (0x267F, 0x267F),
        (0x2693, 0x2693),
        (0x26A1, 0x26A1),
        (0x26AA, 0x26AB),
        (0x26BD, 0x26BE),
        (0x26C4, 0x26C5),
        (0x26CE, 0x26CE),
        (0x26D4, 0x26D4),
        (0x26EA, 0x26EA),
        (0x26F2, 0x26F3),
        (0x26F5, 0x26F5),
        (0x26FA, 0x26FA),
        (0x26FD, 0x26FD),
        (0x2705, 0x2705),
        (0x270A, 0x270B),
        (0x2728, 0x2728),
        (0x274C, 0x274C),
        (0x274E, 0x274E),
        (0x2753, 0x2755),
        (0x2757, 0x2757),
        (0x2795, 0x2797),
        (0x27B0, 0x27B0),
        (0x27BF, 0x27BF),
        (0x2B1B, 0x2B1C),
        (0x2B50, 0x2B50),
        (0x2B55, 0x2B55),
        (0x2E80, 0x303E),
        (0x3040, 0xA4CF),
        (0xAC00, 0xD7A3),
        (0xF900, 0xFAFF),
        (0xFE10, 0xFE19),
        (0xFE30, 0xFE6F),
        (0xFF00, 0xFF60),
        (0xFFE0, 0xFFE6),
        (0x1F004, 0x1F004),
        (0x1F0CF, 0x1F0CF),
        (0x1F18E, 0x1F18E),
        (0x1F191, 0x1F19A),
        (0x1F200, 0x1F202),
        (0x1F210, 0x1F23B),
        (0x1F240, 0x1F248),
        (0x1F250, 0x1F251),
        (0x1F260, 0x1F265),
        (0x1F300, 0x1F64F),
        (0x1F680, 0x1F6FF),
        (0x1F7E0, 0x1F7EB),
        (0x1F90C, 0x1F9FF),
        (0x1FA70, 0x1FAFF),
        (0x20000, 0x2FFFD),
        (0x30000, 0x3FFFD),
    ];
    let u: u32 = u32::from(c);
    if u < 0x300 {
        return 1;
    }
    let within = |table: &[(u32, u32)]| {
        table
            .binary_search_by(|&(lo, hi)| {
                if hi < u {
                    Ordering::Less
                } else if lo > u {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            })
            .is_ok()
    };
    if within(ZERO) {
        0
    } else if within(WIDE) {
        2
    } else {
        1
    }
}

/// Char ranges of every match, following `ignorecase` / `smartcase`.
/// Vim's `\<` and `\>` word boundaries are honored at the pattern's start and end.
// shortcut: lowercasing that changes char counts (e.g. 'İ') shifts columns, fine for source code.
pub fn find_all(line: &str, pattern: &str) -> Vec<Range<usize>> {
    let (pattern, word_start): (&str, bool) = pattern.strip_prefix("\\<").map_or((pattern, false), |p| (p, true));
    let (pattern, word_end): (&str, bool) = pattern.strip_suffix("\\>").map_or((pattern, false), |p| (p, true));
    if pattern.is_empty() {
        return Vec::new();
    }
    let matches: Vec<Range<usize>> = find_plain(line, pattern);
    if !word_start && !word_end {
        return matches;
    }
    let chars: Vec<char> = line.chars().collect();
    let is_word = |i: usize| chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_');
    matches
        .into_iter()
        .filter(|m| (!word_start || m.start == 0 || !is_word(m.start - 1)) && (!word_end || !is_word(m.end)))
        .collect()
}

/// Whole-word, case-sensitive matches of `word` (the word under the cursor).
pub fn find_word(line: &str, word: &str) -> Vec<Range<usize>> {
    if word.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let width: usize = char_len(word);
    let is_word = |i: usize| chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_');
    let (mut byte, mut col): (usize, usize) = (0, 0);
    line.match_indices(word)
        .map(|(b, _)| {
            col += char_len(&line[byte..b]);
            byte = b;
            col..col + width
        })
        .filter(|m| (m.start == 0 || !is_word(m.start - 1)) && !is_word(m.end))
        .collect()
}

/// URLs (`http://`, `https://`, `file://`) as char ranges, for `highlighturl`.
pub fn find_urls(line: &str) -> Vec<Range<usize>> {
    let chars: Vec<char> = line.chars().collect();
    let mut urls: Vec<Range<usize>> = Vec::new();
    let mut i: usize = 0;
    while i < chars.len() {
        let rest: String = chars[i..chars.len().min(i + 8)].iter().collect();
        if ["https://", "http://", "file://"].iter().any(|p| rest.starts_with(p)) {
            let start: usize = i;
            while i < chars.len() && !chars[i].is_whitespace() && !"\"'<>`".contains(chars[i]) {
                i += 1;
            }
            // Trailing punctuation usually ends the sentence, not the URL.
            while i > start && ".,;:!?)]}".contains(chars[i - 1]) {
                i -= 1;
            }
            urls.push(start..i);
            i += 1;
        } else {
            i += 1;
        }
    }
    urls
}

/// `#RRGGBB` color literals (highlight-colors) as char ranges with their value.
pub fn find_hex_colors(line: &str) -> Vec<(Range<usize>, u32)> {
    let chars: Vec<char> = line.chars().collect();
    let mut found: Vec<(Range<usize>, u32)> = Vec::new();
    for i in 0..chars.len() {
        let digits: Option<&[char]> = chars.get(i + 1..i + 7);
        if chars[i] == '#'
            && let Some(digits) = digits
            && digits.iter().all(char::is_ascii_hexdigit)
            && !chars.get(i + 7).is_some_and(char::is_ascii_alphanumeric)
            && let Ok(value) = u32::from_str_radix(&digits.iter().collect::<String>(), 16)
        {
            found.push((i..i + 7, value));
        }
    }
    found
}

fn find_plain(line: &str, pattern: &str) -> Vec<Range<usize>> {
    let smart_sensitive: bool = SMART_CASE.load(Relaxed) && pattern.chars().any(char::is_uppercase);
    let (lowered_line, lowered_pattern);
    let (hay, pattern): (&str, &str) = if IGNORE_CASE.load(Relaxed) && !smart_sensitive {
        lowered_line = line.to_lowercase();
        lowered_pattern = pattern.to_lowercase();
        (lowered_line.as_str(), lowered_pattern.as_str())
    } else {
        (line, pattern)
    };
    let width: usize = char_len(pattern);
    let (mut byte, mut col): (usize, usize) = (0, 0);
    hay.match_indices(pattern)
        .map(|(b, _)| {
            col += char_len(&hay[byte..b]);
            byte = b;
            col..col + width
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_and_matches() {
        assert_eq!(byte_index("héllo", 2), 3);
        assert_eq!(display_col("\tab", 2), 3);
        assert_eq!(display_col("日本x", 2), 4);
        assert_eq!(str_width("e\u{301}🦀a"), 4);
        assert_eq!(char_width('\u{e7a8}'), 1); // Nerd Font icons are single-cell
        assert_eq!(find_all("Foo bar foo", "foo"), vec![0..3, 8..11]);
        assert_eq!(find_all("Foo bar foo", "Foo"), vec![0..3]);
        assert_eq!(find_all("é foo", "foo"), vec![2..5]);
        assert_eq!(find_all("foo food _foo foo", "\\<foo\\>"), vec![0..3, 14..17]);
        assert_eq!(find_word("Foo foo food", "foo"), vec![4..7]);
        assert_eq!(find_hex_colors("a #1A1D23 #12345 #abcdefg"), vec![(2..9, 0x1A1D23)]);
        assert_eq!(find_urls("see https://x.io/a?b=1). and http://y"), vec![4..22, 29..37]);
    }
}
