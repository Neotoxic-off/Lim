//! Raw stdin bytes → keys. Reads the fd directly so `select` sees exactly what is unread.

use std::io;
use std::time::Instant;

use super::sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Ctrl(char),
    /// Function keys F1–F12.
    F(u8),
    /// Ctrl + arrow (split resizing).
    CtrlUp,
    CtrlDown,
    CtrlLeft,
    CtrlRight,
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
                let (key, used) = match decode(&self.pending) {
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
            let wait = deadline.map(|d| d.saturating_duration_since(Instant::now()).as_millis().min(u128::from(u32::MAX)) as u32);
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
        let mut buf = [0u8; 4096];
        let n = match sys::read_stdin(&mut buf) {
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
    let first = *bytes.first()?;
    Some(match first {
        0x1b => return escape(bytes),
        // Raw mode leaves Enter as '\r'; 0x0a and 0x08 are Ctrl-j / Ctrl-h (window moves).
        b'\r' => (Key::Enter, 1),
        0x7f => (Key::Backspace, 1),
        b'\t' => (Key::Tab, 1),
        1..=26 => (Key::Ctrl(char::from(b'a' + first - 1)), 1),
        0x20..=0x7e => (Key::Char(char::from(first)), 1),
        0 => (Key::Ctrl(' '), 1),
        28..=31 => (Key::Unknown, 1),
        _ => {
            let len = match first {
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

fn escape(bytes: &[u8]) -> Option<(Key, usize)> {
    match bytes.get(1)? {
        b'[' | b'O' => {
            let end = 2 + bytes[2..].iter().position(|b| (0x40..=0x7e).contains(b))?;
            let key = match &bytes[2..=end] {
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
        assert_eq!(decode(b"\x1c"), Some((Key::Unknown, 1)));
    }
}
