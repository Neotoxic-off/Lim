//! Visual block (`<C-v>`): rectangular delete, yank, change, insert/append on every line,
//! replace, case and blockwise put. The block's columns are screen cells, so tabs and wide chars line up as drawn.

use super::registers::Register;
use super::vim::{Case, convert};
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos, text};

/// Text typed into the first line of a block insert, copied to the others on `<Esc>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockInsert {
    pub top: usize,
    pub bottom: usize,
    /// Char column of the insert on the first line.
    pub col: usize,
    /// Screen cell of the insert, where it goes on the other lines.
    pub cell: usize,
    /// `A`: lines shorter than `cell` are padded so the text lines up.
    pub pad: bool,
    /// `$A`: the text goes at the end of every line instead of at `cell`.
    pub eol: bool,
}

/// Screen cells `start..end` of the char at `col` (one cell past the end of the line).
fn cells(line: &str, col: usize) -> (usize, usize) {
    let start: usize = text::display_col(line, col);
    let width: usize = match line.chars().nth(col) {
        Some('\t') => text::tab_cells(start),
        Some(c) => text::char_width(c),
        None => 1,
    };
    (start, start + width.max(1))
}

/// Char columns `start..end` of the chars of `line` that start in cells `left..=right`.
fn span(line: &str, left: usize, right: usize) -> (usize, usize) {
    let mut cell: usize = 0;
    let mut start: Option<usize> = None;
    for (i, c) in line.chars().enumerate() {
        if cell > right {
            return (start.unwrap_or(i), i);
        }
        if start.is_none() && cell >= left {
            start = Some(i);
        }
        cell += if c == '\t' { text::tab_cells(cell) } else { text::char_width(c) };
    }
    let len: usize = text::char_len(line);
    (start.unwrap_or(len), len)
}

/// The char column where cell `cell` starts, or `Err(n)` when the line is `n` cells too short to reach it.
fn col_for(line: &str, cell: usize) -> Result<usize, usize> {
    let mut width: usize = 0;
    for (i, c) in line.chars().enumerate() {
        if width >= cell {
            return Ok(i);
        }
        width += if c == '\t' { text::tab_cells(width) } else { text::char_width(c) };
    }
    if width >= cell { Ok(text::char_len(line)) } else { Err(cell - width) }
}

/// The char column of cell `cell` on line `y`, padding the line with spaces when it is too short.
fn padded_col(buf: &mut Buffer, y: usize, cell: usize) -> usize {
    match col_for(buf.line(y), cell) {
        Ok(col) => col,
        Err(missing) => {
            let len: usize = buf.line_len(y);
            buf.insert(Pos::new(y, len), &" ".repeat(missing));
            len + missing
        }
    }
}

impl Editor {
    /// Lines `top..=bottom` and screen cells `left..=right` of the block.
    pub(super) fn block_bounds(&self) -> (usize, usize, usize, usize) {
        let (a, b): (Pos, Pos) = (self.anchor, self.buf().cursor);
        let (a_cells, b_cells): ((usize, usize), (usize, usize)) =
            (cells(self.buf().line(a.line.min(self.buf().last_line())), a.col), cells(self.buf().line(b.line), b.col));
        let right: usize = if self.block_to_eol { usize::MAX - 1 } else { a_cells.1.max(b_cells.1) - 1 };
        (a.line.min(b.line), a.line.max(b.line), a_cells.0.min(b_cells.0), right)
    }

    /// `d`, `x`, `y`, `c` on the block; the register keeps it as a column.
    pub(super) fn block_cut(&mut self, delete: bool, change: bool) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        let mut pieces: Vec<String> = Vec::with_capacity(bottom - top + 1);
        for y in top..=bottom {
            let (start, end): (usize, usize) = span(buf.line(y), left, right);
            let (start, end): (Pos, Pos) = (Pos::new(y, start), Pos::new(y, end));
            pieces.push(buf.text(start, end));
            if delete && start != end {
                buf.remove(start, end);
            }
        }
        let col: usize = span(buf.line(top), left, right).0;
        buf.cursor = Pos::new(top, col);
        self.mode = Mode::Normal;
        self.store_register(Register { text: pieces.join("\n"), linewise: false, block: true }, !delete);
        if change {
            self.block_insert = Some(BlockInsert { top, bottom, col, cell: left, pad: false, eol: false });
            self.mode = Mode::Insert;
        }
    }

    /// `I` / `A` in block mode.
    pub(super) fn block_insert_start(&mut self, append: bool) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let eol: bool = append && self.block_to_eol;
        let buf: &mut Buffer = self.buf_mut();
        let cell: usize = if eol {
            text::str_width(buf.line(top))
        } else if append {
            right + 1
        } else {
            left
        };
        let col: usize = if eol { buf.line_len(top) } else { padded_col(buf, top, cell) };
        buf.cursor = Pos::new(top, col);
        self.block_insert = Some(BlockInsert { top, bottom, col, cell, pad: append && !eol, eol });
        self.mode = Mode::Insert;
    }

    /// On leaving insert mode: repeat what was typed on the block's first line on the others.
    pub(super) fn finish_block_insert(&mut self) {
        let Some(block): Option<BlockInsert> = self.block_insert.take() else { return };
        let buf: &mut Buffer = self.buf_mut();
        let cursor: Pos = buf.cursor;
        if cursor.line != block.top || cursor.col <= block.col {
            return;
        }
        let typed: String = buf.text(Pos::new(block.top, block.col), cursor);
        for y in block.top + 1..=block.bottom.min(buf.last_line()) {
            let col: usize = if block.eol {
                buf.line_len(y)
            } else if block.pad {
                padded_col(buf, y, block.cell)
            } else {
                let Ok(col): Result<usize, usize> = col_for(buf.line(y), block.cell) else { continue };
                col
            };
            buf.insert(Pos::new(y, col), &typed);
        }
    }

    /// `r{c}` in block mode.
    pub(super) fn block_replace(&mut self, ch: char) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        for y in top..=bottom {
            let (start, end): (usize, usize) = span(buf.line(y), left, right);
            if start == end {
                continue;
            }
            buf.remove(Pos::new(y, start), Pos::new(y, end));
            buf.insert(Pos::new(y, start), &ch.to_string().repeat(end - start));
        }
        buf.cursor = Pos::new(top, span(buf.line(top), left, right).0);
        self.mode = Mode::Normal;
    }

    /// `~`, `u`, `U` in block mode.
    pub(super) fn block_case(&mut self, case: Case) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        for y in top..=bottom {
            let (start, end): (usize, usize) = span(buf.line(y), left, right);
            let (start, end): (Pos, Pos) = (Pos::new(y, start), Pos::new(y, end));
            let converted: String = convert(&buf.text(start, end), case);
            buf.remove(start, end);
            buf.insert(start, &converted);
        }
        buf.cursor = Pos::new(top, span(buf.line(top), left, right).0);
        self.mode = Mode::Normal;
    }

    /// `p` / `P` of a block register: one piece per line, starting at the cursor's screen cell.
    pub(super) fn put_block(&mut self, reg: &Register, after: bool) {
        let buf: &mut Buffer = self.buf_mut();
        let here: Pos = buf.cursor;
        let (start, end): (usize, usize) = cells(buf.line(here.line), here.col);
        let cell: usize = if after && buf.line_len(here.line) > 0 { end } else { start };
        for (i, piece) in reg.text.split('\n').enumerate() {
            let y: usize = here.line + i;
            if y > buf.last_line() {
                let end: usize = buf.line_len(buf.last_line());
                buf.insert(Pos::new(buf.last_line(), end), "\n");
            }
            let col: usize = padded_col(buf, y, cell);
            buf.insert(Pos::new(y, col), piece);
        }
        let col: usize = col_for(buf.line(here.line), cell).unwrap_or(here.col);
        buf.cursor = Pos::new(here.line, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_spans_and_columns() {
        assert_eq!(cells("\tx", 0), (0, 4), "tabstop 4");
        assert_eq!(cells("日x", 1), (2, 3));
        assert_eq!(cells("ab", 5), (2, 3), "past the end");
        assert_eq!(span("日本x", 2, 3), (1, 2));
        assert_eq!(span("\tab", 5, 5), (2, 3));
        assert_eq!(span("ab", 4, 6), (2, 2));
        assert_eq!(col_for("\tab", 5), Ok(2));
        assert_eq!(col_for("ab", 2), Ok(2));
        assert_eq!(col_for("ab", 5), Err(3));
    }
}
