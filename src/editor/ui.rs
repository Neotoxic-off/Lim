//! which-key menus and AstroNvim's `<Leader>u` UI toggles.

use super::{Editor, Message, Mode, bindings};
use crate::config::keys::{self, LEADER};
use crate::term::Key;

/// Internal prefix slots for leader sub-menus (`<Leader>b…`, `<Leader>f…`, …).
pub const LEADER_B: char = '\u{F8F0}';
pub const LEADER_F: char = '\u{F8F1}';
pub const LEADER_U: char = '\u{F8F2}';
pub const LEADER_T: char = '\u{F8F3}';
pub const LEADER_G: char = '\u{F8F4}';
pub const CTRL_W: char = '\u{17}';
pub const LEADER_S: char = '\u{F8F5}';
/// `<Leader>bs`: sort buffers.
pub const LEADER_BS: char = '\u{F8F6}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub key: String,
    pub desc: String,
    /// Opens another menu (`+Buffers`).
    pub group: bool,
}

/// A menu's title and its (key, description, opens a group) entries.
type Menu = (&'static str, Vec<(String, &'static str, bool)>);

/// A built-in menu from the binding table.
fn builtin(prefix: char) -> Option<Menu> {
    let title = bindings::menu_title(prefix)?;
    let entries = bindings::BINDINGS
        .iter()
        .filter(|b| b.prefix == prefix)
        .map(|b| (bindings::key_name(b.key), b.desc, matches!(b.action, bindings::Action::Group(_))))
        .collect();
    Some((title, entries))
}

/// Every built-in command as (menu notation, [(key, description)]), for the keymaps picker and cheat sheet.
pub fn builtin_maps() -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    bindings::MENUS
        .iter()
        .filter_map(|&(prefix, _)| builtin(prefix))
        .map(|(title, entries)| (title, entries.into_iter().filter(|(_, _, group)| !group).map(|(k, d, _)| (k, d)).collect()))
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
        LEADER_S => vec![Key::Char(LEADER), Key::Char('S')],
        LEADER_BS => vec![Key::Char(LEADER), Key::Char('b'), Key::Char('s')],
        CTRL_W => vec![Key::Ctrl('w')],
        c => vec![Key::Char(c)],
    }
}

impl Editor {
    /// The which-key popup for the keys typed so far, if they open a menu.
    pub fn which_key(&self) -> Option<(String, Vec<MenuEntry>)> {
        let (title, typed, builtins) = match self.pending.prefix.and_then(|p| builtin(p).map(|b| (p, b))) {
            Some((p, (title, entries))) => (title.to_owned(), prefix_keys(p), entries),
            None if !self.typeahead.is_empty() => (keys::display(&self.typeahead), self.typeahead.clone(), Vec::new()),
            None => return None,
        };
        let mut entries: Vec<MenuEntry> =
            builtins.into_iter().map(|(key, desc, group)| MenuEntry { key, desc: desc.to_owned(), group }).collect();
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
            'r' => ("reference highlighting", flip(&mut o.wordhighlight)),
            'c' | 'C' => ("autocompletion", flip(&mut o.completion)),
            'z' => ("color highlighting", flip(&mut o.colorhighlight)),
            '>' => ("foldcolumn", flip(&mut o.foldcolumn)),
            'p' => {
                // Paste mode: no autopairs, no auto-indent, so pasted text arrives as typed.
                let on = o.autopairs || o.autoindent;
                o.autopairs = !on;
                o.autoindent = !on;
                ("paste mode", on)
            }
            'Z' => {
                self.zen = !self.zen;
                ("zen mode", self.zen)
            }
            'i' => {
                let width = if o.expandtab { o.indent_width() as i64 } else { -(o.tabstop as i64) };
                return self.open_prompt("Set indent value (>0 expandtab, <=0 noexpandtab)", &width.to_string(), super::prompt::PromptAction::IndentWidth);
            }
            'b' => return self.toggle_background(),
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

impl Editor {
    /// `<Leader>ub`: the light or dark variant of the current theme family.
    fn toggle_background(&mut self) {
        const PAIRS: &[(&str, &str)] = &[
            ("astrodark", "astrolight"),
            ("catppuccin-mocha", "catppuccin-latte"),
            ("catppuccin-macchiato", "catppuccin-latte"),
            ("catppuccin-frappe", "catppuccin-latte"),
            ("tokyonight-night", "tokyonight-day"),
            ("tokyonight-storm", "tokyonight-day"),
            ("tokyonight-moon", "tokyonight-day"),
            ("rose-pine", "rose-pine-dawn"),
            ("rose-pine-moon", "rose-pine-dawn"),
        ];
        let current = self.colorscheme.clone();
        let other = PAIRS
            .iter()
            .find_map(|&(dark, light)| if dark == current { Some(light) } else if light == current { Some(dark) } else { None })
            .unwrap_or(if crate::render::palettes::THEMES.iter().any(|t| t.name == current && t.light) { "astrodark" } else { "astrolight" });
        self.set_colorscheme(other, true);
    }
}

fn flip(flag: &mut bool) -> bool {
    *flag = !*flag;
    *flag
}
