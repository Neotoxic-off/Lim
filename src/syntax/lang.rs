//! Per-language facts the editor needs beyond colors (syntect's grammars do those): comment leaders, quotes,
//! the kind of file and its devicon.

use std::path::Path;

use super::Hl;

const BLOCK_COMMENT: (&str, &str, Hl) = ("/*", "*/", Hl::Comment);
const HTML_COMMENT: (&str, &str, Hl) = ("<!--", "-->", Hl::Comment);

/// What kind of text a language is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Code,
    /// Settings: `key: value`, `key = value`, `[section]`.
    Config,
    /// Tags: HTML, XML.
    Markup,
    /// Markdown, which gets the preview.
    Markdown,
}

pub struct Lang {
    /// Nerd Font devicon and its nvim-web-devicons color.
    pub icon: char,
    pub icon_color: u32,
    /// Line comment leader; empty when the language only has block comments.
    pub comment: &'static str,
    /// String delimiters; also the quotes autopairs closes in this language.
    pub quotes: &'static str,
    /// Multi-line regions: (open, close, group); the comment one is what `gc` wraps block-only languages in.
    pub regions: &'static [(&'static str, &'static str, Hl)],
    pub kind: Kind,
}

const BASE: Lang = Lang {
    icon: '\u{f15b}',
    icon_color: 0x6d8086,
    comment: "//",
    quotes: "\"'",
    regions: &[BLOCK_COMMENT],
    kind: Kind::Code,
};

/// By file name first (`Makefile`, `Dockerfile`, dotfiles), then by extension.
pub fn lang_for(path: &Path) -> Option<&'static Lang> {
    Some(match path.file_name()?.to_str()? {
        "Makefile" | "makefile" | "GNUmakefile" => &MAKE,
        "CMakeLists.txt" => &CMAKE,
        "Gemfile" | "Rakefile" | "Podfile" | "Vagrantfile" => &RUBY,
        ".bashrc" | ".bash_profile" | ".bash_aliases" | ".zshrc" | ".zprofile" | ".profile" | "PKGBUILD" => &SHELL,
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitconfig" | ".dockerignore" | ".npmrc"
        | ".editorconfig" => &CONFIG,
        n if n.starts_with("Dockerfile") || n.starts_with("Containerfile") => &DOCKER,
        n if n.starts_with(".env") => &CONFIG,
        _ => match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "rs" => &RUST,
            "lua" => &LUA,
            "py" | "pyi" | "pyw" => &PYTHON,
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "ino" | "m" | "mm" | "cu" => &C,
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "mts" | "cts" => &JS,
            "go" => &GO,
            "sh" | "bash" | "zsh" | "fish" | "ksh" => &SHELL,
            "java" | "groovy" | "gradle" => &JAVA,
            "kt" | "kts" => &KOTLIN,
            "cs" => &CSHARP,
            "swift" => &SWIFT,
            "dart" => &DART,
            "scala" | "sc" => &SCALA,
            "php" => &PHP,
            "rb" | "rake" | "gemspec" => &RUBY,
            "sql" => &SQL,
            "html" | "htm" | "xhtml" | "vue" | "svelte" | "astro" => &HTML,
            "xml" | "svg" | "xaml" | "csproj" | "fsproj" | "vcxproj" | "plist" | "xsd" | "xsl" => &XML,
            "css" => &CSS,
            "scss" | "sass" | "less" => &SCSS,
            "md" | "markdown" | "mdx" => &MARKDOWN,
            "ps1" | "psm1" | "psd1" => &POWERSHELL,
            "bat" | "cmd" => &BATCH,
            "mk" | "mak" => &MAKE,
            "dockerfile" => &DOCKER,
            "cmake" => &CMAKE,
            "toml" | "yml" | "yaml" | "ini" | "conf" | "cfg" | "properties" | "env" => &CONFIG,
            "json" | "jsonc" | "json5" => &JSON,
            "zig" => &ZIG,
            "hs" => &HASKELL,
            "ex" | "exs" => &ELIXIR,
            "r" => &R,
            "pl" | "pm" => &PERL,
            "asm" | "s" => &ASM,
            "nix" => &NIX,
            "tf" | "tfvars" | "hcl" => &TERRAFORM,
            _ => return None,
        },
    })
}

const RUST: Lang = Lang {
    icon: '\u{e7a8}',
    icon_color: 0xdea584,
    quotes: "\"",
    regions: &[BLOCK_COMMENT, ("\"", "\"", Hl::Str)],
    ..BASE
};

const LUA: Lang = Lang {
    icon: '\u{e620}',
    icon_color: 0x51a0cf,
    comment: "--",
    regions: &[("--[[", "]]", Hl::Comment), ("[[", "]]", Hl::Str)],
    ..BASE
};

const PYTHON: Lang = Lang {
    icon: '\u{e606}',
    icon_color: 0xa3b8ef,
    comment: "#",
    regions: &[("\"\"\"", "\"\"\"", Hl::Str), ("'''", "'''", Hl::Str)],
    ..BASE
};

const C: Lang = Lang { icon: '\u{e61e}', icon_color: 0x599eff, ..BASE };

const JS: Lang = Lang {
    icon: '\u{e74e}',
    icon_color: 0xcbcb41,
    quotes: "\"'`",
    regions: &[BLOCK_COMMENT, ("`", "`", Hl::Str)],
    ..BASE
};

const GO: Lang = Lang {
    icon: '\u{e627}',
    icon_color: 0x00add8,
    quotes: "\"'`",
    regions: &[BLOCK_COMMENT, ("`", "`", Hl::Str)],
    ..BASE
};

const SHELL: Lang = Lang { icon: '\u{f489}', icon_color: 0x89e051, comment: "#", regions: &[], ..BASE };

const JAVA: Lang =
    Lang { icon: '\u{e738}', icon_color: 0xcc3e44, regions: &[BLOCK_COMMENT, ("\"\"\"", "\"\"\"", Hl::Str)], ..BASE };

const KOTLIN: Lang =
    Lang { icon: '\u{e634}', icon_color: 0x7f52ff, regions: &[BLOCK_COMMENT, ("\"\"\"", "\"\"\"", Hl::Str)], ..BASE };

const CSHARP: Lang = Lang { icon: '\u{f031b}', icon_color: 0x596706, ..BASE };

const SWIFT: Lang = Lang {
    icon: '\u{e755}',
    icon_color: 0xe37933,
    quotes: "\"",
    regions: &[BLOCK_COMMENT, ("\"\"\"", "\"\"\"", Hl::Str)],
    ..BASE
};

const DART: Lang = Lang {
    icon: '\u{e798}',
    icon_color: 0x03589c,
    regions: &[BLOCK_COMMENT, ("\"\"\"", "\"\"\"", Hl::Str), ("'''", "'''", Hl::Str)],
    ..BASE
};

const SCALA: Lang =
    Lang { icon: '\u{e737}', icon_color: 0xcc3e44, regions: &[BLOCK_COMMENT, ("\"\"\"", "\"\"\"", Hl::Str)], ..BASE };

const PHP: Lang = Lang { icon: '\u{e608}', icon_color: 0xa074c4, ..BASE };

const RUBY: Lang =
    Lang { icon: '\u{e739}', icon_color: 0x701516, comment: "#", regions: &[("=begin", "=end", Hl::Comment)], ..BASE };

const SQL: Lang = Lang { icon: '\u{e706}', icon_color: 0xdad8d8, comment: "--", ..BASE };

const HTML: Lang = Lang {
    icon: '\u{e736}',
    icon_color: 0xe44d26,
    comment: "",
    quotes: "\"",
    regions: &[HTML_COMMENT],
    kind: Kind::Markup,
};

const XML: Lang = Lang { icon: '\u{f05c0}', icon_color: 0xe37933, ..HTML };

const CSS: Lang = Lang { icon: '\u{e749}', icon_color: 0x42a5f5, comment: "", kind: Kind::Config, ..BASE };

const SCSS: Lang = Lang { icon: '\u{e603}', icon_color: 0xf55385, comment: "//", ..CSS };

const MARKDOWN: Lang = Lang {
    icon: '\u{f48a}',
    icon_color: 0xdddddd,
    comment: "",
    quotes: "`",
    regions: &[("```", "```", Hl::Str), HTML_COMMENT],
    kind: Kind::Markdown,
};

const POWERSHELL: Lang =
    Lang { icon: '\u{f0a0a}', icon_color: 0x4273ca, comment: "#", regions: &[("<#", "#>", Hl::Comment)], ..BASE };

const BATCH: Lang = Lang { icon: '\u{e629}', icon_color: 0xc1f12e, comment: "::", quotes: "\"", regions: &[], ..BASE };

const MAKE: Lang = Lang { icon: '\u{e779}', icon_color: 0x6d8086, comment: "#", regions: &[], ..BASE };

const DOCKER: Lang = Lang { icon: '\u{f0868}', icon_color: 0x458ee6, comment: "#", regions: &[], ..BASE };

const CMAKE: Lang = Lang { icon: '\u{e615}', icon_color: 0x6d8086, comment: "#", quotes: "\"", regions: &[], ..BASE };

const CONFIG: Lang =
    Lang { icon: '\u{e615}', icon_color: 0x6d8086, comment: "#", regions: &[], kind: Kind::Config, ..BASE };

const JSON: Lang =
    Lang { icon: '\u{e60b}', icon_color: 0xcbcb41, comment: "//", quotes: "\"", regions: &[BLOCK_COMMENT], ..CONFIG };

const ZIG: Lang = Lang { icon: '\u{e6a9}', icon_color: 0xf69a1b, regions: &[], ..BASE };

const HASKELL: Lang = Lang {
    icon: '\u{e61f}',
    icon_color: 0xa074c4,
    comment: "--",
    quotes: "\"",
    regions: &[("{-", "-}", Hl::Comment)],
    ..BASE
};

const ELIXIR: Lang =
    Lang { icon: '\u{e62d}', icon_color: 0xa074c4, comment: "#", regions: &[("\"\"\"", "\"\"\"", Hl::Str)], ..BASE };

const R: Lang = Lang { icon: '\u{f07d4}', icon_color: 0x2266ba, comment: "#", regions: &[], ..BASE };

const PERL: Lang =
    Lang { icon: '\u{e769}', icon_color: 0x519aba, comment: "#", regions: &[("=pod", "=cut", Hl::Comment)], ..BASE };

const ASM: Lang = Lang { icon: '\u{e637}', icon_color: 0x0091bd, comment: ";", regions: &[], ..BASE };

const NIX: Lang = Lang {
    icon: '\u{f313}',
    icon_color: 0x7ebae4,
    comment: "#",
    quotes: "\"",
    regions: &[BLOCK_COMMENT, ("''", "''", Hl::Str)],
    ..BASE
};

const TERRAFORM: Lang =
    Lang { icon: '\u{f1062}', icon_color: 0x5f43e9, comment: "#", quotes: "\"", kind: Kind::Config, ..BASE };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_map_by_name_then_extension() {
        let cases: [(&str, char); 8] = [
            ("Makefile", MAKE.icon),
            ("Dockerfile.dev", DOCKER.icon),
            (".env.local", CONFIG.icon),
            ("CMakeLists.txt", CMAKE.icon),
            ("Main.JAVA", JAVA.icon),
            ("q.sql", SQL.icon),
            ("index.vue", HTML.icon),
            ("README.md", MARKDOWN.icon),
        ];
        for (file, icon) in cases {
            assert_eq!(lang_for(Path::new(file)).map(|l| l.icon), Some(icon), "{file}");
        }
        assert!(lang_for(Path::new("notes.txt")).is_none());
        assert!(lang_for(Path::new("LICENSE")).is_none());
    }
}
