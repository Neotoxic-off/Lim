//! Screen cells: rows built left to right, a grid that floats can be drawn over,
//! and the encoding of a row into ANSI with an SGR sequence only when the style changes.

use std::fmt::{self, Write};
use std::ops::Range;

use super::theme::{Group, Rgb, Style, theme};
use crate::buffer::text;
use crate::editor::Rect;
use crate::term::ColorDepth;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    /// One combining mark drawn on top of `ch`.
    pub mark: Option<char>,
    pub style: Style,
    /// Right half of a wide char: drawn by the cell before it.
    pub wide_tail: bool,
}

impl Cell {
    pub fn blank(style: Style) -> Self {
        Self { ch: ' ', mark: None, style, wide_tail: false }
    }
}

/// A row of exactly `width` cells, filled left to right.
pub struct Row {
    cells: Vec<Cell>,
    width: usize,
}

impl Row {
    pub fn new(width: usize) -> Self {
        Self { cells: Vec::with_capacity(width), width }
    }

    pub fn used(&self) -> usize {
        self.cells.len()
    }

    pub fn is_full(&self) -> bool {
        self.cells.len() >= self.width
    }

    /// Draws `c` over as many cells as it takes; a wide char that does not fit becomes padding.
    pub fn cell(&mut self, style: Style, c: char) {
        // File contents must never reach the terminal as control sequences, nor as bidi controls that make a terminal
        // reorder the text (shown text differing from the file: "Trojan Source").
        let bidi: bool =
            matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
        let c: char = if c.is_control() || bidi { '?' } else { c };
        match text::char_width(c) {
            0 => {
                if let Some(last) = self.cells.iter_mut().rev().find(|c| !c.wide_tail) {
                    last.mark = Some(c);
                }
            }
            w if self.cells.len() + w > self.width => self.pad_to(style, self.width),
            w => {
                self.cells.push(Cell { ch: c, mark: None, style, wide_tail: false });
                if w == 2 {
                    self.cells.push(Cell { ch: ' ', mark: None, style, wide_tail: true });
                }
            }
        }
    }

    pub fn text(&mut self, style: Style, s: &str) {
        for c in s.chars() {
            self.cell(style, c);
        }
    }

    /// Fills with blanks up to column `col` (capped at the row width).
    pub fn pad_to(&mut self, style: Style, col: usize) {
        let target: usize = col.min(self.width);
        if self.cells.len() < target {
            self.cells.resize(target, Cell::blank(style));
        }
    }

    pub fn finish(mut self, style: Style) -> Vec<Cell> {
        self.pad_to(style, self.width);
        self.cells
    }
}

/// The whole screen as cells.
pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize, style: Style) -> Self {
        Self { width, height, cells: vec![Cell::blank(style); width * height] }
    }

    pub fn row(&self, y: usize) -> &[Cell] {
        self.cells.get(y * self.width..(y + 1) * self.width).unwrap_or(&[])
    }

    /// Copies `cells` to (x, y), clipped to the screen. A wide char cut by the edge becomes a blank.
    pub fn put(&mut self, x: usize, y: usize, cells: &[Cell]) {
        if y >= self.height || x >= self.width {
            return;
        }
        let n: usize = cells.len().min(self.width - x);
        if n == 0 {
            return;
        }
        let start: usize = y * self.width + x;
        let end: usize = start + n;
        // Covering half of a wide char already on screen orphans the other half.
        let orphan_left: bool = x > 0 && self.cells.get(start).is_some_and(|c| c.wide_tail);
        let orphan_right: bool = x + n < self.width && self.cells.get(end).is_some_and(|c| c.wide_tail);
        if let Some(dst) = self.cells.get_mut(start..end) {
            dst.copy_from_slice(&cells[..n]);
            if let Some(first) = dst.first_mut()
                && first.wide_tail
            {
                *first = Cell::blank(first.style);
            }
            if let Some(last) = dst.last_mut()
                && cells.get(n).is_some_and(|c| c.wide_tail)
            {
                *last = Cell::blank(last.style);
            }
        }
        for (orphaned, i) in [(orphan_left, start.wrapping_sub(1)), (orphan_right, end)] {
            if orphaned && let Some(cell) = self.cells.get_mut(i) {
                *cell = Cell::blank(cell.style);
            }
        }
    }

    pub fn fill(&mut self, area: Rect, style: Style) {
        for y in area.y..area.y + area.h {
            self.put(area.x, y, &vec![Cell::blank(style); area.w]);
        }
    }

    pub fn text(&mut self, x: usize, y: usize, width: usize, style: Style, s: &str) {
        let mut row: Row = Row::new(width);
        row.text(style, s);
        self.put(x, y, &row.finish(style));
    }

    /// A `│` separator in column `x` over `rows`.
    pub fn vline(&mut self, x: usize, rows: Range<usize>, style: Style) {
        for y in rows {
            self.text(x, y, 1, style, "\u{2502}");
        }
    }

    /// A rounded float (`winborder = "rounded"`) with an optional `FloatTitle` pill. The border is part of the
    /// panel: it takes `fill`'s background, never `border`'s or the colors underneath, so the float reads as one
    /// block. A border whose fg equals its bg (the borderless look of `NvChad`, the default) is drawn in the fill's color.
    pub fn float(&mut self, area: Rect, border: Style, fill: Style, title: &str) {
        let Rect { x, y, w, h }: Rect = area;
        if w < 2 || h < 2 {
            return;
        }
        let fg: Rgb = if border.fg == border.bg { fill.bg } else { border.fg };
        let border: Style = Style { fg, bg: fill.bg, ..border };
        let mut top: Row = Row::new(w);
        top.cell(border, '╭');
        if !title.is_empty() {
            top.text(border, "─");
            top.text(theme().on(Group::FloatTitle, fill.bg), &format!(" {title} "));
        }
        while top.used() < w - 1 {
            top.cell(border, '─');
        }
        top.cell(border, '╮');
        self.put(x, y, &top.finish(border));
        for r in 1..h - 1 {
            let mut row: Row = Row::new(w);
            row.cell(border, '│');
            row.pad_to(fill, w - 1);
            row.cell(border, '│');
            self.put(x, y + r, &row.finish(fill));
        }
        let mut bottom: Row = Row::new(w);
        bottom.cell(border, '╰');
        while bottom.used() < w - 1 {
            bottom.cell(border, '─');
        }
        bottom.cell(border, '╯');
        self.put(x, y + h - 1, &bottom.finish(border));
    }
}

/// ANSI for one row of cells.
pub fn encode(cells: &[Cell], depth: ColorDepth) -> String {
    let mut out: String = String::with_capacity(cells.len() * 2);
    let mut current: Option<Style> = None;
    for cell in cells.iter().filter(|c| !c.wide_tail) {
        if current != Some(cell.style) {
            current = Some(cell.style);
            sgr(&mut out, cell.style, depth);
        }
        out.push(cell.ch);
        if let Some(mark) = cell.mark {
            out.push(mark);
        }
    }
    out
}

fn sgr(out: &mut String, style: Style, depth: ColorDepth) {
    out.push_str("\x1b[0");
    color(out, 38, style.fg, depth);
    color(out, 48, style.bg, depth);
    if style.bold {
        out.push_str(";1");
    }
    if style.italic {
        out.push_str(";3");
    }
    if style.underline {
        out.push_str(";4");
    }
    out.push('m');
}

fn color(out: &mut String, layer: u8, Rgb(c): Rgb, depth: ColorDepth) {
    let (r, g, b): (u32, u32, u32) = (c >> 16 & 0xff, c >> 8 & 0xff, c & 0xff);
    let _: fmt::Result = match depth {
        ColorDepth::TrueColor => write!(out, ";{layer};2;{r};{g};{b}"),
        ColorDepth::Ansi256 => write!(out, ";{layer};5;{}", ansi256(r, g, b)),
    };
}

/// Nearest entry of the xterm 6×6×6 color cube.
fn ansi256(r: u32, g: u32, b: u32) -> u32 {
    let level = |v: u32| {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (v - 35) / 40
        }
    };
    16 + 36 * level(r) + 6 * level(g) + level(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Over any background, a float's border and inside share one color: no band around the panel.
    #[test]
    fn float_is_one_block() {
        let (under, panel): (Style, Style) =
            (Style::new(Rgb(0x111111), Rgb(0xaa0000)), Style::new(Rgb(0xeeeeee), Rgb(0x222222)));
        let mut grid: Grid = Grid::new(10, 5, under);
        let border: Style = Style::new(Rgb(0x888888), Rgb(0x333333));
        grid.float(Rect { x: 1, y: 1, w: 6, h: 3 }, border, panel, "");
        for y in 1..4 {
            assert!(grid.row(y)[1..7].iter().all(|c| c.style.bg == panel.bg), "row {y}");
            assert_eq!(grid.row(y)[0].style.bg, under.bg, "outside stays");
        }
        assert_eq!(grid.row(1)[1].ch, '╭');
        assert_eq!(grid.row(2)[1].style.fg, border.fg, "a set border color shows");
        let borderless: Style = Style::new(Rgb(0x333333), Rgb(0x333333));
        grid.float(Rect { x: 1, y: 1, w: 6, h: 3 }, borderless, panel, "t");
        assert_eq!(grid.row(2)[1].style.fg, panel.bg, "fg == bg: the frame vanishes into the panel");
        assert_eq!(grid.row(1)[4].style.bg, theme().bg(Group::FloatTitle), "the title is a pill");
    }

    #[test]
    fn rows_grid_and_encoding() {
        let style: Style = Style::new(Rgb(0xffffff), Rgb(0));
        let mut row: Row = Row::new(4);
        row.text(style, "a\x1b[2Jbcdef");
        assert_eq!(encode(&row.finish(style), ColorDepth::Ansi256), "\x1b[0;38;5;231;48;5;16ma?[2");
        let mut row: Row = Row::new(4);
        row.text(style, "a\u{202e}b\u{200f}");
        assert_eq!(encode(&row.finish(style), ColorDepth::Ansi256), "\x1b[0;38;5;231;48;5;16ma?b?", "bidi controls");
        let mut row: Row = Row::new(4);
        row.text(style, "a日本"); // 本 needs 2 cells, 1 is left: padded instead
        let cells: Vec<Cell> = row.finish(style);
        assert_eq!(cells.len(), 4);
        assert!(encode(&cells, ColorDepth::TrueColor).ends_with("a日 "));
        let mut grid: Grid = Grid::new(4, 1, style);
        grid.put(0, 0, &cells);
        grid.text(2, 0, 2, style, "xy"); // overwrites the tail of 日: its head becomes blank
        assert!(encode(grid.row(0), ColorDepth::TrueColor).ends_with("a xy"));
        assert_eq!(ansi256(0x50, 0xA4, 0xE9), 74);
    }
}
