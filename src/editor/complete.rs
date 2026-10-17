//! blink.cmp's buffer source: words from the open buffers, offered while typing in insert mode.
//! Keys as `AstroNvim` maps them: `<Tab>`/`<C-n>`/`<C-j>`/`<Down>` next, `<S-Tab>`/`<C-p>`/`<C-k>`/`<Up>`
//! previous, `<CR>` accept (only when an item is selected), `<C-e>` hide, `<C-Space>` show.

use std::collections::HashSet;
use std::iter;

use super::{Editor, Mode};
use crate::buffer::{Buffer, CharClass, Pos, text};
use crate::term::Key;

/// Items kept per request; the menu shows the first ones (`pumheight = 10`).
const MAX_ITEMS: usize = 50;
pub const MENU_HEIGHT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    /// Where the word being completed starts.
    pub start: Pos,
    pub prefix: String,
    pub items: Vec<String>,
    /// `completeopt=noselect`: nothing is selected until you move.
    pub selected: Option<usize>,
}

fn is_word(c: char) -> bool {
    CharClass::of(Some(c)) == CharClass::Word
}

/// Candidates for `prefix` from word lists (current buffer first), smartcase, best first.
pub fn candidates<'a>(prefix: &str, sources: impl Iterator<Item = &'a [String]>) -> Vec<String> {
    let sensitive: bool = prefix.chars().any(char::is_uppercase);
    // Compared char by char: lowercasing every word of every buffer on each key was the slow part.
    let matches = |w: &str| {
        if sensitive {
            return w.starts_with(prefix);
        }
        let mut word = w.chars().flat_map(char::to_lowercase);
        prefix.chars().flat_map(char::to_lowercase).all(|p| word.next() == Some(p))
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut found: Vec<(usize, String)> = Vec::new();
    for (rank, words) in sources.enumerate() {
        for w in words.iter().filter(|w| w.as_str() != prefix && w.len() > prefix.len() && matches(w)) {
            if seen.insert(w.clone()) {
                found.push((rank, w.clone()));
            }
        }
    }
    // Same-case prefix first, then the current buffer, then shorter words.
    found.sort_by_key(|(rank, w)| (!w.starts_with(prefix), *rank, w.chars().count(), w.clone()));
    found.into_iter().map(|(_, w)| w).take(MAX_ITEMS).collect()
}

impl Editor {
    /// Refreshes (or closes) the menu for the word before the cursor; `force` shows it even for
    /// an empty prefix result (`<C-Space>`).
    pub(super) fn update_completion(&mut self, force: bool) {
        // Replayed keys (`.`, macros, `:norm`) open no menu: nobody sees it, and it would cost a scan per key.
        if (!self.options.completion && !force) || self.replaying_keys() {
            self.completion = None;
            return;
        }
        let buf: &Buffer = self.buf();
        let at: Pos = buf.cursor;
        let before: Vec<char> = buf.line(at.line).chars().take(at.col).collect();
        let len: usize = before.iter().rev().take_while(|c| is_word(**c)).count();
        if len == 0 {
            self.completion = None;
            return;
        }
        let prefix: String = before[before.len() - len..].iter().collect();
        let current: usize = self.current;
        for buf in &mut self.buffers {
            buf.refresh_words();
        }
        // Current buffer first, the others after, without copying their lists.
        let order = iter::once(current).chain((0..self.buffers.len()).filter(|&i| i != current));
        let items: Vec<String> = candidates(&prefix, order.map(|i| self.buffers[i].words()));
        self.completion = (!items.is_empty()).then(|| Completion {
            start: Pos::new(at.line, at.col - len),
            prefix,
            items,
            selected: None,
        });
    }

    /// Brings the menu up to date with what was typed since it was last computed.
    pub fn refresh_completion(&mut self) {
        if std::mem::take(&mut self.completion_stale) {
            self.update_completion(false);
        }
    }

    /// Keys the open menu takes; false lets insert mode handle the key.
    pub(super) fn completion_key(&mut self, key: Key) -> bool {
        // Only the keys that act on the menu need it current; `<Esc>` and typing do not.
        if matches!(
            key,
            Key::Tab | Key::BackTab | Key::Down | Key::Up | Key::Enter | Key::Ctrl('n' | 'j' | 'p' | 'k' | 'e')
        ) {
            self.refresh_completion();
        }
        if key == Key::Ctrl(' ') {
            self.update_completion(true);
            return true;
        }
        let Some(menu): &mut Option<Completion> = &mut self.completion else { return false };
        let count: usize = menu.items.len();
        match key {
            Key::Tab | Key::Ctrl('n' | 'j') | Key::Down => {
                menu.selected = Some(menu.selected.map_or(0, |i| (i + 1) % count));
                true
            }
            Key::BackTab | Key::Ctrl('p' | 'k') | Key::Up => {
                menu.selected = Some(menu.selected.map_or(count - 1, |i| (i + count - 1) % count));
                true
            }
            Key::Ctrl('e') => {
                self.completion = None;
                true
            }
            Key::Enter if menu.selected.is_some() => {
                self.accept_completion();
                true
            }
            Key::Enter | Key::Esc | Key::Ctrl('c') => {
                self.completion = None;
                false
            }
            _ => false,
        }
    }

    fn accept_completion(&mut self) {
        let Some(menu): Option<Completion> = self.completion.take() else { return };
        let Some(word): Option<String> = menu.selected.and_then(|i| menu.items.get(i)).cloned() else { return };
        let buf: &mut Buffer = self.buf_mut();
        let end: Pos = buf.cursor;
        if end.line == menu.start.line && end.col >= menu.start.col {
            buf.remove(menu.start, end);
            buf.cursor = buf.insert(menu.start, &word);
        }
        self.mode = Mode::Insert;
    }
}

/// Unique words of at least two chars, in first-seen order.
pub fn words_of(lines: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for line in lines {
        let mut word: String = String::new();
        for c in line.chars().chain(iter::once(' ')) {
            if is_word(c) {
                word.push(c);
            } else if !word.is_empty() {
                if text::char_len(&word) >= 2 && seen.insert(word.clone()) {
                    words.push(word.clone());
                }
                word.clear();
            }
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_candidates() {
        let here: Vec<String> = words_of(&["let restock = Restore; res".into()]);
        let other: Vec<String> = words_of(&["result resolve restock".into()]);
        assert_eq!(here, vec!["let", "restock", "Restore", "res"]);
        let found: Vec<String> = candidates("res", [here.as_slice(), other.as_slice()].into_iter());
        assert_eq!(found, vec!["restock", "result", "resolve", "Restore"]);
        assert_eq!(candidates("Res", [here.as_slice()].into_iter()), vec!["Restore"]);
    }
}
