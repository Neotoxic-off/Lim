//! User configuration from JSON: Neovim-style options, astrotheme palette, highlight groups and keymaps.
//!
//! Looked up at `$LIM_CONFIG`, else `$XDG_CONFIG_HOME/lim/config.json`, else `~/.config/lim/config.json`
//! (`%APPDATA%\lim\config.json` on Windows).
//! Every problem becomes a warning and the default stays in place: a bad config never stops the editor.

pub mod json;
pub mod keys;

use std::path::{Path, PathBuf};
use std::{env, fs, io};

use crate::term::Key;
use json::Json;

/// The documented defaults, shipped as `config.example.json`; a test keeps it in sync with `Config::default`.
pub const EXAMPLE: &str = include_str!("../../config.example.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    fn parse(s: &str) -> Option<Self> {
        let hex: &str = s.strip_prefix('#')?;
        (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok().map(Self)).flatten()
    }
}

/// Mirrors the `AstroNvim` option of the same name; the defaults give `NvChad`'s look.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "mirrors Neovim's boolean options one to one")]
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
    /// `clipboard=unnamedplus`: yank and put go through the system clipboard.
    pub clipboard: bool,
    /// One blank column left of the numbers, as `AstroNvim`'s statuscolumn.
    pub foldcolumn: bool,
    /// highlight-colors: `#RRGGBB` shown in its own color.
    pub colorhighlight: bool,
    /// todo-comments: `TODO:`, `FIX:`, `HACK:`… highlighted in comments.
    pub todocomments: bool,
    /// Built-in theme name (see `:settings`); unset, the last one picked (or nvchad).
    pub colorscheme: Option<String>,
    /// Icon set: "nerd", "unicode" or "ascii".
    pub iconset: String,
    /// blink.cmp-style completion from buffer words while typing.
    pub completion: bool,
    /// Shape of the statusline section ends (Nerd Font icon set only).
    pub statusline_style: StatusStyle,
    /// Which drawing of the hydra the start screen shows.
    pub splash: Splash,
    /// Clicks, drags and the wheel; off leaves the terminal's own selection (to copy) working.
    pub mouse: bool,
}

/// The start screen's hydra: one of its drawings, or a random one that fits, different each start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Splash {
    Random,
    Classic,
    Braille,
    Small,
}

impl Splash {
    pub const ALL: [Self; 4] = [Self::Random, Self::Classic, Self::Braille, Self::Small];

    pub fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Classic => "classic",
            Self::Braille => "braille",
            Self::Small => "small",
        }
    }
}

/// How statusline sections end: `NvChad`'s rounded caps, powerline slants or arrows, or flat blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusStyle {
    Rounded,
    Slanted,
    Arrow,
    Flat,
}

impl StatusStyle {
    pub const ALL: [Self; 4] = [Self::Rounded, Self::Slanted, Self::Arrow, Self::Flat];

    pub fn name(self) -> &'static str {
        match self {
            Self::Rounded => "rounded",
            Self::Slanted => "slanted",
            Self::Arrow => "arrow",
            Self::Flat => "flat",
        }
    }
}

/// Every option, in the order the setup screen writes them.
pub const OPTION_NAMES: &[&str] = &[
    "number",
    "relativenumber",
    "signcolumn",
    "cursorline",
    "tabstop",
    "shiftwidth",
    "expandtab",
    "scrolloff",
    "sidescrolloff",
    "ignorecase",
    "smartcase",
    "hlsearch",
    "autopairs",
    "autoindent",
    "mapleader",
    "timeoutlen",
    "showtabline",
    "laststatus",
    "winbar",
    "cmdheight",
    "icons",
    "truecolor",
    "highlighturl",
    "indentguides",
    "matchparen",
    "gitsigns",
    "wordhighlight",
    "notifications",
    "dashboard",
    "syntax",
    "clipboard",
    "foldcolumn",
    "colorhighlight",
    "todocomments",
    "colorscheme",
    "iconset",
    "completion",
    "statusline_style",
    "splash",
    "mouse",
];

impl Default for Options {
    fn default() -> Self {
        Self {
            number: true,
            relativenumber: false,
            signcolumn: true,
            cursorline: false,
            tabstop: 4,
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
            winbar: false,
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
            clipboard: true,
            foldcolumn: true,
            colorhighlight: true,
            todocomments: true,
            colorscheme: None,
            iconset: "nerd".into(),
            completion: true,
            statusline_style: StatusStyle::Rounded,
            splash: Splash::Random,
            mouse: true,
        }
    }
}

impl Options {
    pub fn indent_width(&self) -> usize {
        if self.shiftwidth == 0 { self.tabstop } else { self.shiftwidth }
    }

    /// An option's value as the config file writes it; `Null` for an unset colorscheme or an unknown name.
    #[expect(clippy::cast_precision_loss, reason = "option values are small counts")]
    pub fn get(&self, name: &str) -> Json {
        let count = |n: usize| Json::Number(n as f64);
        match name {
            "number" => Json::Bool(self.number),
            "relativenumber" => Json::Bool(self.relativenumber),
            "signcolumn" => Json::Bool(self.signcolumn),
            "cursorline" => Json::Bool(self.cursorline),
            "tabstop" => count(self.tabstop),
            "shiftwidth" => count(self.shiftwidth),
            "expandtab" => Json::Bool(self.expandtab),
            "scrolloff" => count(self.scrolloff),
            "sidescrolloff" => count(self.sidescrolloff),
            "ignorecase" => Json::Bool(self.ignorecase),
            "smartcase" => Json::Bool(self.smartcase),
            "hlsearch" => Json::Bool(self.hlsearch),
            "autopairs" => Json::Bool(self.autopairs),
            "autoindent" => Json::Bool(self.autoindent),
            "mapleader" => Json::String(match self.mapleader {
                Key::Char(c) => c.to_string(),
                key => keys::display(&[key]),
            }),
            "timeoutlen" => count(usize::try_from(self.timeoutlen).unwrap_or(usize::MAX)),
            "showtabline" => Json::Bool(self.showtabline),
            "laststatus" => Json::Bool(self.laststatus),
            "winbar" => Json::Bool(self.winbar),
            "cmdheight" => count(self.cmdheight),
            "icons" => Json::Bool(self.icons),
            "truecolor" => self.truecolor.map_or_else(|| Json::String("auto".into()), Json::Bool),
            "highlighturl" => Json::Bool(self.highlighturl),
            "indentguides" => Json::Bool(self.indentguides),
            "matchparen" => Json::Bool(self.matchparen),
            "gitsigns" => Json::Bool(self.gitsigns),
            "wordhighlight" => Json::Bool(self.wordhighlight),
            "notifications" => Json::Bool(self.notifications),
            "dashboard" => Json::Bool(self.dashboard),
            "syntax" => Json::Bool(self.syntax),
            "clipboard" => Json::String(if self.clipboard { "unnamedplus" } else { "" }.into()),
            "foldcolumn" => Json::Bool(self.foldcolumn),
            "colorhighlight" => Json::Bool(self.colorhighlight),
            "todocomments" => Json::Bool(self.todocomments),
            "colorscheme" => self.colorscheme.clone().map_or(Json::Null, Json::String),
            "iconset" => Json::String(self.iconset.clone()),
            "completion" => Json::Bool(self.completion),
            "statusline_style" => Json::String(self.statusline_style.name().into()),
            "splash" => Json::String(self.splash.name().into()),
            "mouse" => Json::Bool(self.mouse),
            _ => Json::Null,
        }
    }
}

/// Writes `options` into the config file at `path`, keeping its other sections. The previous file is copied to
/// `config.json.bak` first, since its comments do not survive. A file that does not parse is left alone.
pub fn save_options(path: &Path, options: &Options) -> Result<(), String> {
    let old: Option<String> = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut sections: Vec<(String, Json)> = match old.as_deref().map(json::parse) {
        None => Vec::new(),
        Some(Ok(Json::Object(sections))) => sections,
        Some(Ok(_)) => return Err(format!("{}: not a JSON object, not overwritten", path.display())),
        Some(Err(e)) => return Err(format!("{}: {e}, not overwritten", path.display())),
    };
    let fields: Vec<(String, Json)> = OPTION_NAMES
        .iter()
        .map(|&name| (name.to_owned(), options.get(name)))
        .filter(|(_, value)| *value != Json::Null)
        .collect();
    match sections.iter_mut().find(|(name, _)| name == "options") {
        Some((_, value)) => *value = Json::Object(fields),
        None => sections.insert(0, ("options".to_owned(), Json::Object(fields))),
    }
    let write = |p: &Path, text: &str| fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()));
    if let Some(old) = old {
        write(&path.with_extension("json.bak"), &old)?;
    } else if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    write(path, &(json::pretty(&Json::Object(sections)) + "\n"))
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

/// A keymap's `lhs`, `rhs` and `desc` as read from JSON, before it is expanded per mode.
type ParsedMapping = (Vec<Key>, Option<Vec<Key>>, String);

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
    pub commands: Vec<UserCommand>,
}

/// What a `:Name` command does with its program's stdout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutput {
    /// Replaces the lines it was given (a formatter, a sorter).
    Replace,
    Message,
}

/// `:Name` from the config: the lines go to `run`'s stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserCommand {
    pub name: String,
    /// Program and arguments (`windows` instead of `run` on Windows when given); `${file}` and `${line}` are expanded
    /// when it runs.
    pub run: Vec<String>,
    pub output: CommandOutput,
    pub desc: String,
    pub on: Trigger,
    /// File extensions (`"js"`, `"*.ts"`) an automatic run is limited to; empty for every file.
    pub files: Vec<String>,
}

/// When a command runs besides `:Name`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trigger {
    #[default]
    Manual,
    /// `"on": "save"`: before the buffer is written (format on save).
    Save,
}

impl UserCommand {
    /// Whether it runs automatically when a file with this extension is written.
    pub fn runs_on_save(&self, path: &Path) -> bool {
        let ext: String = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        self.on == Trigger::Save
            && (self.files.is_empty()
                || self.files.iter().any(|f| f.trim_start_matches("*.").eq_ignore_ascii_case(&ext)))
    }
}

pub fn path() -> Option<PathBuf> {
    if let Some(p) = env::var_os("LIM_CONFIG") {
        return Some(PathBuf::from(p));
    }
    Some(user_dir("XDG_CONFIG_HOME", "APPDATA", &[".config"])?.join("config.json"))
}

/// Where sessions, recent files and the last colorscheme live: `$XDG_STATE_HOME/lim`, else `~/.local/state/lim`
/// (`%LOCALAPPDATA%\lim` on Windows).
pub fn state_dir() -> Option<PathBuf> {
    user_dir("XDG_STATE_HOME", "LOCALAPPDATA", &[".local", "state"])
}

/// `$xdg/lim` when set to an absolute path, else the Windows folder `%windows%\lim`, else `~/<home…>/lim`.
fn user_dir(xdg: &str, windows: &str, home: &[&str]) -> Option<PathBuf> {
    let absolute = |var: &str| env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute());
    let base: PathBuf = absolute(xdg)
        .or_else(|| if cfg!(windows) { absolute(windows) } else { None })
        .or_else(|| env::var_os("HOME").map(|h| home.iter().fold(PathBuf::from(h), |p, part| p.join(part))))?;
    Some(base.join("lim"))
}

/// The config and every problem found. A missing file is not a problem.
pub fn load() -> (Config, Vec<String>) {
    let mut config: Config = Config::default();
    let mut warnings: Vec<String> = Vec::new();
    let Some(path): Option<PathBuf> = path() else { return (config, warnings) };
    match fs::read_to_string(&path) {
        Ok(text) => {
            config.read(&text, &mut warnings);
            for w in &mut warnings {
                *w = format!("{}: {w}", path.display());
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => warnings.push(format!("{}: {e}", path.display())),
    }
    (config, warnings)
}

impl Config {
    #[cfg(test)]
    pub fn from_str(text: &str) -> (Self, Vec<String>) {
        let mut config: Config = Self::default();
        let mut warnings: Vec<String> = Vec::new();
        config.read(text, &mut warnings);
        (config, warnings)
    }

    /// Merges one config.json over what was read before.
    fn read(&mut self, text: &str, warnings: &mut Vec<String>) {
        match json::parse(text) {
            Ok(Json::Object(sections)) => {
                for (name, value) in sections {
                    match name.as_str() {
                        "options" => self.read_options(&value, warnings),
                        "palette" => self.read_palette(&value, warnings),
                        "highlights" => self.read_highlights(&value, warnings),
                        "keymaps" => self.read_keymaps(&value, warnings),
                        "commands" => self.read_commands(&value, warnings),
                        "$schema" | "_comment" => {}
                        other => warnings.push(format!("unknown section \"{other}\"")),
                    }
                }
            }
            Ok(other) => warnings.push(format!("expected an object at the top, found {}", other.kind())),
            Err(e) => warnings.push(e.to_string()),
        }
    }

    fn read_commands(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(commands): &Json = value else {
            return warnings.push("\"commands\" must be an object".into());
        };
        for (name, spec) in commands {
            match user_command(name, spec) {
                Ok(command) => {
                    self.commands.retain(|c| c.name != command.name);
                    self.commands.push(command);
                }
                Err(e) => warnings.push(format!("commands.{name}: {e}")),
            }
        }
    }

    #[expect(clippy::too_many_lines, reason = "one arm per option")]
    pub fn read_options(&mut self, value: &Json, warnings: &mut Vec<String>) {
        let Json::Object(fields): &Json = value else { return warnings.push("\"options\" must be an object".into()) };
        let o: &mut Options = &mut self.options;
        for (name, value) in fields {
            let ok: bool = match name.as_str() {
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
                    let mut ms: usize = 0;
                    let ok: bool = set_count(&mut ms, value, 0, 10_000);
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
                "foldcolumn" => set_bool(&mut o.foldcolumn, value),
                "completion" => set_bool(&mut o.completion, value),
                "colorhighlight" => set_bool(&mut o.colorhighlight, value),
                "todocomments" => set_bool(&mut o.todocomments, value),
                "clipboard" => match value {
                    Json::String(s) => {
                        o.clipboard = s.contains("unnamed");
                        true
                    }
                    other => set_bool(&mut o.clipboard, other),
                },
                "colorscheme" => match value {
                    Json::String(s) => {
                        o.colorscheme = Some(s.clone());
                        true
                    }
                    _ => false,
                },
                "iconset" => match value {
                    Json::String(s) if crate::render::icons::SETS.iter().any(|(name, _, _)| name == s) => {
                        o.iconset.clone_from(s);
                        true
                    }
                    _ => false,
                },
                "statusline_style" => match value {
                    Json::String(s) => StatusStyle::ALL
                        .into_iter()
                        .find(|style| style.name() == s)
                        .map(|style| o.statusline_style = style)
                        .is_some(),
                    _ => false,
                },
                "mouse" => set_bool(&mut o.mouse, value),
                "splash" => match value {
                    Json::String(s) => {
                        Splash::ALL.into_iter().find(|art| art.name() == s).map(|art| o.splash = art).is_some()
                    }
                    _ => false,
                },
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
        let Json::Object(sections): &Json = value else { return warnings.push("\"palette\" must be an object".into()) };
        for (section, colors) in sections {
            let Json::Object(colors): &Json = colors else {
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
        let Json::Object(groups): &Json = value else {
            return warnings.push("\"highlights\" must be an object".into());
        };
        for (group, spec) in groups {
            let Json::Object(fields): &Json = spec else {
                warnings.push(format!("highlights.{group} must be an object"));
                continue;
            };
            let mut hl: HighlightSpec = HighlightSpec::default();
            for (field, value) in fields {
                let ok: bool = match (field.as_str(), value) {
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
        let Json::Object(modes): &Json = value else { return warnings.push("\"keymaps\" must be an object".into()) };
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
            let Json::Object(maps): &Json = maps else {
                warnings.push(format!("keymaps.{mode_name} must be an object"));
                continue;
            };
            for (lhs, rhs) in maps {
                let parsed: Result<ParsedMapping, String> = keys::parse(lhs).and_then(|lhs| {
                    let (rhs, desc): (Option<Vec<Key>>, String) = match rhs {
                        Json::Bool(false) => (None, String::new()),
                        Json::String(s) => (Some(keys::parse(s)?), String::new()),
                        Json::Object(fields) => {
                            let field = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
                            let Some(Json::String(keys)): Option<&Json> = field("rhs") else {
                                return Err("missing \"rhs\"".into());
                            };
                            let desc: String = match field("desc") {
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

fn user_command(name: &str, spec: &Json) -> Result<UserCommand, String> {
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("a name starts with an uppercase letter, then letters or digits, as in Vim".into());
    }
    let Json::Object(fields): &Json = spec else { return Err(format!("expected an object, found {}", spec.kind())) };
    let field = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
    let words =
        |key: &str| -> Result<Option<Vec<String>>, String> {
            match field(key) {
                None => Ok(None),
                Some(Json::Array(parts)) if !parts.is_empty() => parts
                    .iter()
                    .map(|p| {
                        if let Json::String(s) = p {
                            Ok(s.clone())
                        } else {
                            Err(format!("\"{key}\" holds {}", p.kind()))
                        }
                    })
                    .collect::<Result<Vec<String>, String>>()
                    .map(Some),
                Some(_) => Err(format!("\"{key}\" must be a non-empty array of strings")),
            }
        };
    // `windows` replaces `run` there, for tools that differ (coreutils vs PowerShell).
    let (run, windows): (Option<Vec<String>>, Option<Vec<String>>) = (words("run")?, words("windows")?);
    let Some(run): Option<Vec<String>> = (if cfg!(windows) { windows.or(run) } else { run }) else {
        return Err("\"run\" must be a non-empty array of strings".into());
    };
    let output: CommandOutput = match field("output") {
        None => CommandOutput::Message,
        Some(Json::String(s)) if s == "message" => CommandOutput::Message,
        Some(Json::String(s)) if s == "replace" => CommandOutput::Replace,
        Some(_) => return Err("\"output\" is \"replace\" or \"message\"".into()),
    };
    let desc: String = match field("desc") {
        Some(Json::String(d)) => d.clone(),
        _ => String::new(),
    };
    let on: Trigger = match field("on") {
        None => Trigger::Manual,
        Some(Json::String(s)) if s == "save" => Trigger::Save,
        Some(_) => return Err("\"on\" can only be \"save\"".into()),
    };
    let files: Vec<String> = words("files")?.unwrap_or_default();
    Ok(UserCommand { name: name.to_owned(), run, output, desc, on, files })
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

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "bounds are small config limits and `n` is a whole number checked against them first"
)]
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
        let (config, warnings): (Config, Vec<String>) = Config::from_str(EXAMPLE);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(config.options, Options::default());
    }

    /// Every option written by `save_options` reads back to the same value, and other sections survive.
    #[test]
    fn saved_options_read_back() {
        let options: Options = Options {
            tabstop: 8,
            truecolor: Some(false),
            clipboard: false,
            colorscheme: Some("nord".into()),
            statusline_style: StatusStyle::Arrow,
            mapleader: Key::Char(','),
            ..Options::default()
        };
        let path: PathBuf = env::temp_dir().join(format!("lim-save-{}.json", std::process::id()));
        fs::write(&path, "// mine\n{ \"keymaps\": { \"i\": { \"jk\": \"<Esc>\" } }, \"options\": { \"x\": 1 } }")
            .unwrap();
        save_options(&path, &options).unwrap();
        let text: String = fs::read_to_string(&path).unwrap();
        let backup: String = fs::read_to_string(path.with_extension("json.bak")).unwrap();
        let _: io::Result<()> = fs::remove_file(&path);
        let _: io::Result<()> = fs::remove_file(path.with_extension("json.bak"));
        assert!(backup.starts_with("// mine"));
        let (config, warnings): (Config, Vec<String>) = Config::from_str(&text);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(config.options, options);
        assert_eq!(config.keymaps.len(), 1);
        assert!(OPTION_NAMES.iter().all(|name| *name == "colorscheme" || Options::default().get(name) != Json::Null));
    }

    /// A command defined twice keeps the last one, and bad commands are reported.
    #[test]
    fn reads_commands() {
        let (config, warnings): (Config, Vec<String>) = Config::from_str(
            r#"{ "commands": { "Sort": { "run": ["sort"] }, "Sort": { "run": ["sort", "-r"], "output": "replace" }, "bad": { "run": [] } } }"#,
        );
        assert_eq!(config.commands.len(), 1);
        assert_eq!(config.commands[0].run, ["sort", "-r"]);
        assert_eq!(config.commands[0].output, CommandOutput::Replace);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("commands.bad"), "{warnings:?}");
    }

    #[test]
    fn reads_overrides_and_reports_problems() {
        let (config, warnings): (Config, Vec<String>) = Config::from_str(
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
        assert_eq!(
            warnings,
            vec![
                "unknown option \"wrap\"".to_owned(),
                "options.number: invalid value (a string)".to_owned(),
                "palette.syntax.red: expected \"#RRGGBB\"".to_owned(),
            ]
        );
    }
}
