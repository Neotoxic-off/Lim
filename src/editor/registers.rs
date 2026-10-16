//! Vim registers: unnamed `"`, named `a`–`z` (`A`–`Z` append), yank `0`, delete `1` and `-`,
//! black hole `_`, clipboard `+`/`*`, and read-only `%` (file) and `/` (search).

use std::collections::HashMap;

use super::{Editor, clipboard};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Register {
    pub text: String,
    pub linewise: bool,
    /// From a visual-block selection: one entry per line, pasted as a column.
    pub block: bool,
}

#[derive(Debug, Default)]
pub struct Registers {
    slots: HashMap<char, Register>,
    /// Last text lim put on the system clipboard: when the clipboard still holds it, the
    /// register's own kind (linewise, block) is used instead of guessing from the text.
    copied: Option<String>,
}

impl Registers {
    pub fn list(&self) -> Vec<(char, Register)> {
        let mut list: Vec<_> = self.slots.iter().map(|(k, v)| (*k, v.clone())).collect();
        list.sort_by_key(|(k, _)| *k);
        list
    }
}

impl Editor {
    /// Records yanked (`yank`) or deleted text in the register chosen with `"x`, following Vim.
    pub(super) fn store_register(&mut self, value: Register, yank: bool) {
        let name: Option<char> = self.register_name.take();
        if name == Some('_') {
            return;
        }
        let slots: &mut HashMap<char, Register> = &mut self.registers.slots;
        match name {
            Some(c @ 'A'..='Z') => {
                let slot: &mut Register = slots.entry(c.to_ascii_lowercase()).or_default();
                let sep: &str = if slot.linewise || value.linewise { "\n" } else { "" };
                slot.text = format!("{}{sep}{}", slot.text, value.text);
                slot.linewise |= value.linewise;
            }
            Some(c @ ('a'..='z' | '0'..='9' | '-')) => {
                slots.insert(c, value.clone());
            }
            _ => {}
        }
        let numbered: char = if yank {
            '0'
        } else if value.linewise || value.text.contains('\n') {
            '1'
        } else {
            '-'
        };
        slots.insert(numbered, value.clone());
        if matches!(name, Some('+' | '*')) || (name.is_none() && self.options.clipboard) {
            self.set_clipboard(&value);
        }
        self.registers.slots.insert('"', value);
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
