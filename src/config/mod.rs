//! User configuration from JSON: Neovim-style options, astrotheme palette, highlight groups and keymaps.
//!
//! Looked up at `$LIM_CONFIG`, else `$XDG_CONFIG_HOME/lim/config.json`, else `~/.config/lim/config.json`.
//! Every problem becomes a warning and the default stays in place: a bad config never stops the editor.

pub mod json;
pub mod keys;

use std::path::PathBuf;
use std::{env, fs, io};

use crate::term::Key;
use json::Json;

/// The documented defaults, shipped as `config.example.json`; a test keeps it in sync with `Config::default`.
pub const EXAMPLE: &str = include_str!("../../config.example.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok().map(Self)).flatten()
    }
}

/// Mirrors the AstroNvim option of the same name; defaults are AstroNvim's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub number: bool,
    pub relativenumber: bool,
    pub signcolumn: bool,
    pub cursorline: bool,
    pub tabstop: usize,
    /// 0 means "use tabstop", as in Vim.
    pub shiftwidth: usize,
    pub expandtab: bool,
    pub scrolloff: usize,
    pub sidescrolloff: usize,
    pub ignorecase: bool,
    pub smartcase: bool,
    pub hlsearch: bool,
    pub autopairs: bool,
    pub autoindent: bool,
    pub mapleader: Key,
    pub timeoutlen: u64,
    pub showtabline: bool,
    pub laststatus: bool,
    pub winbar: bool,
    /// 0: the command line only appears while typing, messages become notifications.
    pub cmdheight: usize,
    pub icons: bool,
    /// None: detect from the environment.
    pub truecolor: Option<bool>,
    pub highlighturl: bool,
    pub indentguides: bool,
    pub matchparen: bool,
    pub gitsigns: bool,
    pub wordhighlight: bool,
    pub notifications: bool,
    pub dashboard: bool,
    /// Keyword highlighting (`<Leader>uy`).
    pub syntax: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            number: true,
            relativenumber: true,
            signcolumn: true,
            cursorline: true,
            tabstop: 2,
            shiftwidth: 0,
            expandtab: true,
            scrolloff: 0,
            sidescrolloff: 0,
            ignorecase: true,
            smartcase: true,
            hlsearch: true,
            autopairs: true,
            autoindent: true,
            mapleader: Key::Char(' '),
            timeoutlen: 500,
            showtabline: true,
            laststatus: true,
            winbar: true,
            cmdheight: 0,
            icons: true,
            truecolor: None,
            highlighturl: true,
            indentguides: true,
            matchparen: true,
            gitsigns: true,
            wordhighlight: true,
            notifications: true,
            dashboard: true,
            syntax: true,
        }
    }
}

impl Options {
    pub fn indent_width(&self) -> usize {
        if self.shiftwidth == 0 { self.tabstop } else { self.shiftwidth }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMode {
    Normal,
    Insert,
    Visual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    pub mode: MapMode,
    pub lhs: Vec<Key>,
    /// None disables the key (`false` in JSON).
    pub rhs: Option<Vec<Key>>,
    pub desc: String,
}

/// One highlight group override; unset fields keep the theme's value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HighlightSpec {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub link: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub options: Options,
    /// astrotheme palette overrides, by `section.name` (e.g. `ui.base`, `syntax.red`).
    pub palette: Vec<(String, Rgb)>,
    pub highlights: Vec<(String, HighlightSpec)>,
    pub keymaps: Vec<Keymap>,
}

pub fn path() -> Option<PathBuf> {
    if let Some(p) = env::var_os("LIM_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("lim").join("config.json"))
}

/// The config and every problem found in it. A missing file is not a problem.
pub fn load() -> (Config, Vec<String>) {
    let Some(path) = path() else { return (Config::default(), Vec::new()) };
    match fs::read_to_string(&path) {
        Ok(text) => {
            let (config, warnings) = Config::from_str(&text);
            let warnings = warnings.into_iter().map(|w| format!("{}: {w}", path.display())).collect();
            (config, warnings)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Config::default(), Vec::new()),
        Err(e) => (Config::default(), vec![format!("{}: {e}", path.display())]),
    }
}

impl Config {
    pub fn from_str(text: &str) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();
        match json::parse(text) {
            Ok(Json::Object(sections)) => {
                for (name, value) in sections {
                    match name.as_str() {
                        "options" => config.read_options(&value, &mut warnings),
                        "palette" => config.read_palette(&value, &mut warnings),
                        "highlights" => config.read_highlights(&value, &mut warnings),
                        "keymaps" => config.read_keymaps(&value, &mut warnings),
                        "$schema" | "_comment" => {}
                        other => warnings.push(format!("unknown section \"{other}\"")),
                    }
                }
            }
            Ok(other) => warnings.push(format!("expected an object at the top, found {}", other.kind())),
            Err(e) => warnings.push(e.to_string()),
        }
        (config, warnings)
    }

    fn read_options(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(fields) = value else { return warnings.push("\"options\" must be an object".into()) };
        let o = &mut self.options;
        for (name, value) in fields {
            let ok = match name.as_str() {
                "number" => set_bool(&mut o.number, value),
                "relativenumber" => set_bool(&mut o.relativenumber, value),
                "signcolumn" => set_bool(&mut o.signcolumn, value),
                "cursorline" => set_bool(&mut o.cursorline, value),
                "tabstop" => set_count(&mut o.tabstop, value, 1, 16),
                "shiftwidth" => set_count(&mut o.shiftwidth, value, 0, 16),
                "expandtab" => set_bool(&mut o.expandtab, value),
                "scrolloff" => set_count(&mut o.scrolloff, value, 0, 999),
                "sidescrolloff" => set_count(&mut o.sidescrolloff, value, 0, 999),
                "ignorecase" => set_bool(&mut o.ignorecase, value),
                "smartcase" => set_bool(&mut o.smartcase, value),
                "hlsearch" => set_bool(&mut o.hlsearch, value),
                "autopairs" => set_bool(&mut o.autopairs, value),
                "autoindent" => set_bool(&mut o.autoindent, value),
                "timeoutlen" => {
                    let mut ms = 0;
                    let ok = set_count(&mut ms, value, 0, 10_000);
                    o.timeoutlen = ms as u64;
                    ok
                }
                "showtabline" => set_bool(&mut o.showtabline, value),
                "laststatus" => set_bool(&mut o.laststatus, value),
                "winbar" => set_bool(&mut o.winbar, value),
                "cmdheight" => set_count(&mut o.cmdheight, value, 0, 1),
                "icons" => set_bool(&mut o.icons, value),
                "highlighturl" => set_bool(&mut o.highlighturl, value),
                "indentguides" => set_bool(&mut o.indentguides, value),
                "matchparen" => set_bool(&mut o.matchparen, value),
                "gitsigns" => set_bool(&mut o.gitsigns, value),
                "wordhighlight" => set_bool(&mut o.wordhighlight, value),
                "notifications" => set_bool(&mut o.notifications, value),
                "dashboard" => set_bool(&mut o.dashboard, value),
                "syntax" => set_bool(&mut o.syntax, value),
                "truecolor" => match value {
                    Json::Bool(b) => {
                        o.truecolor = Some(*b);
                        true
                    }
                    Json::String(s) if s == "auto" => {
                        o.truecolor = None;
                        true
                    }
                    _ => false,
                },
                "mapleader" => match value {
                    Json::String(s) => match keys::parse_one(s) {
                        Ok(key) => {
                            o.mapleader = key;
                            true
                        }
                        Err(e) => {
                            warnings.push(format!("options.mapleader: {e}"));
                            continue;
                        }
                    },
                    _ => false,
                },
                _ => {
                    warnings.push(format!("unknown option \"{name}\""));
                    continue;
                }
            };
            if !ok {
                warnings.push(format!("options.{name}: invalid value ({})", value.kind()));
            }
        }
    }

    fn read_palette(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(sections) = value else { return warnings.push("\"palette\" must be an object".into()) };
        for (section, colors) in sections {
            let Json::Object(colors) = colors else {
                warnings.push(format!("palette.{section} must be an object"));
                continue;
            };
            for (name, color) in colors {
                match color {
                    Json::String(s) => match Rgb::parse(s) {
                        Some(rgb) => self.palette.push((format!("{section}.{name}"), rgb)),
                        None => warnings.push(format!("palette.{section}.{name}: expected \"#RRGGBB\"")),
                    },
                    _ => warnings.push(format!("palette.{section}.{name}: expected \"#RRGGBB\"")),
                }
            }
        }
    }

    fn read_highlights(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(groups) = value else { return warnings.push("\"highlights\" must be an object".into()) };
        for (group, spec) in groups {
            let Json::Object(fields) = spec else {
                warnings.push(format!("highlights.{group} must be an object"));
                continue;
            };
            let mut hl = HighlightSpec::default();
            for (field, value) in fields {
                let ok = match (field.as_str(), value) {
                    ("fg", Json::String(s)) => Rgb::parse(s).map(|c| hl.fg = Some(c)).is_some(),
                    ("bg", Json::String(s)) => Rgb::parse(s).map(|c| hl.bg = Some(c)).is_some(),
                    ("bold", Json::Bool(b)) => {
                        hl.bold = Some(*b);
                        true
                    }
                    ("italic", Json::Bool(b)) => {
                        hl.italic = Some(*b);
                        true
                    }
                    ("underline", Json::Bool(b)) => {
                        hl.underline = Some(*b);
                        true
                    }
                    ("link", Json::String(s)) => {
                        hl.link = Some(s.clone());
                        true
                    }
                    _ => false,
                };
                if !ok {
                    warnings.push(format!("highlights.{group}.{field}: unsupported value"));
                }
            }
            self.highlights.push((group.clone(), hl));
        }
    }

    fn read_keymaps(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(modes) = value else { return warnings.push("\"keymaps\" must be an object".into()) };
        for (mode_name, maps) in modes {
            let modes: &[MapMode] = match mode_name.as_str() {
                "n" => &[MapMode::Normal],
                "i" => &[MapMode::Insert],
                "v" | "x" => &[MapMode::Visual],
                "nv" | "" => &[MapMode::Normal, MapMode::Visual],
                other => {
                    warnings.push(format!("keymaps.{other}: modes are \"n\", \"i\", \"v\" or \"nv\""));
                    continue;
                }
            };
            let Json::Object(maps) = maps else {
                warnings.push(format!("keymaps.{mode_name} must be an object"));
                continue;
            };
            for (lhs, rhs) in maps {
                let parsed = keys::parse(lhs).and_then(|lhs| {
                    let (rhs, desc) = match rhs {
                        Json::Bool(false) => (None, String::new()),
                        Json::String(s) => (Some(keys::parse(s)?), String::new()),
                        Json::Object(fields) => {
                            let field = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
                            let Some(Json::String(keys)) = field("rhs") else { return Err("missing \"rhs\"".into()) };
                            let desc = match field("desc") {
                                Some(Json::String(d)) => d.clone(),
                                _ => String::new(),
                            };
                            (Some(keys::parse(keys)?), desc)
                        }
                        other => return Err(format!("unexpected {}", other.kind())),
                    };
                    Ok((lhs, rhs, desc))
                });
                match parsed {
                    Ok((lhs, rhs, desc)) => {
                        for &mode in modes {
                            self.keymaps.retain(|m| !(m.mode == mode && m.lhs == lhs));
                            self.keymaps.push(Keymap { mode, lhs: lhs.clone(), rhs: rhs.clone(), desc: desc.clone() });
                        }
                    }
                    Err(e) => warnings.push(format!("keymaps.{mode_name}.{lhs}: {e}")),
                }
            }
        }
    }
}

fn set_bool(slot: &mut bool, value: &Json) -> bool {
    match value {
        Json::Bool(b) => {
            *slot = *b;
            true
        }
        _ => false,
    }
}

fn set_count(slot: &mut usize, value: &Json, min: usize, max: usize) -> bool {
    match value {
        Json::Number(n) if n.fract() == 0.0 && *n >= min as f64 && *n <= max as f64 => {
            *slot = *n as usize;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_matches_defaults() {
        let (config, warnings) = Config::from_str(EXAMPLE);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(config.options, Options::default());
    }

    #[test]
    fn reads_overrides_and_reports_problems() {
        let (config, warnings) = Config::from_str(
            r##"{
                "options": { "tabstop": 4, "mapleader": ",", "wrap": true, "number": "yes" },
                "palette": { "ui": { "base": "#000000" }, "syntax": { "red": "red" } },
                "highlights": { "Comment": { "fg": "#112233", "italic": false }, "Visual": { "link": "Search" } },
                "keymaps": { "i": { "jk": "<Esc>" }, "n": { "<Leader>w": { "rhs": ":w<CR>", "desc": "Save" }, "Q": false } }
            }"##,
        );
        assert_eq!(config.options.tabstop, 4);
        assert_eq!(config.options.mapleader, Key::Char(','));
        assert_eq!(config.palette, vec![("ui.base".to_owned(), Rgb(0))]);
        assert_eq!(config.highlights[0].1.fg, Some(Rgb(0x112233)));
        assert_eq!(config.highlights[1].1.link.as_deref(), Some("Search"));
        assert_eq!(config.keymaps.len(), 3);
        assert_eq!(config.keymaps[1].desc, "Save");
        assert_eq!(config.keymaps[2].rhs, None);
        assert_eq!(warnings, vec![
            "unknown option \"wrap\"".to_owned(),
            "options.number: invalid value (a string)".to_owned(),
            "palette.syntax.red: expected \"#RRGGBB\"".to_owned(),
        ]);
    }
}
