//! Raw stdin bytes → keys. Reads the fd directly so `select` sees exactly what is unread.

use std::io;
use std::time::Instant;

use super::sys;

/// A mouse report (SGR mode, `\x1b[<b;x;yM`): what happened, at which 0-based screen cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouse {
    pub kind: MouseKind,
    pub col: usize,
    pub row: usize,
}

/// The left button and the wheel; other buttons are not reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    Press,
    Drag,
    Release,
    ScrollUp,
    ScrollDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Mouse(Mouse),
    Char(char),
    Ctrl(char),
    /// Function keys F1–F12.
    F(u8),
    /// Ctrl + arrow (split resizing).
    CtrlUp,
    CtrlDown,
    CtrlLeft,
    CtrlRight,
    /// Shift + arrow (word and page moves, as in Vim).
    ShiftUp,
    ShiftDown,
    ShiftLeft,
    ShiftRight,
    Esc,
    Enter,
    Backspace,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    PageUp,
    PageDown,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    Resize,
    /// The deadline passed without input.
    Timeout,
}

/// How long to wait for the rest of an escape sequence before taking a lone Esc.
const ESC_TIMEOUT_MS: u32 = 25;

#[derive(Default)]
pub struct Input {
    pending: Vec<u8>,
}

impl Input {
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Next key or resize; `Timeout` once `deadline` passes with nothing typed.
    pub fn read(&mut self, deadline: Option<Instant>) -> io::Result<Event> {
        loop {
            if !self.pending.is_empty() {
                let (key, used): (Key, usize) = match decode(&self.pending) {
                    Some(decoded) => decoded,
                    None if self.fill(Some(ESC_TIMEOUT_MS))? => continue,
                    None if self.pending[0] == 0x1b => (Key::Esc, 1),
                    None => (Key::Unknown, 1),
                };
                self.pending.drain(..used);
                return Ok(Event::Key(key));
            }
            if sys::take_resized() {
                return Ok(Event::Resize);
            }
            let wait: Option<u32> = deadline
                .map(|d| u32::try_from(d.saturating_duration_since(Instant::now()).as_millis()).unwrap_or(u32::MAX));
            if wait == Some(0) {
                return Ok(Event::Timeout);
            }
            if !self.fill(wait)? && deadline.is_some_and(|d| Instant::now() >= d) {
                return Ok(Event::Timeout);
            }
        }
    }

    /// Appends what stdin has; Ok(false) when nothing arrived (timeout or signal).
    fn fill(&mut self, timeout_ms: Option<u32>) -> io::Result<bool> {
        if !sys::wait_readable(timeout_ms)? {
            return Ok(false);
        }
        let mut buf: [u8; 4096] = [0u8; 4096];
        let n: usize = match sys::read_stdin(&mut buf) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Ok(false),
            r => r?,
        };
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stdin closed"));
        }
        self.pending.extend_from_slice(&buf[..n]);
        Ok(true)
    }
}

/// One key and the bytes it used; None while a sequence may still be incomplete.
fn decode(bytes: &[u8]) -> Option<(Key, usize)> {
    let first: u8 = *bytes.first()?;
    Some(match first {
        0x1b => return escape(bytes),
        // Raw mode leaves Enter as '\r'; 0x0a and 0x08 are Ctrl-j / Ctrl-h (window moves).
        b'\r' => (Key::Enter, 1),
        0x7f => (Key::Backspace, 1),
        b'\t' => (Key::Tab, 1),
        1..=26 => (Key::Ctrl(char::from(b'a' + first - 1)), 1),
        0x20..=0x7e => (Key::Char(char::from(first)), 1),
        0 => (Key::Ctrl(' '), 1),
        // Ctrl-^ (Ctrl-6 on most keyboards): the alternate buffer.
        30 => (Key::Ctrl('^'), 1),
        28 | 29 | 31 => (Key::Unknown, 1),
        _ => {
            let len: usize = match first {
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf7 => 4,
                _ => return Some((Key::Unknown, 1)),
            };
            if bytes.len() < len {
                return None;
            }
            match std::str::from_utf8(&bytes[..len]).ok().and_then(|s| s.chars().next()) {
                Some(c) => (Key::Char(c), len),
                None => (Key::Unknown, 1),
            }
        }
    })
}

/// `<button;col;row` then `M` (press, drag, wheel) or `m` (release); 1-based cells.
fn mouse(seq: &[u8]) -> Option<Key> {
    let (&end, body): (&u8, &[u8]) = seq.split_last()?;
    let mut fields = std::str::from_utf8(body.get(1..)?).ok()?.split(';').map(|f| f.parse::<usize>().ok());
    let (button, col, row): (usize, usize, usize) = (fields.next()??, fields.next()??, fields.next()??);
    let kind: MouseKind = match (button, end) {
        (64, _) => MouseKind::ScrollUp,
        (65, _) => MouseKind::ScrollDown,
        (32, _) => MouseKind::Drag,
        (0, b'M') => MouseKind::Press,
        (0, b'm') => MouseKind::Release,
        _ => return None,
    };
    Some(Key::Mouse(Mouse { kind, col: col.checked_sub(1)?, row: row.checked_sub(1)? }))
}

fn escape(bytes: &[u8]) -> Option<(Key, usize)> {
    match bytes.get(1)? {
        b'[' | b'O' => {
            let end: usize = 2 + bytes[2..].iter().position(|b| (0x40..=0x7e).contains(b))?;
            let key: Key = match &bytes[2..=end] {
                b"A" => Key::Up,
                b"B" => Key::Down,
                b"C" => Key::Right,
                b"D" => Key::Left,
                b"H" | b"1~" | b"7~" => Key::Home,
                b"F" | b"4~" | b"8~" => Key::End,
                b"Z" => Key::BackTab,
                b"3~" => Key::Delete,
                b"5~" => Key::PageUp,
                b"6~" => Key::PageDown,
                b"1;5A" => Key::CtrlUp,
                b"1;5B" => Key::CtrlDown,
                b"1;5C" => Key::CtrlRight,
                b"1;5D" => Key::CtrlLeft,
                b"1;2A" => Key::ShiftUp,
                b"1;2B" => Key::ShiftDown,
                b"1;2C" => Key::ShiftRight,
                b"1;2D" => Key::ShiftLeft,
                b"P" => Key::F(1),
                b"Q" => Key::F(2),
                b"R" => Key::F(3),
                b"S" => Key::F(4),
                b"15~" => Key::F(5),
                b"17~" => Key::F(6),
                b"18~" => Key::F(7),
                b"19~" => Key::F(8),
                b"20~" => Key::F(9),
                b"21~" => Key::F(10),
                b"23~" => Key::F(11),
                b"24~" => Key::F(12),
                seq if seq.starts_with(b"<") => mouse(seq).unwrap_or(Key::Unknown),
                _ => Key::Unknown,
            };
            Some((key, end + 1))
        }
        // Esc followed by another key (Alt chords or fast typing): take the Esc alone.
        _ => Some((Key::Esc, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Any bytes the terminal sends (escape sequences cut anywhere, UTF-8 halves, garbage) decode without
    /// panicking, and every key consumes at least one byte and no more than there are.
    #[test]
    fn decodes_any_bytes() {
        let pieces: &[&[u8]] = &[
            b"\x1b",
            b"[",
            b"O",
            b"1;5A",
            b"A",
            b"~",
            b"3",
            b";",
            b"2",
            b"<",
            b"M",
            b"\x7f",
            b"\r",
            b"\t",
            b"x",
            b"\x00",
            b"\x1e",
            "é".as_bytes(),
            &[0xe6, 0x97],
            &[0xff],
            &[0xc3],
        ];
        let mut state: u64 = 17;
        let mut next = |n: usize| {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(state >> 33).unwrap_or(0) % n
        };
        for _ in 0..20_000 {
            let bytes: Vec<u8> = (0..next(12)).flat_map(|_| pieces[next(pieces.len())].iter().copied()).collect();
            let mut rest: &[u8] = &bytes;
            while let Some((_, used)) = decode(rest) {
                assert!((1..=rest.len()).contains(&used), "{bytes:?}: used {used} of {}", rest.len());
                rest = &rest[used..];
            }
        }
    }

    #[test]
    fn decodes_keys() {
        assert_eq!(decode(b"\x1b[A"), Some((Key::Up, 3)));
        assert_eq!(decode(b"\x1b[3~x"), Some((Key::Delete, 4)));
        assert_eq!(decode(b"\x1b[Z"), Some((Key::BackTab, 3)));
        assert_eq!(decode(b"\x1b[18~"), Some((Key::F(7), 5)));
        assert_eq!(decode(b"\x1bj"), Some((Key::Esc, 1)));
        assert_eq!(decode(b"\x1b"), None);
        assert_eq!(decode(b"\x1b[1"), None);
        assert_eq!(decode("é".as_bytes()), Some((Key::Char('é'), 2)));
        assert_eq!(decode(&"é".as_bytes()[..1]), None);
        assert_eq!(decode(b"\x13"), Some((Key::Ctrl('s'), 1)));
        assert_eq!(decode(b"\x1e"), Some((Key::Ctrl('^'), 1)));
        let click = |kind: MouseKind| Key::Mouse(Mouse { kind, col: 9, row: 4 });
        assert_eq!(decode(b"\x1b[<0;10;5M"), Some((click(MouseKind::Press), 10)));
        assert_eq!(decode(b"\x1b[<0;10;5m"), Some((click(MouseKind::Release), 10)));
        assert_eq!(decode(b"\x1b[<32;10;5M"), Some((click(MouseKind::Drag), 11)));
        assert_eq!(decode(b"\x1b[<65;10;5M"), Some((click(MouseKind::ScrollDown), 11)));
        assert_eq!(decode(b"\x1b[<2;10;5M"), Some((Key::Unknown, 10)), "right button");
        assert_eq!(decode(b"\x1c"), Some((Key::Unknown, 1)));
    }

    #[test]
    fn decodes_plain_and_modified_keys() {
        let cases: [(&[u8], Key); 14] = [
            (b"\x1b[1;2A", Key::ShiftUp),
            (b"\x1b[1;2D", Key::ShiftLeft),
            (b"\r", Key::Enter),
            (b"\x7f", Key::Backspace),
            (b"\t", Key::Tab),
            (b"\0", Key::Ctrl(' ')),
            (b"\x1b[1;5C", Key::CtrlRight),
            (b"\x1bOP", Key::F(1)),
            (b"\x1b[24~", Key::F(12)),
            (b"\x1b[H", Key::Home),
            (b"\x1b[4~", Key::End),
            (b"\x1b[6~", Key::PageDown),
            (b"\x1b[99~", Key::Unknown),
            ("日".as_bytes(), Key::Char('日')),
        ];
        for (bytes, key) in cases {
            assert_eq!(decode(bytes), Some((key, bytes.len())), "{bytes:?}");
        }
        assert_eq!(decode(b"\xc3("), Some((Key::Unknown, 1)), "invalid UTF-8");
        assert_eq!(decode(b"\xff"), Some((Key::Unknown, 1)));
        assert_eq!(decode(b""), None);
    }
}
