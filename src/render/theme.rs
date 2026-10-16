//! astrodark (AstroNvim/astrotheme) as a palette plus Neovim-named highlight groups.
//! The config can override palette colors (`palette.ui.base`) and any group (`highlights.Comment`).

use std::sync::{Arc, RwLock};

pub use crate::config::Rgb;
use crate::config::{Config, HighlightSpec};
use crate::editor::Mode;
use crate::syntax::Hl;
use super::palettes::THEMES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Style {
    pub const fn new(fg: Rgb, bg: Rgb) -> Self {
        Self { fg, bg, bold: false, italic: false, underline: false }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

macro_rules! groups {
    ($($group:ident = $name:literal,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Group { $($group,)* }
        const GROUP_NAMES: &[(Group, &str)] = &[$((Group::$group, $name),)*];
    };
}

groups! {
    Normal = "Normal",
    NormalNC = "NormalNC",
    CursorLine = "CursorLine",
    LineNr = "LineNr",
    CursorLineNr = "CursorLineNr",
    Visual = "Visual",
    Search = "Search",
    CurSearch = "CurSearch",
    MatchParen = "MatchParen",
    Identifier = "@variable",
    Comment = "@comment",
    String = "@string",
    Number = "@number",
    Keyword = "@keyword",
    Function = "@function",
    Type = "@type",
    TypeBuiltin = "@type.builtin",
    Constant = "@constant",
    Macro = "@function.macro",
    Property = "@property",
    StatusLine = "StatusLine",
    StatusSection = "StatusLineSection",
    StatusModeNormal = "StatusLineNormal",
    StatusModeInsert = "StatusLineInsert",
    StatusModeVisual = "StatusLineVisual",
    StatusModeCommand = "StatusLineCommand",
    StatusModeReplace = "StatusLineReplace",
    StatusCwd = "StatusLineCwd",
    StatusPosition = "StatusLinePosition",
    StatusRecording = "StatusLineRecording",
    TabLine = "TabLine",
    TabLineSel = "TabLineSel",
    TabLineFill = "TabLineFill",
    TabLineIndicator = "TabLineIndicator",
    TabLineModified = "TabLineModified",
    BufferPicker = "BufferPicker",
    WinSeparator = "WinSeparator",
    WinBar = "WinBar",
    WinBarNC = "WinBarNC",
    MsgArea = "MsgArea",
    ErrorMsg = "ErrorMsg",
    NormalFloat = "NormalFloat",
    FloatBorder = "FloatBorder",
    FloatTitle = "FloatTitle",
    PmenuSel = "PmenuSel",
    Directory = "Directory",
    GitSignsAdd = "GitSignsAdd",
    GitSignsChange = "GitSignsChange",
    GitSignsDelete = "GitSignsDelete",
    IndentGuide = "IblIndent",
    WordHighlight = "LspReferenceText",
    Url = "HighlightURL",
    WhichKey = "WhichKey",
    WhichKeyDesc = "WhichKeyDesc",
    WhichKeyPrefix = "WhichKeyGroup",
    WhichKeySeparator = "WhichKeySeparator",
    PickerMatch = "SnacksPickerMatch",
    NotifyInfo = "SnacksNotifierInfo",
    NotifyError = "SnacksNotifierError",
    ExplorerNormal = "NeoTreeNormal",
    ExplorerRoot = "NeoTreeRootName",
    ExplorerDirectory = "NeoTreeDirectoryIcon",
    ExplorerFile = "NeoTreeFileName",
    ExplorerIndent = "NeoTreeIndentMarker",
    ExplorerOpened = "NeoTreeFileNameOpened",
    ExplorerGitModified = "NeoTreeGitModified",
    ExplorerGitAdded = "NeoTreeGitAdded",
    ExplorerGitUntracked = "NeoTreeGitUntracked",
    ExplorerGitDeleted = "NeoTreeGitDeleted",
    PickerDir = "SnacksPickerDir",
    CompletionMatch = "BlinkCmpLabelMatch",
    CompletionKind = "BlinkCmpKind",
    CompletionSource = "BlinkCmpSource",
    CheatsheetKey = "LimCheatsheetKey",
    CheatsheetDesc = "LimCheatsheetDesc",
    TodoFix = "TodoFgFIX",
    TodoTodo = "TodoFgTODO",
    TodoHack = "TodoFgHACK",
    TodoWarn = "TodoFgWARN",
    TodoPerf = "TodoFgPERF",
    TodoNote = "TodoFgNOTE",
    TodoTest = "TodoFgTEST",
}

/// A resolved group: unset colors fall back to `Normal` where it is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Highlight {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct Theme {
    groups: Vec<Highlight>,
}

static THEME: RwLock<Option<Arc<Theme>>> = RwLock::new(None);

/// The theme in use; astrodark until `install` runs. Cheap: a shared pointer.
pub fn theme() -> Arc<Theme> {
    let current = THEME.read().map(|t| t.clone()).unwrap_or_else(|e| e.into_inner().clone());
    current.unwrap_or_else(|| Arc::new(Theme::build(&Config::default(), "astrodark").0))
}

/// Swaps the theme for every later frame (colorscheme switch).
pub fn install(theme: Theme) {
    let mut slot = THEME.write().unwrap_or_else(|e| e.into_inner());
    *slot = Some(Arc::new(theme));
}

fn hl(fg: Option<Rgb>, bg: Option<Rgb>) -> Highlight {
    Highlight { fg, bg, ..Highlight::default() }
}

impl Theme {
    /// The named built-in theme with the config's palette and highlight overrides, plus warnings for unknown names.
    pub fn build(config: &Config, colorscheme: &str) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let base = THEMES.iter().find(|t| t.name == colorscheme).or(THEMES.first());
        let mut palette: Vec<(&str, Rgb)> = base.map(|t| t.colors.iter().map(|&(n, c)| (n, Rgb(c))).collect()).unwrap_or_default();
        for (name, color) in &config.palette {
            match palette.iter_mut().find(|(n, _)| n == name) {
                Some(slot) => slot.1 = *color,
                None => warnings.push(format!("unknown palette color \"{name}\"")),
            }
        }
        let p = |name: &str| palette.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
        let mut groups: Vec<Highlight> = GROUP_NAMES.iter().map(|&(g, _)| default_group(g, &p)).collect();
        for (name, spec) in &config.highlights {
            let Some(index) = GROUP_NAMES.iter().position(|(_, n)| n == name) else {
                warnings.push(format!("unknown highlight group \"{name}\""));
                continue;
            };
            let linked = spec.link.as_ref().and_then(|l| GROUP_NAMES.iter().position(|(_, n)| n == l));
            if spec.link.is_some() && linked.is_none() {
                warnings.push(format!("highlights.{name}: unknown link target"));
            }
            let base = linked.and_then(|i| groups.get(i).copied());
            if let Some(slot) = groups.get_mut(index) {
                *slot = merge(base.unwrap_or(*slot), spec);
            }
        }
        (Self { groups }, warnings)
    }

    pub fn get(&self, group: Group) -> Highlight {
        self.groups.get(group as usize).copied().unwrap_or_default()
    }

    pub fn fg(&self, group: Group) -> Rgb {
        self.get(group).fg.or(self.get(Group::Normal).fg).unwrap_or(Rgb(0xADB0BB))
    }

    pub fn bg(&self, group: Group) -> Rgb {
        self.get(group).bg.or(self.get(Group::Normal).bg).unwrap_or(Rgb(0x1A1D23))
    }

    /// `group` drawn over `bg` when it has no background of its own.
    pub fn on(&self, group: Group, bg: Rgb) -> Style {
        let h = self.get(group);
        Style {
            fg: h.fg.unwrap_or_else(|| self.fg(Group::Normal)),
            bg: h.bg.unwrap_or(bg),
            bold: h.bold,
            italic: h.italic,
            underline: h.underline,
        }
    }

    pub fn style(&self, group: Group) -> Style {
        self.on(group, self.bg(Group::Normal))
    }

    pub fn syntax(&self, hl: Hl, bg: Rgb) -> Style {
        self.on(syntax_group(hl), bg)
    }

    pub fn mode(&self, mode: Mode) -> Group {
        match mode {
            Mode::Normal | Mode::BufferPick(_) | Mode::Picker | Mode::Explorer | Mode::Prompt | Mode::Cheatsheet => {
                Group::StatusModeNormal
            }
            Mode::Insert => Group::StatusModeInsert,
            Mode::Replace => Group::StatusModeReplace,
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => Group::StatusModeVisual,
            Mode::Command | Mode::Search => Group::StatusModeCommand,
        }
    }
}

pub fn syntax_group(hl: Hl) -> Group {
    match hl {
        Hl::Text => Group::Identifier,
        Hl::Keyword => Group::Keyword,
        Hl::Type => Group::Type,
        Hl::Builtin => Group::TypeBuiltin,
        Hl::Str => Group::String,
        Hl::Num => Group::Number,
        Hl::Comment => Group::Comment,
        Hl::Func => Group::Function,
        Hl::Const => Group::Constant,
        Hl::Macro => Group::Macro,
        Hl::Property => Group::Property,
    }
}

fn merge(mut h: Highlight, spec: &HighlightSpec) -> Highlight {
    h.fg = spec.fg.or(h.fg);
    h.bg = spec.bg.or(h.bg);
    h.bold = spec.bold.unwrap_or(h.bold);
    h.italic = spec.italic.unwrap_or(h.italic);
    h.underline = spec.underline.unwrap_or(h.underline);
    h
}

/// astrotheme's group definitions (groups/*.lua, default style options) and AstroNvim's heirline colors.
fn default_group(group: Group, p: &dyn Fn(&str) -> Option<Rgb>) -> Highlight {
    let fg = |name| hl(p(name), None);
    let on = |f, b| hl(p(f), p(b));
    let bold = |h: Highlight| Highlight { bold: true, ..h };
    match group {
        Group::Normal => on("syntax.text", "ui.base"),
        Group::NormalNC => on("syntax.text", "ui.inactive_base"),
        Group::CursorLine => hl(None, p("ui.current_line")),
        Group::LineNr => fg("ui.none_text"),
        Group::CursorLineNr => fg("ui.text_active"),
        Group::Visual | Group::Search => hl(None, p("ui.selection")),
        Group::CurSearch => on("ui.base", "ui.purple"),
        Group::MatchParen => bold(fg("ui.orange")),
        Group::Identifier => fg("syntax.text"),
        Group::Comment => Highlight { italic: true, ..fg("syntax.comment") },
        Group::String => fg("syntax.green"),
        Group::Number => fg("syntax.orange"),
        Group::Keyword => fg("syntax.purple"),
        Group::Function | Group::Type => fg("syntax.blue"),
        Group::TypeBuiltin | Group::Constant | Group::Macro => fg("syntax.yellow"),
        Group::Property => fg("syntax.red"),
        Group::StatusLine => on("ui.text", "ui.statusline"),
        Group::StatusSection => on("ui.text", "ui.prompt"),
        Group::StatusModeNormal => bold(on("ui.base", "ui.blue")),
        Group::StatusModeInsert => bold(on("ui.base", "ui.green")),
        Group::StatusModeVisual => bold(on("ui.base", "ui.purple")),
        Group::StatusModeCommand => bold(on("ui.base", "ui.yellow")),
        Group::StatusModeReplace => bold(on("ui.base", "ui.red")),
        Group::StatusCwd => on("ui.base", "ui.red"),
        Group::StatusPosition => on("ui.base", "ui.green"),
        Group::StatusRecording => bold(on("ui.orange", "ui.statusline")),
        Group::TabLine => on("ui.text_inactive", "ui.tabline"),
        Group::TabLineSel => Highlight { bold: true, italic: true, ..on("ui.text_active", "ui.base") },
        Group::TabLineFill => hl(None, p("ui.tabline")),
        Group::TabLineIndicator => fg("ui.blue"),
        Group::TabLineModified => fg("ui.green"),
        Group::BufferPicker => bold(fg("ui.red")),
        Group::WinSeparator => on("ui.split", "ui.base"),
        Group::WinBar => fg("ui.winbar"),
        Group::WinBarNC => fg("ui.text_inactive"),
        Group::MsgArea => fg("syntax.text"),
        Group::ErrorMsg => fg("ui.red"),
        Group::NormalFloat => on("ui.text", "ui.float"),
        Group::FloatBorder => on("ui.border", "ui.float"),
        Group::FloatTitle => bold(on("ui.title", "ui.float")),
        Group::PmenuSel => bold(hl(None, p("ui.selection"))),
        Group::Directory => fg("ui.blue"),
        Group::GitSignsAdd => fg("ui.green"),
        Group::GitSignsChange => fg("ui.orange"),
        Group::GitSignsDelete => fg("ui.red"),
        Group::IndentGuide => fg("ui.none_text"),
        Group::WordHighlight => hl(None, p("ui.highlight")),
        Group::Url => Highlight { underline: true, ..Highlight::default() },
        Group::WhichKey => fg("ui.cyan"),
        Group::WhichKeyDesc => fg("ui.purple"),
        Group::WhichKeyPrefix => fg("ui.blue"),
        Group::WhichKeySeparator => fg("ui.none_text"),
        Group::PickerMatch => bold(fg("ui.blue")),
        Group::NotifyInfo => fg("ui.green"),
        Group::NotifyError => fg("ui.red"),
        Group::ExplorerNormal => on("ui.text", "ui.tool"),
        Group::ExplorerRoot => bold(fg("ui.text_active")),
        Group::ExplorerDirectory => fg("ui.blue"),
        Group::ExplorerFile => fg("ui.text"),
        Group::ExplorerIndent => fg("ui.none_text"),
        Group::ExplorerOpened | Group::ExplorerGitAdded => fg("ui.green"),
        Group::ExplorerGitModified => fg("ui.orange"),
        Group::ExplorerGitUntracked => fg("ui.yellow"),
        Group::ExplorerGitDeleted => fg("ui.text_inactive"),
        Group::PickerDir => fg("syntax.comment"),
        Group::CompletionMatch => bold(fg("ui.blue")),
        Group::CompletionKind => fg("ui.purple"),
        Group::CompletionSource => fg("syntax.comment"),
        Group::CheatsheetKey => fg("ui.cyan"),
        Group::CheatsheetDesc => fg("ui.text"),
        // todo-comments: error, info, warning, warning, hint-ish and test colors.
        Group::TodoFix => fg("ui.red"),
        Group::TodoTodo => fg("ui.blue"),
        Group::TodoHack | Group::TodoWarn => fg("ui.orange"),
        Group::TodoPerf | Group::TodoTest => fg("ui.purple"),
        Group::TodoNote => fg("ui.green"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_and_links() {
        let (config, _) = Config::from_str(
            r##"{ "palette": { "ui": { "base": "#000001" } },
                  "highlights": { "@comment": { "italic": false }, "Visual": { "link": "CurSearch" }, "Nope": {} } }"##,
        );
        let (theme, warnings) = Theme::build(&config, "astrodark");
        assert_eq!(theme.bg(Group::Normal), Rgb(1));
        assert!(!theme.get(Group::Comment).italic);
        assert_eq!(theme.get(Group::Visual), theme.get(Group::CurSearch));
        assert_eq!(warnings, vec!["unknown highlight group \"Nope\"".to_owned()]);
        assert_eq!(GROUP_NAMES.len(), Group::TodoTest as usize + 1);
    }
}
