//! Icon sets: Nerd Font glyphs (AstroNvim's), plain Unicode for any font, or ASCII.
//! Picked with the `iconset` option, `:iconset` or `<Leader>fI`.

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
    modified: "\u{f111}",
    close: "\u{f00d}",
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
    git_modified: "\u{f111}",
    git_added: "\u{271a}",
    git_untracked: "\u{2605}",
    git_deleted: "\u{2716}",
};

pub const UNICODE: Icons = Icons {
    languages: false,
    file: "\u{25ab}",
    folder_closed: "\u{25b8}",
    folder_open: "\u{25be}",
    modified: "\u{25cf}",
    close: "\u{00d7}",
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

pub fn icons(options: &Options) -> &'static Icons {
    if !options.icons {
        return &ASCII;
    }
    match options.iconset.as_str() {
        "unicode" => &UNICODE,
        "ascii" => &ASCII,
        _ => &NERD,
    }
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
