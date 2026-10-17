//! The settings screen (`:settings`, `<Leader>,`), as VS Code's: every option except keys, searchable, changed live,
//! written to the config file with `w`. `/` searches, `j`/`k` move, `h`/`l` or `<CR>` change the value, `w` saves,
//! `<Esc>` closes and keeps the changes for the session.

use std::path::PathBuf;

use super::{Editor, Message, Mode};
use crate::config::json::Json;
use crate::config::{self, Config, Splash, StatusStyle};
use crate::render::{icons, palettes};
use crate::term::Key;

/// The values an option cycles through.
pub enum Values {
    Toggle,
    Range(usize, usize),
    Words(&'static [&'static str]),
    Themes,
    IconSets,
    StatusStyles,
    Splashes,
    /// `truecolor`: "auto", true or false.
    Auto,
}

impl Values {
    #[expect(clippy::cast_precision_loss, reason = "option ranges are small counts")]
    fn choices(&self) -> Vec<Json> {
        let words = |names: &mut dyn Iterator<Item = &str>| names.map(|n| Json::String(n.to_owned())).collect();
        match self {
            Self::Toggle => vec![Json::Bool(false), Json::Bool(true)],
            Self::Range(min, max) => (*min..=*max).map(|n| Json::Number(n as f64)).collect(),
            Self::Words(names) => words(&mut names.iter().copied()),
            Self::Themes => words(&mut palettes::THEMES.iter().map(|t| t.name)),
            Self::IconSets => words(&mut icons::SETS.iter().map(|(name, _, _)| *name)),
            Self::StatusStyles => words(&mut StatusStyle::ALL.iter().map(|s| s.name())),
            Self::Splashes => words(&mut Splash::ALL.iter().map(|s| s.name())),
            Self::Auto => vec![Json::String("auto".into()), Json::Bool(true), Json::Bool(false)],
        }
    }
}

pub struct Setting {
    pub section: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub values: Values,
}

const fn setting(section: &'static str, name: &'static str, desc: &'static str, values: Values) -> Setting {
    Setting { section, name, desc, values }
}

/// Shown in this order, grouped by section. `mapleader` and `timeoutlen` are left to the keymaps in the file.
pub const SETTINGS: &[Setting] = &[
    setting("Look", "colorscheme", "Theme", Values::Themes),
    setting("Look", "iconset", "Icon set", Values::IconSets),
    setting("Look", "icons", "Show icons at all", Values::Toggle),
    setting("Look", "statusline_style", "Statusline section ends (Nerd Font)", Values::StatusStyles),
    setting("Look", "truecolor", "24-bit colors", Values::Auto),
    setting("Look", "syntax", "Keyword highlighting", Values::Toggle),
    setting("Look", "cursorline", "Highlight the cursor line", Values::Toggle),
    setting("Layout", "number", "Line numbers", Values::Toggle),
    setting("Layout", "relativenumber", "Relative line numbers", Values::Toggle),
    setting("Layout", "signcolumn", "Sign column", Values::Toggle),
    setting("Layout", "foldcolumn", "Blank column left of the numbers", Values::Toggle),
    setting("Layout", "indentguides", "Indent guides", Values::Toggle),
    setting("Layout", "showtabline", "Buffer tabline", Values::Toggle),
    setting("Layout", "laststatus", "Statusline", Values::Toggle),
    setting("Layout", "winbar", "Path bar on each window", Values::Toggle),
    setting("Layout", "cmdheight", "Command line always shown", Values::Range(0, 1)),
    setting("Layout", "notifications", "Messages as notifications", Values::Toggle),
    setting("Layout", "dashboard", "Start screen", Values::Toggle),
    setting("Layout", "splash", "Start screen monster", Values::Splashes),
    setting("Editing", "tabstop", "Columns per tab", Values::Range(1, 16)),
    setting("Editing", "shiftwidth", "Indent width (0: tabstop)", Values::Range(0, 16)),
    setting("Editing", "expandtab", "Indent with spaces", Values::Toggle),
    setting("Editing", "autoindent", "Keep the indent on new lines", Values::Toggle),
    setting("Editing", "autopairs", "Close brackets and quotes", Values::Toggle),
    setting("Editing", "completion", "Completion menu while typing", Values::Toggle),
    setting("Editing", "clipboard", "System clipboard (\"\": internal)", Values::Words(&["unnamedplus", ""])),
    setting("Editing", "scrolloff", "Lines kept above / below the cursor", Values::Range(0, 999)),
    setting("Editing", "sidescrolloff", "Columns kept left / right", Values::Range(0, 999)),
    setting("Search", "ignorecase", "Ignore case", Values::Toggle),
    setting("Search", "smartcase", "…unless the pattern has capitals", Values::Toggle),
    setting("Search", "hlsearch", "Highlight matches", Values::Toggle),
    setting("Helpers", "matchparen", "Matching bracket", Values::Toggle),
    setting("Helpers", "wordhighlight", "Other uses of the word under the cursor", Values::Toggle),
    setting("Helpers", "highlighturl", "Underline URLs", Values::Toggle),
    setting("Helpers", "colorhighlight", "#RRGGBB in its color", Values::Toggle),
    setting("Helpers", "todocomments", "TODO: FIX: HACK: highlighted", Values::Toggle),
    setting("Helpers", "gitsigns", "Git signs (files opened from now on)", Values::Toggle),
];

#[derive(Debug, Default)]
pub struct Setup {
    /// Position in `shown()`.
    pub selected: usize,
    pub query: String,
    /// Keys go to the search field.
    pub typing: bool,
}

impl Setup {
    /// Indexes into `SETTINGS` of those whose name, description or section holds the query, ignoring case.
    pub fn shown(&self) -> Vec<usize> {
        let query: String = self.query.to_lowercase();
        SETTINGS
            .iter()
            .enumerate()
            .filter(|(_, s)| [s.name, s.desc, s.section].iter().any(|t| t.to_lowercase().contains(&query)))
            .map(|(i, _)| i)
            .collect()
    }

    fn current(&self) -> Option<&'static Setting> {
        SETTINGS.get(*self.shown().get(self.selected)?)
    }
}

impl Editor {
    pub(super) fn open_setup(&mut self) {
        self.setup = Some(Setup::default());
        self.mode = Mode::Setup;
    }

    /// The current value of a setting, as the config file writes it.
    fn setup_json(&self, name: &str) -> Json {
        if name == "colorscheme" { Json::String(self.colorscheme.clone()) } else { self.options.get(name) }
    }

    /// The current value of a setting, as the setup screen shows it.
    pub fn setup_value(&self, name: &str) -> String {
        match self.setup_json(name) {
            Json::Bool(b) => (if b { "on" } else { "off" }).into(),
            Json::Number(n) => n.to_string(),
            Json::String(s) if s.is_empty() => "\"\"".into(),
            Json::String(s) => s,
            other => other.kind().into(),
        }
    }

    pub(super) fn setup_key(&mut self, key: Key) {
        let Some(setup): &mut Option<Setup> = &mut self.setup else {
            self.mode = Mode::Normal;
            return;
        };
        if setup.typing {
            match key {
                Key::Char(c) => setup.query.push(c),
                Key::Backspace | Key::Ctrl('h') => {
                    setup.query.pop();
                }
                Key::Esc | Key::Ctrl('c') => {
                    setup.query.clear();
                    setup.typing = false;
                }
                Key::Enter | Key::Down | Key::Tab => setup.typing = false,
                _ => {}
            }
            setup.selected = 0;
            return;
        }
        let last: usize = setup.shown().len().saturating_sub(1);
        match key {
            Key::Char('/') => setup.typing = true,
            Key::Esc if !setup.query.is_empty() => {
                setup.query.clear();
                setup.selected = 0;
            }
            Key::Esc | Key::Ctrl('c') | Key::Char('q') => {
                self.setup = None;
                self.mode = Mode::Normal;
            }
            Key::Down | Key::Char('j') | Key::Ctrl('n') | Key::Tab => setup.selected = (setup.selected + 1).min(last),
            Key::Up | Key::Char('k') | Key::Ctrl('p') | Key::BackTab => {
                setup.selected = setup.selected.saturating_sub(1);
            }
            Key::Char('g') | Key::Home => setup.selected = 0,
            Key::Char('G') | Key::End => setup.selected = last,
            Key::Right | Key::Char('l' | ' ') | Key::Enter => self.setup_step(true),
            Key::Left | Key::Char('h') => self.setup_step(false),
            Key::Char('w') | Key::Ctrl('s') => self.setup_save(),
            _ => {}
        }
    }

    /// The selected setting's next (or previous) value, applied at once.
    fn setup_step(&mut self, forward: bool) {
        let Some(setting): Option<&Setting> = self.setup.as_ref().and_then(Setup::current) else {
            return;
        };
        let choices: Vec<Json> = setting.values.choices();
        let current: Json = self.setup_json(setting.name);
        let at: usize = choices.iter().position(|c| *c == current).unwrap_or(0);
        let next: usize = (if forward { at + 1 } else { at + choices.len() - 1 }) % choices.len();
        let Some(value): Option<Json> = choices.get(next).cloned() else { return };
        // Like the theme picker: remembered for the next start, and pinned only if the config already pins one.
        if let Json::String(theme) = &value
            && setting.name == "colorscheme"
        {
            if self.options.colorscheme.is_some() {
                self.options.colorscheme = Some(theme.clone());
            }
            return self.set_colorscheme(theme, true);
        }
        let mut config: Config = Config { options: self.options.clone(), ..Config::default() };
        let mut warnings: Vec<String> = Vec::new();
        config.read_options(&Json::Object(vec![(setting.name.to_owned(), value.clone())]), &mut warnings);
        self.options = config.options;
        self.apply_options();
    }

    fn setup_save(&mut self) {
        let Some(path): Option<PathBuf> = config::path() else {
            return self.set_message(Message::Error("no config path".into()));
        };
        if !super::may_touch(&path) {
            return;
        }
        match config::save_options(&path, &self.options) {
            Ok(()) => {
                self.config.options.clone_from(&self.options);
                self.info(format!("Wrote {}", path.display()));
            }
            Err(e) => self.set_message(Message::Error(e)),
        }
    }
}
