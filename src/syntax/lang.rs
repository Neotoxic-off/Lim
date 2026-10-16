//! Per-language word lists, comment leader, quotes and devicon.

use std::path::Path;

use super::Hl;

const BLOCK_COMMENT: (&str, &str, Hl) = ("/*", "*/", Hl::Comment);

pub struct Lang {
    /// Nerd Font devicon and its nvim-web-devicons color.
    pub icon: char,
    pub icon_color: u32,
    pub comment: &'static str,
    /// String delimiters; also the quotes autopairs closes in this language.
    pub quotes: &'static str,
    pub macros: bool,
    /// Multi-line regions: (open, close, group), tried before anything else on a line.
    pub regions: &'static [(&'static str, &'static str, Hl)],
    pub keywords: &'static [&'static str],
    pub builtins: &'static [&'static str],
    pub consts: &'static [&'static str],
}

pub fn lang_for(path: &Path) -> Option<&'static Lang> {
    Some(match path.extension()?.to_str()? {
        "rs" => &RUST,
        "lua" => &LUA,
        "py" => &PYTHON,
        "c" | "h" | "cc" | "cpp" | "hpp" => &C,
        "js" | "jsx" | "ts" | "tsx" | "mjs" => &JS,
        "go" => &GO,
        "sh" | "bash" | "zsh" | "fish" => &SHELL,
        "toml" | "yml" | "yaml" | "json" | "ini" | "conf" => &CONFIG,
        _ => return None,
    })
}

const RUST: Lang = Lang {
    icon: '\u{e7a8}',
    icon_color: 0xdea584,
    comment: "//",
    quotes: "\"",
    macros: true,
    regions: &[BLOCK_COMMENT, ("\"", "\"", Hl::Str)],
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for",
        "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "struct",
        "super", "trait", "type", "unsafe", "use", "where", "while",
    ],
    builtins: &[
        "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "Self", "self",
    ],
    consts: &["true", "false", "None", "Some", "Ok", "Err"],
};

const LUA: Lang = Lang {
    icon: '\u{e620}',
    icon_color: 0x51a0cf,
    comment: "--",
    quotes: "\"'",
    macros: false,
    regions: &[("--[[", "]]", Hl::Comment), ("[[", "]]", Hl::Str)],
    keywords: &[
        "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local", "not", "or",
        "repeat", "return", "then", "until", "while",
    ],
    builtins: &["self", "require", "vim"],
    consts: &["true", "false", "nil"],
};

const PYTHON: Lang = Lang {
    icon: '\u{e606}',
    icon_color: 0xa3b8ef,
    comment: "#",
    quotes: "\"'",
    macros: false,
    regions: &[("\"\"\"", "\"\"\"", Hl::Str), ("'''", "'''", Hl::Str)],
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass",
        "raise", "return", "try", "while", "with", "yield",
    ],
    builtins: &["int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes", "self", "cls"],
    consts: &["True", "False", "None"],
};

const C: Lang = Lang {
    icon: '\u{e61e}',
    icon_color: 0x599eff,
    comment: "//",
    quotes: "\"'",
    macros: false,
    regions: &[BLOCK_COMMENT],
    keywords: &[
        "break", "case", "class", "const", "continue", "default", "delete", "do", "else", "enum", "extern", "for",
        "goto", "if", "inline", "namespace", "new", "private", "protected", "public", "return", "sizeof", "static",
        "struct", "switch", "template", "typedef", "union", "using", "virtual", "volatile", "while",
    ],
    builtins: &[
        "void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "bool", "auto", "size_t",
        "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t", "this",
    ],
    consts: &["true", "false", "NULL", "nullptr"],
};

const JS: Lang = Lang {
    icon: '\u{e74e}',
    icon_color: 0xcbcb41,
    comment: "//",
    quotes: "\"'`",
    macros: false,
    regions: &[BLOCK_COMMENT, ("`", "`", Hl::Str)],
    keywords: &[
        "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else",
        "export", "extends", "finally", "for", "from", "function", "if", "import", "in", "instanceof", "interface",
        "let", "new", "of", "return", "switch", "throw", "try", "type", "typeof", "var", "while", "yield",
    ],
    builtins: &["string", "number", "boolean", "any", "void", "unknown", "never", "this"],
    consts: &["true", "false", "null", "undefined"],
};

const GO: Lang = Lang {
    icon: '\u{e627}',
    icon_color: 0x00add8,
    comment: "//",
    quotes: "\"'`",
    macros: false,
    regions: &[BLOCK_COMMENT, ("`", "`", Hl::Str)],
    keywords: &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go",
        "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type",
        "var",
    ],
    builtins: &["bool", "byte", "error", "float64", "int", "int64", "rune", "string", "uint", "any"],
    consts: &["true", "false", "nil", "iota"],
};

const SHELL: Lang = Lang {
    icon: '\u{f489}',
    icon_color: 0x89e051,
    comment: "#",
    quotes: "\"'",
    macros: false,
    regions: &[],
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in", "function",
        "return", "local", "export", "set", "end", "begin", "and", "or", "not",
    ],
    builtins: &["echo", "cd", "source", "exit", "test"],
    consts: &["true", "false"],
};

const CONFIG: Lang = Lang {
    icon: '\u{e615}',
    icon_color: 0x6d8086,
    comment: "#",
    quotes: "\"'",
    macros: false,
    regions: &[],
    keywords: &[],
    builtins: &[],
    consts: &["true", "false", "null"],
};
