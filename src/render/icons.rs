//! Icon sets: Nerd Font glyphs (`AstroNvim`'s), plain Unicode for any font, color emoji, or ASCII.
//! Picked with the `iconset` option, `:settings` or `<Leader>fI`.

use crate::config::Options;
use crate::syntax::Lang;

pub struct Icons {
    /// Language devicons (Nerd Font only); otherwise `file` for every file.
    pub languages: bool,
    pub file: &'static str,
    pub folder_closed: &'static str,
    pub folder_open: &'static str,
    pub modified: &'static str,
    pub close: &'static str,
    /// The tabline's light/dark button.
    pub theme: &'static str,
    /// nvdash's buttons: find file, recent files, find word, themes, mappings; empty without a Nerd Font.
    pub dashboard: [&'static str; 5],
    pub branch: &'static str,
    pub mode: &'static str,
    pub cwd: &'static str,
    pub position: &'static str,
    /// Rounded statusline section ends; empty for flat blocks.
    pub left_cap: &'static str,
    pub right_cap: &'static str,
    pub path_separator: &'static str,
    pub search: &'static str,
    pub info: &'static str,
    pub error: &'static str,
    pub recording: &'static str,
    pub arrow: &'static str,
    /// Completion item kind (text from buffers).
    pub word: &'static str,
    pub git_modified: &'static str,
    pub git_added: &'static str,
    pub git_untracked: &'static str,
    pub git_deleted: &'static str,
}

pub const NERD: Icons = Icons {
    languages: true,
    file: "\u{f15b}",
    folder_closed: "\u{f07b}",
    folder_open: "\u{f07c}",
    modified: "\u{f444}",
    close: "\u{f00d}",
    theme: "\u{f186}",
    // NvChad's own: search, history, word, palette, keyboard.
    dashboard: ["\u{f002}", "\u{f1da}", "\u{f0219}", "\u{f1958}", "\u{f11c}"],
    branch: "\u{e725}",
    mode: "\u{e7c5}",
    cwd: "\u{f07b}",
    position: "\u{e612}",
    left_cap: "\u{e0b6}",
    right_cap: "\u{e0b4}",
    path_separator: "\u{f105}",
    search: "\u{f002}",
    info: "\u{f05a}",
    error: "\u{f057}",
    recording: "\u{f111}",
    arrow: "\u{279c}",
    word: "\u{f031}",
    // Octicons, as NvChad's statusline: dingbats like ✚ ★ ✖ turn into color emoji in some terminals.
    git_modified: "\u{f459}",
    git_added: "\u{f457}",
    git_untracked: "\u{f420}",
    git_deleted: "\u{f458}",
};

pub const UNICODE: Icons = Icons {
    languages: false,
    file: "\u{25ab}",
    folder_closed: "\u{25b8}",
    folder_open: "\u{25be}",
    modified: "\u{25cf}",
    close: "\u{00d7}",
    theme: "\u{25d0}",
    dashboard: ["", "", "", "", ""],
    branch: "\u{2387}",
    mode: "\u{25c6}",
    cwd: "\u{25a3}",
    position: "\u{2261}",
    left_cap: "",
    right_cap: "",
    path_separator: "\u{203a}",
    search: "\u{2315}",
    info: "\u{2139}",
    error: "\u{2716}",
    recording: "\u{25cf}",
    arrow: "\u{2192}",
    word: "\u{2261}",
    git_modified: "\u{25cf}",
    git_added: "+",
    git_untracked: "?",
    git_deleted: "-",
};

pub const ASCII: Icons = Icons {
    languages: false,
    file: "",
    folder_closed: "+",
    folder_open: "-",
    modified: "[+]",
    close: "x",
    theme: "*",
    dashboard: ["", "", "", "", ""],
    branch: "",
    mode: "",
    cwd: "",
    position: "",
    left_cap: "",
    right_cap: "",
    path_separator: ">",
    search: ">",
    info: "i",
    error: "!",
    recording: "@",
    arrow: "->",
    word: "",
    git_modified: "M",
    git_added: "A",
    git_untracked: "?",
    git_deleted: "D",
};

/// Color emoji: no special font, but each takes two cells.
pub const EMOJI: Icons = Icons {
    languages: false,
    file: "\u{1f4c4}",
    folder_closed: "\u{1f4c1}",
    folder_open: "\u{1f4c2}",
    modified: "\u{25cf}",
    close: "\u{00d7}",
    theme: "\u{1f313}",
    dashboard: ["", "", "", "", ""],
    branch: "\u{1f33f}",
    mode: "",
    cwd: "\u{1f4c1}",
    position: "\u{1f4cd}",
    left_cap: "",
    right_cap: "",
    path_separator: "\u{203a}",
    search: "\u{1f50d}",
    info: "\u{1f4a1}",
    error: "\u{274c}",
    recording: "\u{1f534}",
    arrow: "\u{2192}",
    word: "\u{1f4dd}",
    git_modified: "\u{25cf}",
    git_added: "+",
    git_untracked: "?",
    git_deleted: "-",
};

/// Every set by name, with what it needs, for the option, `<Leader>fI` and the settings screen.
pub const SETS: &[(&str, &str, &Icons)] = &[
    ("nerd", "Nerd Font glyphs", &NERD),
    ("unicode", "any font", &UNICODE),
    ("emoji", "color emoji", &EMOJI),
    ("ascii", "plain ASCII", &ASCII),
];

pub fn icons(options: &Options) -> &'static Icons {
    if !options.icons {
        return &ASCII;
    }
    SETS.iter().find(|(name, _, _)| *name == options.iconset).map_or(&NERD, |(_, _, set)| set)
}

impl Icons {
    /// The icon for a file of `lang`, with its devicon color when the set has language icons.
    pub fn for_file(&self, lang: Option<&Lang>) -> (String, Option<u32>) {
        match lang {
            Some(l) if self.languages => (l.icon.to_string(), Some(l.icon_color)),
            _ => (self.file.to_owned(), None),
        }
    }

    /// `glyph` followed by a space, or nothing when the set has no glyph for it.
    pub fn spaced(glyph: &str) -> String {
        if glyph.is_empty() { String::new() } else { format!("{glyph} ") }
    }
}
