//! Vim key notation (`<C-s>`, `<Leader>w`, `jk`, `:w<CR>`) → keys.

use std::str::Chars;

use crate::term::Key;

/// Internal stand-in for the leader key, so `mapleader` can be any key without clashing.
pub const LEADER: char = '\u{F8FF}';

pub fn parse(notation: &str) -> Result<Vec<Key>, String> {
    let chars: Vec<char> = notation.chars().collect();
    let mut keys: Vec<Key> = Vec::new();
    let mut i: usize = 0;
    while i < chars.len() {
        if chars[i] == '<'
            && let Some(len) = chars[i..].iter().position(|&c| c == '>')
            && let Some(key) = special(&chars[i + 1..i + len].iter().collect::<String>())
        {
            keys.push(key);
            i += len + 1;
            continue;
        }
        keys.push(Key::Char(chars[i]));
        i += 1;
    }
    if keys.is_empty() {
        return Err("empty key sequence".into());
    }
    Ok(keys)
}

/// The leader as written in `mapleader`: one char or one `<…>` name.
pub fn parse_one(notation: &str) -> Result<Key, String> {
    match parse(notation)?.as_slice() {
        [key] => Ok(*key),
        _ => Err(format!("\"{notation}\" is not a single key")),
    }
}

fn special(name: &str) -> Option<Key> {
    let lower: String = name.to_ascii_lowercase();
    Some(match lower.as_str() {
        "leader" => Key::Char(LEADER),
        "cr" | "enter" | "return" => Key::Enter,
        "esc" => Key::Esc,
        "bs" | "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "s-tab" => Key::BackTab,
        "space" => Key::Char(' '),
        "lt" => Key::Char('<'),
        "bar" => Key::Char('|'),
        "bslash" => Key::Char('\\'),
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "del" | "delete" => Key::Delete,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "c-up" => Key::CtrlUp,
        "c-down" => Key::CtrlDown,
        "c-left" => Key::CtrlLeft,
        "c-right" => Key::CtrlRight,
        "s-up" => Key::ShiftUp,
        "s-down" => Key::ShiftDown,
        "s-left" => Key::ShiftLeft,
        "s-right" => Key::ShiftRight,
        _ => {
            if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=12).contains(n))
            {
                return Some(Key::F(n));
            }
            if lower == "c-space" {
                return Some(Key::Ctrl(' '));
            }
            let c: &str = lower.strip_prefix("c-")?;
            let mut it: Chars<'_> = c.chars();
            match (it.next(), it.next()) {
                (Some(c), None) if c.is_ascii_lowercase() => Key::Ctrl(c),
                _ => return None,
            }
        }
    })
}

/// Back to notation, for which-key and error messages.
pub fn display(keys: &[Key]) -> String {
    keys.iter()
        .map(|k| match k {
            Key::Char(LEADER) => "<Leader>".to_owned(),
            Key::Char(' ') => "<Space>".to_owned(),
            Key::Char(c) => c.to_string(),
            Key::Ctrl(' ') => "<C-Space>".into(),
            Key::Ctrl(c) => format!("<C-{c}>"),
            Key::F(n) => format!("<F{n}>"),
            Key::Esc => "<Esc>".into(),
            Key::Enter => "<CR>".into(),
            Key::Backspace => "<BS>".into(),
            Key::Tab => "<Tab>".into(),
            Key::BackTab => "<S-Tab>".into(),
            Key::Up => "<Up>".into(),
            Key::Down => "<Down>".into(),
            Key::Left => "<Left>".into(),
            Key::Right => "<Right>".into(),
            Key::Home => "<Home>".into(),
            Key::End => "<End>".into(),
            Key::Delete => "<Del>".into(),
            Key::PageUp => "<PageUp>".into(),
            Key::PageDown => "<PageDown>".into(),
            Key::CtrlUp => "<C-Up>".into(),
            Key::CtrlDown => "<C-Down>".into(),
            Key::CtrlLeft => "<C-Left>".into(),
            Key::CtrlRight => "<C-Right>".into(),
            Key::ShiftUp => "<S-Up>".into(),
            Key::ShiftDown => "<S-Down>".into(),
            Key::ShiftLeft => "<S-Left>".into(),
            Key::ShiftRight => "<S-Right>".into(),
            Key::Unknown => "?".into(),
            Key::Mouse(_) => "<Mouse>".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation() {
        assert_eq!(parse(":w<CR>"), Ok(vec![Key::Char(':'), Key::Char('w'), Key::Enter]));
        assert_eq!(parse("<leader>fF7"), Ok(vec![Key::Char(LEADER), Key::Char('f'), Key::Char('F'), Key::Char('7')]));
        assert_eq!(
            parse("<C-S><F7><lt>x>"),
            Ok(vec![Key::Ctrl('s'), Key::F(7), Key::Char('<'), Key::Char('x'), Key::Char('>')])
        );
        assert_eq!(parse_one("<Space>"), Ok(Key::Char(' ')));
        assert!(parse_one("ab").is_err());
        assert_eq!(display(&parse("<Leader>b<C-w>").unwrap_or_default()), "<Leader>b<C-w>");
    }
}
