//! Vim registers: unnamed `"`, named `a`–`z` (`A`–`Z` append), yank `0`, deletes `1`–`9` (shifting) and `-`,
//! black hole `_`, clipboard `+`/`*`, and read-only `%` (file) and `/` (search).

use std::collections::{HashMap, VecDeque};

use super::{Editor, clipboard};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Register {
    pub text: String,
    pub linewise: bool,
    /// From a visual-block selection: one entry per line, pasted as a column.
    pub block: bool,
}

/// `"1` to `"9`: the newest delete in front, the tenth dropped.
const NUMBERED: usize = 9;

#[derive(Debug, Default)]
pub struct Registers {
    slots: HashMap<char, Register>,
    /// `"1` … `"9`, newest first.
    numbered: VecDeque<Register>,
    /// Last text lim put on the system clipboard: when the clipboard still holds it, the
    /// register's own kind (linewise, block) is used instead of guessing from the text.
    copied: Option<String>,
    /// While `:g`, `:norm` or a macro runs (they nest), only the last text goes to the clipboard, once, at the end.
    clipboard_holds: usize,
    clipboard_pending: Option<Register>,
}

impl Registers {
    pub fn list(&self) -> Vec<(char, Register)> {
        let numbered = self.numbered.iter().zip('1'..='9').map(|(r, name)| (name, r.clone()));
        let mut list: Vec<(char, Register)> = self.slots.iter().map(|(k, v)| (*k, v.clone())).chain(numbered).collect();
        list.sort_by_key(|(k, _)| *k);
        list
    }

    fn numbered(&self, name: char) -> Option<&Register> {
        self.numbered.get(name.to_digit(10)?.checked_sub(1).and_then(|n| usize::try_from(n).ok())?)
    }
}

impl Editor {
    /// Records yanked (`yank`) or deleted text in the register chosen with `"x`, following Vim.
    pub(super) fn store_register(&mut self, value: Register, yank: bool) {
        let name: Option<char> = self.register_name.take();
        if name == Some('_') {
            return;
        }
        let registers: &mut Registers = &mut self.registers;
        match name {
            Some(c @ 'A'..='Z') => {
                let slot: &mut Register = registers.slots.entry(c.to_ascii_lowercase()).or_default();
                let sep: &str = if slot.linewise || value.linewise { "\n" } else { "" };
                slot.text = format!("{}{sep}{}", slot.text, value.text);
                slot.linewise |= value.linewise;
            }
            Some(c @ ('a'..='z' | '0' | '-')) => {
                registers.slots.insert(c, value.clone());
            }
            // `"3yy` writes `"3` itself, padding the list if it is shorter.
            Some(c @ '1'..='9') => {
                let index: usize = c.to_digit(10).and_then(|d| usize::try_from(d).ok()).unwrap_or(1) - 1;
                registers.numbered.resize(registers.numbered.len().max(index + 1), Register::default());
                registers.numbered[index] = value.clone();
            }
            _ => {}
        }
        if yank {
            registers.slots.insert('0', value.clone());
        } else if value.linewise || value.text.contains('\n') {
            registers.numbered.push_front(value.clone());
            registers.numbered.truncate(NUMBERED);
        } else {
            registers.slots.insert('-', value.clone());
        }
        if matches!(name, Some('+' | '*')) || (name.is_none() && self.options.clipboard) {
            if self.registers.clipboard_holds > 0 {
                self.registers.clipboard_pending = Some(value.clone());
            } else {
                self.set_clipboard(&value);
            }
        }
        self.registers.slots.insert('"', value);
    }

    /// From here until `release_clipboard`, the clipboard is written once instead of on every yank or delete
    /// (`:g/x/d` would otherwise run the clipboard tool once per line).
    pub(super) fn hold_clipboard(&mut self) {
        self.registers.clipboard_holds += 1;
    }

    pub(super) fn release_clipboard(&mut self) {
        self.registers.clipboard_holds = self.registers.clipboard_holds.saturating_sub(1);
        if self.registers.clipboard_holds == 0
            && let Some(value) = self.registers.clipboard_pending.take()
        {
            self.set_clipboard(&value);
        }
    }

    /// The register `"x` named, or the unnamed one (the clipboard with `clipboard=unnamedplus`).
    pub(super) fn fetch_register(&mut self) -> Option<Register> {
        let name: Option<char> = self.register_name.take();
        match name {
            Some('%') => {
                self.buf().path.as_ref().map(|p| Register { text: p.display().to_string(), ..Register::default() })
            }
            Some('/') => Some(Register { text: self.search.clone(), ..Register::default() }),
            Some('+' | '*') => self.clipboard_register(),
            Some(c @ '1'..='9') => self.registers.numbered(c).cloned(),
            Some(c) if c != '"' => self.registers.slots.get(&c.to_ascii_lowercase()).cloned(),
            _ if self.options.clipboard => self.clipboard_register(),
            _ => self.registers.slots.get(&'"').cloned(),
        }
    }

    fn clipboard_register(&self) -> Option<Register> {
        let unnamed: Option<Register> = self.registers.slots.get(&'"').cloned();
        match clipboard::paste() {
            // Tools may add or drop a final newline: compare without it.
            Some(text)
                if !text.is_empty()
                    && Some(text.trim_end_matches('\n'))
                        != self.registers.copied.as_deref().map(|c| c.trim_end_matches('\n')) =>
            {
                let linewise: bool = text.ends_with('\n');
                Some(Register { text: text.strip_suffix('\n').unwrap_or(&text).to_owned(), linewise, block: false })
            }
            _ => unnamed,
        }
    }

    fn set_clipboard(&mut self, value: &Register) {
        let text: String = if value.linewise { format!("{}\n", value.text) } else { value.text.clone() };
        if !clipboard::copy(&text) && !cfg!(test) {
            self.osc52 = Some(clipboard::osc52(&text));
        }
        self.registers.copied = Some(text);
    }
}
