//! The gallery of `docs/themes.md`: real frames drawn by the renderer, written as SVG to `docs/gallery`.
//! Regenerate with `cargo test gallery -- --ignored` after changing a theme, an icon set or the bars.

use std::fmt::{self, Write};
use std::fs;
use std::path::{Path, PathBuf};

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

#[test]
#[ignore = "writes docs/gallery; run with `cargo test gallery -- --ignored`"]
fn gallery() {
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
