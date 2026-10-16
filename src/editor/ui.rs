//! which-key menus and AstroNvim's `<Leader>u` UI toggles.

use super::{Editor, Message, Mode};
use crate::config::keys::{self, LEADER};
use crate::term::Key;

/// Internal prefix slots for leader sub-menus (`<Leader>b…`, `<Leader>f…`, …).
pub const LEADER_B: char = '\u{F8F0}';
pub const LEADER_F: char = '\u{F8F1}';
pub const LEADER_U: char = '\u{F8F2}';
pub const LEADER_T: char = '\u{F8F3}';
pub const LEADER_G: char = '\u{F8F4}';
pub const CTRL_W: char = '\u{17}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub key: String,
    pub desc: String,
    /// Opens another menu (`+Buffers`).
    pub group: bool,
}

/// AstroNvim's descriptions for the built-in prefixes.
fn builtin(prefix: char) -> (&'static str, &'static [(&'static str, &'static str)]) {
    match prefix {
        LEADER => ("<Leader>", &[
            ("b", "+Buffers"),
            ("f", "+Find"),
            ("g", "+Git"),
            ("t", "+Terminal"),
            ("u", "+UI/UI Toggles"),
            ("c", "Close buffer"),
            ("C", "Force close buffer"),
            ("e", "Toggle Explorer"),
            ("o", "Toggle Explorer Focus"),
            ("h", "Home Screen"),
            ("n", "New File"),
            ("q", "Quit Window"),
            ("Q", "Exit lim"),
            ("w", "Save"),
            ("/", "Toggle comment line"),
        ]),
        LEADER_B => ("<Leader>b", &[
            ("b", "Select buffer from tabline"),
            ("c", "Close all buffers except current"),
            ("C", "Close all buffers"),
            ("d", "Close buffer from tabline"),
            ("l", "Close all buffers to the left"),
            ("p", "Previous buffer"),
            ("r", "Close all buffers to the right"),
            ("\\", "Horizontal split buffer from tabline"),
            ("|", "Vertical split buffer from tabline"),
        ]),
        LEADER_F => ("<Leader>f", &[
            ("<CR>", "Resume previous search"),
            ("b", "Find buffers"),
            ("c", "Find word under cursor"),
            ("C", "Find commands"),
            ("f", "Find files"),
            ("F", "Find all files"),
            ("g", "Find git files"),
            ("k", "Find keymaps"),
            ("l", "Find lines"),
            ("o", "Find old files"),
            ("w", "Find words"),
            ("W", "Find words in all files"),
        ]),
        LEADER_G => ("<Leader>g", &[("g", "Next Git hunk"), ("G", "Previous Git hunk")]),
        LEADER_T => ("<Leader>t", &[
            ("f", "ToggleTerm float"),
            ("h", "ToggleTerm horizontal split"),
            ("v", "ToggleTerm vertical split"),
        ]),
        LEADER_U => ("<Leader>u", &[
            ("a", "Toggle autopairs"),
            ("D", "Dismiss notifications"),
            ("g", "Toggle signcolumn"),
            ("l", "Toggle statusline"),
            ("n", "Change line numbering"),
            ("N", "Toggle notifications"),
            ("t", "Toggle tabline"),
            ("u", "Toggle URL highlight"),
            ("y", "Toggle syntax highlight"),
            ("|", "Toggle indent guides"),
        ]),
        'g' => ("g", &[("g", "First line"), ("c", "Toggle comment"), ("cc", "Toggle comment line")]),
        ']' => ("]", &[("b", "Next buffer"), ("g", "Next Git hunk")]),
        '[' => ("[", &[("b", "Previous buffer"), ("g", "Previous Git hunk")]),
        CTRL_W => ("<C-w>", &[
            ("v", "Split window vertically"),
            ("s", "Split window"),
            ("c", "Close window"),
            ("o", "Close all other windows"),
            ("w", "Switch windows"),
            ("h", "Go to the left window"),
            ("j", "Go to the down window"),
            ("k", "Go to the up window"),
            ("l", "Go to the right window"),
        ]),
        _ => ("", &[]),
    }
}

/// Every built-in mapping as (prefix notation, [(key, description)]), for the keymaps picker.
pub fn builtin_maps() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    [LEADER, LEADER_B, LEADER_F, LEADER_G, LEADER_T, LEADER_U, 'g', ']', '[', CTRL_W]
        .iter()
        .map(|&p| {
            let (title, entries) = builtin(p);
            (title, entries.iter().filter(|(_, d)| !d.starts_with('+')).copied().collect())
        })
        .collect()
}

/// The user-map prefix a built-in menu corresponds to.
fn prefix_keys(prefix: char) -> Vec<Key> {
    match prefix {
        LEADER_B => vec![Key::Char(LEADER), Key::Char('b')],
        LEADER_F => vec![Key::Char(LEADER), Key::Char('f')],
        LEADER_G => vec![Key::Char(LEADER), Key::Char('g')],
        LEADER_T => vec![Key::Char(LEADER), Key::Char('t')],
        LEADER_U => vec![Key::Char(LEADER), Key::Char('u')],
        CTRL_W => vec![Key::Ctrl('w')],
        c => vec![Key::Char(c)],
    }
}

impl Editor {
    /// The which-key popup for the keys typed so far, if they open a menu.
    pub fn which_key(&self) -> Option<(String, Vec<MenuEntry>)> {
        let (title, typed, builtins) = match self.pending.prefix {
            Some(p) if !matches!(p, 'f' | 'F' | 't' | 'T' | 'r' | 'q' | '@' | 'i' | 'a' | 'Z') => {
                let (title, entries) = builtin(p);
                (title.to_owned(), prefix_keys(p), entries)
            }
            _ if !self.typeahead.is_empty() => (keys::display(&self.typeahead), self.typeahead.clone(), &[][..]),
            _ => return None,
        };
        let mut entries: Vec<MenuEntry> =
            builtins.iter().map(|(k, d)| MenuEntry { key: (*k).to_owned(), desc: (*d).to_owned(), group: d.starts_with('+') }).collect();
        let mode = self.map_mode_for_menu();
        for map in self.keymaps.iter().filter(|m| Some(m.mode) == mode && m.lhs.len() > typed.len() && m.lhs.starts_with(&typed)) {
            let key = keys::display(&map.lhs[typed.len()..=typed.len()]);
            let group = map.lhs.len() > typed.len() + 1;
            let desc = if group { "+Mappings".to_owned() } else if map.desc.is_empty() { keys::display(map.rhs.as_deref().unwrap_or_default()) } else { map.desc.clone() };
            entries.retain(|e| e.key != key);
            entries.push(MenuEntry { key, desc, group });
        }
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|a, b| b.group.cmp(&a.group).then_with(|| a.key.to_lowercase().cmp(&b.key.to_lowercase())));
        Some((title, entries))
    }

    fn map_mode_for_menu(&self) -> Option<crate::config::MapMode> {
        match self.mode {
            Mode::Normal => Some(crate::config::MapMode::Normal),
            Mode::Visual | Mode::VisualLine => Some(crate::config::MapMode::Visual),
            Mode::Insert => Some(crate::config::MapMode::Insert),
            _ => None,
        }
    }

    /// `<Leader>u…`
    pub(super) fn toggle(&mut self, key: char) {
        let o = &mut self.options;
        let (name, on) = match key {
            'a' => ("autopairs", flip(&mut o.autopairs)),
            'g' => ("signcolumn", flip(&mut o.signcolumn)),
            'l' => ("statusline", flip(&mut o.laststatus)),
            'N' => ("notifications", flip(&mut o.notifications)),
            't' => ("tabline", flip(&mut o.showtabline)),
            'u' => ("URL highlighting", flip(&mut o.highlighturl)),
            'y' => ("syntax highlighting", flip(&mut o.syntax)),
            '|' => ("indent guides", flip(&mut o.indentguides)),
            'n' => {
                // AstroNvim cycles: both → number → relativenumber → none → both.
                (o.number, o.relativenumber) = match (o.number, o.relativenumber) {
                    (true, true) => (true, false),
                    (true, false) => (false, true),
                    (false, true) => (false, false),
                    (false, false) => (true, true),
                };
                let state = format!("number={} relativenumber={}", o.number, o.relativenumber);
                return self.set_message(Message::Info(state));
            }
            'D' => {
                self.message = None;
                return;
            }
            _ => return,
        };
        self.set_message(Message::Info(format!("{name} {}", if on { "enabled" } else { "disabled" })));
    }
}

fn flip(flag: &mut bool) -> bool {
    *flag = !*flag;
    *flag
}
