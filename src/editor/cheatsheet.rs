//! The cheat sheet (`<Leader>fh`, `<F1>`, `:help`): every key lim knows, grouped in cards.
//! Typing filters it, arrows or `<C-n>`/`<C-p>` scroll, `<Esc>` closes.

use super::{Editor, Mode};
use crate::config::keys;
use crate::term::Key;

/// Vim keys, grouped as the cheat sheet shows them.
const VIM: &[(&str, &[(&str, &str)])] = &[
    (
        "Motions",
        &[
            ("h j k l", "Left, down, up, right"),
            ("w b e ge", "Word start, back, end, previous end"),
            ("<S-Left> <S-Right>", "Word back / forward (also in insert and visual)"),
            ("<S-Up> <S-Down>", "Page up / down (also in insert and visual)"),
            ("W B E gE", "Same, for WORDs (blank-separated)"),
            ("0 ^ $ g_", "Line start, first char, end, last char"),
            ("gg G", "First / last line ({count}G: line)"),
            ("{ }", "Previous / next paragraph"),
            ("%", "Matching bracket"),
            ("H M L", "Top / middle / bottom of the window"),
            ("f F t T", "To / till a char on the line"),
            ("; ,", "Repeat the last f/t, same / reverse"),
            ("<C-d> <C-u>", "Half page down / up"),
            ("<C-f> <C-b>", "Page down / up"),
            ("<C-e> <C-y>", "Scroll the view one line"),
            ("zz zt zb", "Cursor line to middle / top / bottom"),
        ],
    ),
    (
        "Operators",
        &[
            ("d c y", "Delete, change, yank + motion"),
            ("dd cc yy", "Whole lines"),
            ("> < >> <<", "Indent / dedent"),
            ("gu gU g~", "Lower / upper / toggle case"),
            ("gc gcc", "Toggle comment"),
            ("gco gcO", "Comment line below / above"),
            ("iw aw i( a( i\" a\"", "Text objects: word, brackets, quotes"),
        ],
    ),
    (
        "Editing",
        &[
            ("i a I A", "Insert before / after / line start / end"),
            ("o O", "New line below / above"),
            ("x X s S", "Delete char / before, substitute char / line"),
            ("D C Y", "Delete / change / yank to end of line"),
            ("r R", "Replace one char / replace mode"),
            ("J gJ", "Join lines, with / without space"),
            ("~", "Toggle case"),
            ("<C-a> <C-x>", "Increment / decrement number"),
            ("p P", "Put after / before"),
            ("u <C-r>", "Undo / redo"),
            (".", "Repeat last change"),
            ("q{r} @{r} @@", "Record / play macro"),
            ("\"{r}", "Use register r (\"+ clipboard, \"_ none)"),
        ],
    ),
    (
        "Visual",
        &[
            ("v V <C-v>", "Char / line / block selection"),
            ("o gv", "Other end / reselect"),
            ("d y c > < u U ~", "Operators on the selection"),
            ("<Tab> <S-Tab>", "Indent / dedent, keep selection"),
            ("I A", "Insert / append on every block line"),
            ("p", "Replace selection with register"),
            (":", "Ex command on the selected lines"),
        ],
    ),
    (
        "Search & jumps",
        &[
            ("/ ?", "Search forward / backward"),
            ("<Up> <Down> on : / ?", "Older / newer line starting with the typed text"),
            ("n N", "Next / previous match"),
            ("* #", "Word under cursor forward / backward"),
            ("m{a-z}", "Set mark"),
            ("'{a-z} `{a-z}", "Jump to mark line / position"),
            ("'' <C-o>", "Back to the last jump"),
            ("gx", "Open URL under cursor"),
        ],
    ),
    (
        "Insert mode",
        &[
            ("<Esc>", "Back to normal mode"),
            ("<C-w> <C-u>", "Delete word / to line start"),
            ("<C-r>{r}", "Insert register"),
            ("<C-o>", "Run one normal-mode command"),
            ("<C-t> <C-d>", "Indent / dedent line"),
            ("<Tab> <S-Tab>", "Next / previous completion"),
            ("<C-n> <C-p>", "Next / previous completion"),
            ("<CR> <C-e>", "Accept / hide completion"),
            ("<C-Space>", "Show completion"),
        ],
    ),
    (
        "Commands",
        &[
            (":w :q :wq :x", "Write, quit, both, write if changed"),
            (":e {file} :enew", "Edit file / new buffer"),
            (":sp :vs :only", "Split, vertical split, only window"),
            (":bn :bp :bd", "Next / previous / delete buffer"),
            (":s/a/b/g :%s", "Substitute on the line / everywhere"),
            (":{range}d y > <", "Delete, yank, indent lines"),
            (":colorscheme {name}", "Switch theme"),
            (":noh", "Clear search highlight"),
            (":{n}", "Go to line"),
        ],
    ),
];

/// One card of the cheat sheet.
pub struct Card {
    pub title: String,
    pub rows: Vec<(String, String)>,
}

#[derive(Debug, Default)]
pub struct Cheatsheet {
    pub filter: String,
    pub scroll: usize,
}

impl Editor {
    pub(super) fn open_cheatsheet(&mut self) {
        self.cheatsheet = Some(Cheatsheet::default());
        self.mode = Mode::Cheatsheet;
    }

    /// Cards to show, filtered by what was typed (title, keys or description).
    pub fn cheatsheet_cards(&self) -> Vec<Card> {
        let mut cards: Vec<Card> = VIM
            .iter()
            .map(|(title, rows)| Card {
                title: (*title).to_owned(),
                rows: rows.iter().map(|(k, d)| ((*k).to_owned(), (*d).to_owned())).collect(),
            })
            .collect();
        for (prefix, entries) in super::ui::builtin_maps() {
            let title: String = match prefix {
                "<Leader>" => "Leader".to_owned(),
                "<C-w>" => "Windows".to_owned(),
                "g" | "]" | "[" => "Goto & next/previous".to_owned(),
                p => p.trim_start_matches("<Leader>").to_owned(),
            };
            let rows = entries.iter().map(|(k, d)| (format!("{prefix}{k}").replace("<Leader>", "␣"), (*d).to_owned()));
            match cards.iter_mut().find(|c| c.title == title) {
                Some(card) => card.rows.extend(rows),
                None => cards.push(Card { title, rows: rows.collect() }),
            }
        }
        for card in &mut cards {
            card.title = match card.title.as_str() {
                "b" => "Buffers".into(),
                "bs" => "Sort buffers".into(),
                "f" => "Find".into(),
                "g" => "Git".into(),
                "t" => "Terminal".into(),
                "u" => "UI toggles".into(),
                "S" => "Sessions".into(),
                other => other.into(),
            };
        }
        let user: Vec<(String, String)> = self
            .keymaps
            .iter()
            .filter(|m| m.rhs.is_some())
            .map(|m| {
                (
                    keys::display(&m.lhs).replace("<Leader>", "␣"),
                    if m.desc.is_empty() {
                        keys::display(m.rhs.as_deref().unwrap_or_default())
                    } else {
                        m.desc.clone()
                    },
                )
            })
            .collect();
        if !user.is_empty() {
            cards.push(Card { title: "Your keymaps".into(), rows: user });
        }
        let filter: String = self.cheatsheet.as_ref().map(|c| c.filter.to_lowercase()).unwrap_or_default();
        if !filter.is_empty() {
            for card in &mut cards {
                let title_hit: bool = card.title.to_lowercase().contains(&filter);
                card.rows.retain(|(k, d)| {
                    title_hit || k.to_lowercase().contains(&filter) || d.to_lowercase().contains(&filter)
                });
            }
            cards.retain(|c| !c.rows.is_empty());
        }
        cards
    }

    pub(super) fn cheatsheet_key(&mut self, key: Key) {
        let Some(sheet): &mut Option<Cheatsheet> = &mut self.cheatsheet else {
            self.mode = Mode::Normal;
            return;
        };
        match key {
            Key::Esc | Key::Ctrl('c') | Key::F(1) => {
                self.cheatsheet = None;
                self.mode = Mode::Normal;
            }
            Key::Down | Key::Ctrl('n' | 'j') => sheet.scroll += 1,
            Key::Up | Key::Ctrl('p' | 'k') => sheet.scroll = sheet.scroll.saturating_sub(1),
            Key::PageDown | Key::Ctrl('d') => sheet.scroll += 10,
            Key::PageUp | Key::Ctrl('u') => sheet.scroll = sheet.scroll.saturating_sub(10),
            Key::Backspace | Key::Ctrl('h') => {
                sheet.filter.pop();
                sheet.scroll = 0;
            }
            Key::Char(c) => {
                sheet.filter.push(c);
                sheet.scroll = 0;
            }
            _ => {}
        }
    }
}
