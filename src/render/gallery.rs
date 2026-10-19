//! The gallery of `docs/themes.md` and the README's screenshots: real frames drawn by the renderer, written as SVG.
//! `cargo test gallery -- --ignored` rewrites `docs/gallery` after a change to a theme, an icon set or the bars;
//! `cargo test screenshots -- --ignored` writes the README's scenes to `target/screenshots`, to rasterise
//! (docs/development.md).

use std::fmt::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::icons::SETS;
use super::palettes::{DEFAULT, THEMES};
use super::row::{Cell, Grid};
use super::theme::{Group, Rgb, Style, theme};
use super::{Screen, apply_colorscheme};
use crate::buffer::Buffer;
use crate::config::{Config, Splash, StatusStyle};
use crate::editor::Editor;
use crate::term::{ColorDepth, Key};

const SAMPLE: &str = r#"use std::collections::HashMap;

/// Counts each word of `text`.
fn word_counts(text: &str) -> HashMap<&str, usize> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1; // TODO: ignore case
    }
    counts
}

fn main() {
    let text: &str = "the quick brown fox jumps over the lazy dog";
    let counts = word_counts(text);
    println!("{} distinct words, color #61afef", counts.len());
}
"#;

const COLS: usize = 100;
const ROWS: usize = 22;
/// One cell in pixels; each glyph is placed on its cell, whatever the font.
const CELL_W: usize = 9;
const CELL_H: usize = 18;

/// The start screen with one monster; tall enough for the art, the wordmark and the footer.
fn splash(splash: Splash) -> String {
    apply_colorscheme(DEFAULT, &Config::default());
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = true;
    ed.options.splash = splash;
    let (grid, _): (Grid, Option<(usize, usize)>) = Screen::new(ColorDepth::TrueColor).compose(&mut ed, 80, 38);
    svg(&grid)
}

/// One frame with the file tree open on the left and the sample file on the right.
fn frame(theme: &str, iconset: &str, style: StatusStyle) -> String {
    apply_colorscheme(theme, &Config::default());
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    ed.buffers[0] = Buffer::from_text(Some("main.rs".into()), SAMPLE);
    ed.options.iconset = iconset.into();
    ed.options.statusline_style = style;
    for key in [Key::Char(' '), Key::Char('e'), Key::Ctrl('l'), Key::Char('9'), Key::Char('G')] {
        ed.handle(key);
    }
    let (grid, _): (Grid, Option<(usize, usize)>) = Screen::new(ColorDepth::TrueColor).compose(&mut ed, COLS, ROWS);
    svg(&grid)
}

fn hex(style: &Style, fg: bool) -> String {
    format!("#{:06x}", if fg { style.fg.0 } else { style.bg.0 })
}

fn escape(c: char, out: &mut String) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        c => out.push(c),
    }
}

/// The grid as an SVG: a rect per run of one background, a text per run of one foreground style.
fn svg(grid: &Grid) -> String {
    let (w, h): (usize, usize) = (grid.width * CELL_W, grid.height * CELL_H);
    let mut out: String = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" \
         font-family=\"'JetBrainsMono Nerd Font','Symbols Nerd Font','Cascadia Code',Consolas,monospace\" \
         font-size=\"14\" xml:space=\"preserve\">\n"
    );
    for y in 0..grid.height {
        let row: &[Cell] = grid.row(y);
        let mut x: usize = 0;
        while x < row.len() {
            let bg: String = hex(&row[x].style, false);
            let len: usize = row[x..].iter().take_while(|c| hex(&c.style, false) == bg).count();
            let _: fmt::Result = writeln!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{CELL_H}\" fill=\"{bg}\"/>",
                x * CELL_W,
                y * CELL_H,
                len * CELL_W
            );
            x += len;
        }
        let mut x: usize = 0;
        while x < row.len() {
            let key = |c: &Cell| (c.style.fg, c.style.bold, c.style.italic, c.style.underline);
            let len: usize = row[x..].iter().take_while(|c| key(c) == key(&row[x])).count();
            let run: &[Cell] = &row[x..x + len];
            // Each glyph gets its own cell's x, so fonts of any width stay on the grid; blanks are left out.
            let mut text: String = String::new();
            let mut xs: Vec<String> = Vec::new();
            for (i, c) in run.iter().enumerate().filter(|(_, c)| !c.wide_tail && c.ch != ' ') {
                for ch in [Some(c.ch), c.mark].into_iter().flatten() {
                    escape(ch, &mut text);
                    xs.push(((x + i) * CELL_W).to_string());
                }
            }
            if !xs.is_empty() {
                let s: &Style = &run[0].style;
                let _: fmt::Result = writeln!(
                    out,
                    "<text x=\"{}\" y=\"{}\" fill=\"{}\"{}{}{}>{text}</text>",
                    xs.join(" "),
                    y * CELL_H + 13,
                    hex(s, true),
                    if s.bold { " font-weight=\"bold\"" } else { "" },
                    if s.italic { " font-style=\"italic\"" } else { "" },
                    if s.underline { " text-decoration=\"underline\"" } else { "" },
                );
            }
            x += len;
        }
    }
    out.push_str("</svg>\n");
    out
}

/// The gallery reads the working directory (the file tree), which the screenshots change: one at a time.
static WORKING_DIR: Mutex<()> = Mutex::new(());

#[test]
#[ignore = "writes docs/gallery; run with `cargo test gallery -- --ignored`"]
fn gallery() {
    let _cwd: MutexGuard<'_, ()> = WORKING_DIR.lock().unwrap_or_else(PoisonError::into_inner);
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs").join("gallery");
    fs::create_dir_all(&dir).unwrap();
    // Unicode icons for the themes, so the pictures need no Nerd Font to read.
    for theme in THEMES {
        fs::write(dir.join(format!("{}.svg", theme.name)), frame(theme.name, "unicode", StatusStyle::Flat)).unwrap();
    }
    for (set, _, _) in SETS {
        fs::write(dir.join(format!("icons-{set}.svg")), frame(DEFAULT, set, StatusStyle::Rounded)).unwrap();
    }
    for style in StatusStyle::ALL {
        let file: String = format!("statusline-{}.svg", style.name());
        fs::write(dir.join(file), frame(DEFAULT, "nerd", style)).unwrap();
    }
    for art in Splash::ALL.into_iter().filter(|s| *s != Splash::Random) {
        fs::write(dir.join(format!("splash-{}.svg", art.name())), splash(art)).unwrap();
    }
}

#[test]
fn svg_escapes_and_sizes() {
    let mut grid: Grid = Grid::new(4, 1, Style::new(Rgb(0xabb2bf), Rgb(0x1e222a)));
    grid.text(0, 0, 4, Style::new(Rgb(0xabb2bf), Rgb(0x1e222a)), "a<&");
    let out: String = svg(&grid);
    assert!(out.contains("width=\"36\" height=\"18\""));
    assert!(out.contains("x=\"0 9 18\" y=\"13\""));
    assert!(out.contains(">a&lt;&amp;</text>"));
}

/// Full screen, the settings cover the bars entirely, borders included.
#[test]
fn settings_hide_the_bars() {
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    for c in " ,".chars() {
        ed.handle(Key::Char(c));
    }
    let (grid, _): (Grid, Option<(usize, usize)>) = Screen::new(ColorDepth::TrueColor).compose(&mut ed, 80, 20);
    let bg: Rgb = grid.row(1)[1].style.bg;
    assert!(grid.row(19).iter().all(|c| c.style.bg == bg), "the bottom border keeps the float's background");
}

/// The which-key menu, frame included, has exactly the editor's background (nvchad, the default theme).
#[test]
fn which_key_has_the_editor_background() {
    apply_colorscheme(DEFAULT, &Config::default());
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    ed.handle(Key::Char(' '));
    let (grid, _): (Grid, Option<(usize, usize)>) = Screen::new(ColorDepth::TrueColor).compose(&mut ed, 120, 14);
    let editor: Rgb = theme().bg(Group::Normal);
    let menu_rows: Vec<usize> = (0..14).filter(|&y| grid.row(y).iter().any(|c| c.ch == '╭' || c.ch == '╰')).collect();
    assert_eq!(menu_rows.len(), 2, "the menu's top and bottom borders are on screen");
    for y in menu_rows[0]..=menu_rows[1] {
        let pill: Rgb = theme().bg(Group::FloatTitle);
        assert!(
            grid.row(y).iter().all(|c| c.style.bg == editor || c.style.bg == pill),
            "row {y} has another background"
        );
    }
}

/// The demo project the README's screenshots are taken in.
const DEMO: &[(&str, &str)] = &[
    ("Cargo.toml", "[package]\nname = \"inventory\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    ("README.md", "# inventory\n\nA tiny stock service.\n"),
    ("notes.txt", "restock on monday\n"),
    ("src/stock.rs", "pub struct Stock;\n"),
    ("src/main.rs", DEMO_MAIN),
];

const DEMO_MAIN: &str = r##"//! A tiny inventory service. Docs: https://example.com/inventory
use std::collections::HashMap;

const ACCENT: &str = "#61afef";

/* Items are kept in memory;
   swap in a database later. */
#[derive(Debug, Clone, PartialEq)]
struct Item {
    name: String,
    quantity: u32,
}

fn restock(stock: &mut HashMap<String, Item>, name: &str, amount: u32) {
    match stock.get_mut(name) {
        Some(item) => {
            item.quantity = item.quantity.saturating_add(amount);
            println!("restocked {} to {}", item.name, item.quantity);
        }
        None => {
            // TODO: log unknown items
            let item = Item { name: name.to_owned(), quantity: amount };
            stock.insert(name.to_owned(), item);
        }
    }
}

fn main() {
    let mut stock: HashMap<String, Item> = HashMap::new();
    restock(&mut stock, "apples", 12);
    restock(&mut stock, "pears", 4);
    println!("{} items, accent {ACCENT}", stock.len());
}
"##;

/// The README's screenshots: each scene played with real keys in the demo project, its frame written as SVG to
/// `target/screenshots`, then rasterised to `docs/screenshots/*.png` (see docs/development.md).
#[test]
#[ignore = "writes target/screenshots; run with `cargo test screenshots -- --ignored`"]
fn screenshots() {
    let _cwd: MutexGuard<'_, ()> = WORKING_DIR.lock().unwrap_or_else(PoisonError::into_inner);
    let out: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("screenshots");
    fs::create_dir_all(&out).unwrap();
    let demo: PathBuf = std::env::temp_dir().join(format!("lim-demo-{}", std::process::id())).join("inventory");
    for (name, text) in DEMO {
        fs::create_dir_all(demo.join(name).parent().unwrap()).unwrap();
        fs::write(demo.join(name), text).unwrap();
    }
    let back: PathBuf = std::env::current_dir().unwrap();
    std::env::set_current_dir(&demo).unwrap();
    for (name, size, keys) in scenes() {
        // The theme picker installs the theme it previews: each scene starts from the default one.
        apply_colorscheme(DEFAULT, &Config::default());
        // As `lim src/main.rs`; the start screen is `lim` alone.
        let files: Vec<String> = if name == "dashboard" { Vec::new() } else { vec!["src/main.rs".into()] };
        let mut ed: Editor = Editor::new(&files);
        ed.options.iconset = "nerd".into();
        for key in keys {
            ed.handle(key);
        }
        let (grid, _): (Grid, Option<(usize, usize)>) =
            Screen::new(ColorDepth::TrueColor).compose(&mut ed, size.0, size.1);
        fs::write(out.join(format!("{name}.svg")), svg(&grid)).unwrap();
    }
    std::env::set_current_dir(back).unwrap();
    fs::remove_dir_all(demo.parent().unwrap()).unwrap();
    apply_colorscheme(DEFAULT, &Config::default());
}

/// Each screenshot: its file name, its size in cells and the keys that set it up.
fn scenes() -> Vec<(&'static str, (usize, usize), Vec<Key>)> {
    let keys = |typed: &str| crate::fuzz::phrase_keys(typed);
    let with = |mut first: Vec<Key>, more: &[Key]| {
        first.extend_from_slice(more);
        first
    };
    vec![
        // The tree opens on `src`, unfolded, then the focus goes back to the code.
        ("editor", (120, 34), with(keys(" el"), &[Key::Ctrl('l'), Key::Char('2'), Key::Char('1'), Key::Char('G')])),
        ("dashboard", (120, 40), Vec::new()),
        ("picker", (120, 34), keys(" ffma")),
        ("grep", (120, 34), keys(" fwrestock")),
        ("whichkey", (120, 34), keys("17G ")),
        ("cheatsheet", (120, 34), vec![Key::F(1)]),
        ("themes", (120, 34), keys(" fttokyo")),
        ("completion", (120, 34), keys("30Goresto")),
        ("cmdline", (120, 34), keys(":s")),
        ("visual", (120, 34), keys("15GV4j")),
        ("tools", (120, 34), keys(" xc2+3*4\nsqrt(2)\n200*15%\n")),
        ("settings", (120, 34), keys(":settings\n")),
    ]
}
