//! Fuzzing: random input at every entry point, so no input can crash lim. Each fuzzer checks for panics,
//! broken invariants and hangs, and on failure prints the seed and the last inputs to replay it.
//! `LIM_FUZZ=<n>` runs n times more rounds (`LIM_FUZZ=50 cargo test --release fuzz`) for a long session.

use std::env;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::thread;
use std::time::{Duration, Instant};

use crate::buffer::regex::{Match, Regex};
use crate::buffer::{Buffer, text};
use crate::config::{Config, json, keys};
use crate::editor::Editor;
use crate::render::{Screen, markdown};
use crate::term::{ColorDepth, Key, Mouse, MouseKind, Size};

/// A tiny deterministic generator (PCG-style), so a failing seed replays exactly.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap_or(0) % n.max(1)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

/// Rounds per fuzzer: the default keeps `cargo test` quick; `LIM_FUZZ` scales it.
fn rounds(base: usize) -> usize {
    base * env::var("LIM_FUZZ").ok().and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// Any single input taking longer than this is a hang (debug builds, busy machine included).
const HANG: Duration = Duration::from_secs(3);

/// Runs `round(rng)` for each seed; a panic or an `Err` reports the seed and what the round logged.
/// `LIM_FUZZ_SEED=<n>` replays that one seed (the number a failure prints).
fn fuzz(name: &str, seeds: usize, mut round: impl FnMut(&mut Rng, &mut Vec<String>) -> Result<(), String>) {
    let only: Option<usize> = env::var("LIM_FUZZ_SEED").ok().and_then(|n| n.parse().ok());
    let all: Vec<usize> = only.map_or_else(|| (0..rounds(seeds)).collect(), |seed| vec![seed]);
    for seed in all {
        let mut rng: Rng = Rng(u64::try_from(seed).unwrap_or(0) * 7919 + 17);
        let mut log: Vec<String> = Vec::new();
        let outcome: thread::Result<Result<(), String>> =
            panic::catch_unwind(AssertUnwindSafe(|| round(&mut rng, &mut log)));
        let last: Vec<&String> = log.iter().rev().take(40).rev().collect();
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(why)) => panic!("{name}, seed {seed}: {why}\nlast inputs: {last:?}"),
            Err(payload) => {
                let why: &str = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("?");
                panic!("{name}, seed {seed}: panicked: {why}\nlast inputs: {last:?}")
            }
        }
    }
}

// --- The editor: every key in every mode, plus phrases that reach deep features ------------------------------

fn every_key() -> Vec<Key> {
    let printable: &str =
        "hjklwbeWBE0^$%{}gGfFtTnN*#iaIAoOxXsSDCYpPJ~ruvVqQ@:/?<>dcy|\\ 123456789()[]{}\"'`;,-_=+&!.mzZ abcé日😀\t";
    printable
        .chars()
        .map(Key::Char)
        .chain([
            Key::Esc,
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::BackTab,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Home,
            Key::End,
            Key::Delete,
            Key::PageUp,
            Key::PageDown,
            Key::ShiftUp,
            Key::ShiftDown,
            Key::ShiftLeft,
            Key::ShiftRight,
            Key::CtrlUp,
            Key::CtrlDown,
            Key::CtrlLeft,
            Key::CtrlRight,
            Key::Unknown,
            Key::Ctrl(' '),
            Key::Ctrl('^'),
        ])
        .chain((1..=12).map(Key::F))
        .chain(('a'..='z').map(Key::Ctrl))
        .chain(
            [MouseKind::Press, MouseKind::Drag, MouseKind::Release, MouseKind::ScrollUp, MouseKind::ScrollDown]
                .into_iter()
                .flat_map(|kind| [(0, 0), (3, 2), (12, 5), (60, 20), (500, 500)].map(|(col, row)| (kind, col, row)))
                .map(|(kind, col, row)| Key::Mouse(Mouse { kind, col, row })),
        )
        .collect()
}

/// Sequences a random key stream would almost never type, so the fuzz reaches every screen and command.
const PHRASES: &[&str] = &[
    ":settings\n",
    " ,",
    " ff",
    " fw",
    " fl",
    " fb",
    " fC",
    " fk",
    " fr",
    " f'",
    " fn",
    " ft",
    " fI",
    " fo",
    " fs",
    " fT",
    " gg",
    " gc",
    " gt",
    " e",
    " o",
    " um",
    " uZ",
    " ub",
    " un",
    " ?",
    " bb",
    " bse",
    " h",
    "/\\v(a|b)+\n",
    "/\\(\n",
    "?x\\{2,}\n",
    ":%s/\\w\\+/[&]/g\n",
    ":%s/\\v(\\a)(\\d)/\\2\\1/gi\n",
    ":s/a/b/n\n",
    ":%s/a/b/gc\nynyl",
    ":%s/\\w/\\r/c\na",
    ":g/a/norm Ax\n",
    ":g/^$/d\n",
    ":v/x/d\n",
    ":g!/e/s/e/E/\n",
    ":sort\n",
    ":sort! n u\n",
    ":2,$sort i\n",
    ":1m$\n",
    ":t0\n",
    ":.,+2m0\n",
    ":$-1,$d\n",
    ":ls\n",
    ":b 1\n",
    ":b 2\n",
    ":bn\n",
    ":e!\n",
    ":Nope arg\n",
    ":norm ci(x\n",
    ":r Cargo.toml\n",
    ":w\n",
    ":sp\n",
    ":vs\n",
    ":only\n",
    ":close\n",
    ":noh\n",
    ":help\n",
    "gn",
    "cgnX\x1b..",
    "dap",
    "cit<x>\x1b",
    "viWd",
    "\x16jj$Ax\x1b",
    "\x16jjIy\x1b",
    "qaddjq3@a",
    "ma'a`a",
    "\"ayy\"Ap\"2p",
    "gUiwgu$g~~",
    "gccgco\x1b",
    "10\x01\x18",
    "R12\x1b",
    "ggVGJ",
    "gf",
    "\x1e",
    "zzztzb",
    "]r[r]T[T]g[g",
];

/// Keys spelled as a string, control chars standing for their keys (`\x1b` Esc, `\n` Enter, `\x12` `<C-r>`…).
pub fn phrase_keys(phrase: &str) -> Vec<Key> {
    phrase
        .chars()
        .map(|c| match c {
            '\x1b' => Key::Esc,
            '\n' => Key::Enter,
            '\x16' => Key::Ctrl('v'),
            '\x01' => Key::Ctrl('a'),
            '\x18' => Key::Ctrl('x'),
            '\x12' => Key::Ctrl('r'),
            '\x1e' => Key::Ctrl('^'),
            c => Key::Char(c),
        })
        .collect()
}

/// Files of every shape: code in several languages, Markdown, wide and combining chars, tabs, a huge line,
/// an empty file.
fn sample_buffers() -> Vec<Buffer> {
    let long: String = "word ".repeat(2000);
    [
        ("fuzz.rs", "fn main() {\n    let x = \"a(b)\";\n    // TODO: x\n}\n\n/* c\n*/\n"),
        ("notes.md", "# T\n\n- **a** `b` [c](d)\n  1. e\n\n> q\n\n```rs\nx\n```\n| a | b |\n|---|---|\n| 1 | 2 |\n"),
        ("page.html", "<ul>\n  <li class=\"x\">one <b>two</b></li>\n</ul>\n<!-- c -->"),
        ("data.json", "{ \"a\": [1, 2.5e3, true, null], \"b\": { \"c\": \"\\u00e9\" } }"),
        ("wide.txt", "é日本語😀\tx\u{301}y\n\t\tindent\n  mixed \t tabs\n#ff0000 https://x.y/z"),
        ("long.txt", long.as_str()),
        ("empty.py", ""),
        ("script.sh", "#!/bin/sh\necho \"$HOME\" | grep -c x # c\n"),
    ]
    .into_iter()
    .map(|(name, body)| Buffer::from_text(Some(name.into()), body))
    .collect()
}

/// The invariants every frame and command rely on.
fn check(ed: &Editor) -> Result<(), String> {
    if ed.buffers.is_empty() || ed.current >= ed.buffers.len() {
        return Err(format!("buffer index {} of {}", ed.current, ed.buffers.len()));
    }
    if ed.windows.is_empty() || ed.active >= ed.windows.len() {
        return Err(format!("window index {} of {}", ed.active, ed.windows.len()));
    }
    if let Some(w) = ed.windows.iter().find(|w| w.buffer >= ed.buffers.len()) {
        return Err(format!("window shows buffer {} of {}", w.buffer, ed.buffers.len()));
    }
    for (i, buf) in ed.buffers.iter().enumerate() {
        if buf.lines().is_empty() {
            return Err(format!("buffer {i} has no line"));
        }
        if buf.cursor.line > buf.last_line() || buf.cursor.col > buf.line_len(buf.cursor.line) {
            return Err(format!("buffer {i} cursor {:?} outside the text", buf.cursor));
        }
    }
    Ok(())
}

#[test]
fn editor_survives_any_input() {
    let alphabet: Vec<Key> = every_key();
    let phrases: Vec<Vec<Key>> = PHRASES.iter().map(|p| phrase_keys(p)).collect();
    fuzz("editor", 12, |rng, log| {
        let mut ed: Editor = Editor::new(&[]);
        ed.dashboard = rng.chance(10);
        ed.buffers = sample_buffers();
        ed.current = rng.below(ed.buffers.len());
        let mut screen: Screen = Screen::new(if rng.chance(50) { ColorDepth::TrueColor } else { ColorDepth::Ansi256 });
        for step in 0..3000 {
            let typed: Vec<Key> = if rng.chance(8) { rng.pick(&phrases).clone() } else { vec![*rng.pick(&alphabet)] };
            log.push(format!("{step}: {typed:?}"));
            for key in typed {
                let start: Instant = Instant::now();
                ed.handle(key);
                if start.elapsed() > HANG {
                    return Err(format!("step {step}, key {key:?} took {:?}", start.elapsed()));
                }
            }
            let start: Instant = Instant::now();
            ed.quit = false;
            ed.shell_requested = None;
            if rng.chance(20) {
                ed.tick();
                let size: Size = Size { cols: rng.below(170), rows: rng.below(60) };
                log.push(format!("draw {size:?}"));
                screen.draw(&mut ed, size);
            }
            if start.elapsed() > HANG {
                return Err(format!("step {step}, frame took {:?}", start.elapsed()));
            }
            check(&ed)?;
        }
        Ok(())
    });
}

// --- Parsers and text helpers: random strings straight in ----------------------------------------------------

fn random_text(rng: &mut Rng, alphabet: &[&str], len: usize) -> String {
    (0..rng.below(len)).map(|_| *rng.pick(alphabet)).collect()
}

const UNICODE: &[&str] = &[
    "a", "Z", "0", " ", "\t", "\n", "\r", "é", "e\u{301}", "日", "😀", "\u{200b}", "İ", "ß", "\u{0}", "\u{7f}", "\\",
    "\"", "'", "`", "*", "_", "#", "[", "]", "(", ")", "{", "}", "|", "-", ">", "<", "/", ":", ",", "~", "!",
];

#[test]
fn regex_survives_any_pattern() {
    let pieces: &[&str] = &[
        "a", "b", "é", "日", ".", "*", "\\+", "\\=", "\\?", "\\{", "}", "\\{1,3}", "\\{,}", "\\(", "\\)", "\\|", "[",
        "]", "[^", "-", "^", "$", "\\<", "\\>", "\\d", "\\w", "\\s", "\\D", "\\v", "(", ")", "|", "+", "?", "{2}",
        "\\c", "\\C", "\\", "\\t", "\\n", "\\.",
    ];
    fuzz("regex", 600, |rng, log| {
        let pattern: String = random_text(rng, pieces, 14);
        log.push(format!("pattern {pattern:?}"));
        let Ok(re): Result<Regex, String> = Regex::new(&pattern, rng.chance(50)) else { return Ok(()) };
        for _ in 0..8 {
            let line: String = random_text(rng, UNICODE, 60);
            log.push(format!("line {line:?}"));
            let len: usize = line.chars().count();
            let start: Instant = Instant::now();
            let found: Vec<Match> = re.find_all(&line);
            if found != re.without_shortcuts().find_all(&line) {
                return Err("the shortcuts changed the matches".into());
            }
            for m in found {
                if m.range.start > m.range.end || m.range.end > len {
                    return Err(format!("match {:?} outside a {len}-char line", m.range));
                }
                if m.groups.iter().flatten().any(|g| g.start > g.end || g.end > len) {
                    return Err(format!("group outside the line: {:?}", m.groups));
                }
            }
            if start.elapsed() > HANG {
                return Err(format!("took {:?}", start.elapsed()));
            }
        }
        Ok(())
    });
}

#[test]
fn config_survives_any_json() {
    let tokens: &[&str] = &[
        "{",
        "}",
        "[",
        "]",
        ":",
        ",",
        "\"options\"",
        "\"keymaps\"",
        "\"commands\"",
        "\"palette\"",
        "\"highlights\"",
        "\"n\"",
        "\"tabstop\"",
        "\"colorscheme\"",
        "\"run\"",
        "\"on\"",
        "\"save\"",
        "\"<Leader>x\"",
        "\":w<CR>\"",
        "\"#ff0000\"",
        "\"ui\"",
        "\"base\"",
        "\"Normal\"",
        "\"fg\"",
        "\"link\"",
        "true",
        "false",
        "null",
        "0",
        "-1",
        "1e999",
        "3.5",
        "\"\\u00e9\"",
        "\"\\uD800\"",
        "\"\\x\"",
        "//c\n",
        "/*",
        "\"",
        " ",
        "\n",
    ];
    let example: Vec<char> = crate::config::EXAMPLE.chars().collect();
    fuzz("config", 800, |rng, log| {
        let input: String = if rng.chance(30) {
            // The real example file, cut and spliced at random places.
            let mut chars: Vec<char> = example.clone();
            for _ in 0..=rng.below(6) {
                let at: usize = rng.below(chars.len() + 1);
                if rng.chance(50) && at < chars.len() {
                    chars.remove(at);
                } else {
                    chars.insert(at, *rng.pick(&['{', '"', ',', ']', '\\', 'x']));
                }
            }
            chars.into_iter().collect()
        } else {
            random_text(rng, tokens, 40)
        };
        log.push(format!("{input:?}"));
        let _: Result<json::Json, json::ParseError> = json::parse(&input);
        let (config, _): (Config, Vec<String>) = Config::from_str(&input);
        let mut ed: Editor = Editor::new(&[]);
        ed.configure(&config);
        Ok(())
    });
}

#[test]
fn key_notation_survives_any_string() {
    let pieces: &[&str] = &[
        "<", ">", "C-", "S-", "M-", "Leader", "CR", "Esc", "lt", "Bar", "F1", "F99", "Up", "x", "-", "<C-", "<>", "é",
        "日", " ",
    ];
    fuzz("keys", 1000, |rng, log| {
        let notation: String = random_text(rng, pieces, 10);
        log.push(format!("{notation:?}"));
        // Display is for the UI (`<b` reads better than `<lt>b`), so it need not parse back; it must not fail.
        if let Ok(parsed) = keys::parse(&notation) {
            let shown: String = keys::display(&parsed);
            let _: Result<Vec<Key>, String> = keys::parse(&shown);
        }
        Ok(())
    });
}

#[test]
fn text_helpers_survive_any_string() {
    fuzz("text", 1500, |rng, log| {
        let line: String = random_text(rng, UNICODE, 40);
        log.push(format!("{line:?}"));
        let chars: usize = text::char_len(&line);
        for col in 0..=chars + 2 {
            let byte: usize = text::byte_index(&line, col);
            if byte > line.len() || !line.is_char_boundary(byte) {
                return Err(format!("byte_index({col}) = {byte}"));
            }
            let _: usize = text::display_col(&line, col);
        }
        let _: usize = text::str_width(&line);
        for r in text::find_urls(&line).into_iter().chain(text::find_hex_colors(&line).into_iter().map(|(r, _)| r)) {
            if r.end > chars {
                return Err(format!("range {r:?} past {chars} chars"));
            }
        }
        let _: Vec<Range<usize>> = text::find_literal(&line, "a");
        Ok(())
    });
}

/// File names that pick each highlighter.
const NAMES: [&str; 10] = ["x.rs", "x.md", "x.html", "x.py", "x.c", "x.json", "x.sh", "x.css", "x.lua", "x.toml"];

#[test]
fn markdown_and_highlighting_survive_any_document() {
    let pieces: &[&str] = &[
        "# ", "## ", "###### ", "- ", "* ", "1. ", "  ", "- [ ] ", "- [x] ", "> ", "```", "~~~", "| ", " |", "|---|",
        "---", "**", "*", "_", "`", "[", "](", ")", "![", "~~", "\\", "<p>", "word ", "é ", "日本 ", "😀", "\t", "\n",
        "\n\n",
    ];
    fuzz("markdown", 400, |rng, log| {
        let doc: String = random_text(rng, pieces, 80);
        log.push(format!("{doc:?}"));
        let lines: Vec<String> = doc.split('\n').map(str::to_owned).collect();
        for width in [0, 1, 5, 17, 80] {
            let _: Vec<markdown::Line> = markdown::render(&lines, width);
        }
        // The same text as source code in every language the highlighter knows, drawn on screen.
        let name: &str = rng.pick::<&str>(&NAMES);
        let mut ed: Editor = Editor::new(&[]);
        ed.dashboard = false;
        ed.buffers[0] = Buffer::from_text(Some(name.into()), &doc);
        ed.markdown_preview = true;
        Screen::new(ColorDepth::TrueColor).draw(&mut ed, Size { cols: 90, rows: 30 });
        check(&ed)
    });
}

#[test]
fn ex_commands_survive_any_line() {
    let pieces: &[&str] = &[
        "s", "g", "v", "!", "norm ", "sort", " ", "m", "t", "co", "r", "e", "b", "w", "q", "sp", "d", "y", "j", ">",
        "<", "%", ".", "$", "'a", "'<,'>", ",", "+", "-", "1", "9", "0", "/", "#", "\\", "\\(", "\\v", "a", "x", "&",
        "\\1", "i", "n", "u", "Name", "settings", "help", "ls", "\x1b", "é",
    ];
    fuzz("ex", 1500, |rng, log| {
        let mut ed: Editor = Editor::new(&[]);
        ed.dashboard = false;
        ed.buffers = sample_buffers();
        ed.current = rng.below(ed.buffers.len());
        for _ in 0..4 {
            let line: String = random_text(rng, pieces, 12);
            log.push(format!(":{line}"));
            let start: Instant = Instant::now();
            ed.handle(Key::Esc);
            ed.handle(Key::Char(':'));
            for c in line.chars() {
                ed.handle(if c == '\x1b' { Key::Esc } else { Key::Char(c) });
            }
            ed.handle(Key::Enter);
            ed.quit = false;
            if start.elapsed() > HANG {
                return Err(format!("took {:?}", start.elapsed()));
            }
            check(&ed)?;
        }
        Ok(())
    });
}
