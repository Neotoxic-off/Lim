//! Visual block (`<C-v>`): rectangular delete, yank, change, insert/append on every line,
//! replace, case and blockwise put.
// shortcut: columns are chars, not screen cells; tabs and wide chars in a block select unevenly. Switch to display
// columns if block edits on tab-indented or CJK files come up.

use super::registers::Register;
use super::vim::{Case, convert};
use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos};

/// Text typed into the first line of a block insert, copied to the others on `<Esc>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockInsert {
    pub top: usize,
    pub bottom: usize,
    pub col: usize,
    /// `A`: lines shorter than `col` are padded so the text lines up.
    pub pad: bool,
}

impl Editor {
    fn block_bounds(&self) -> (usize, usize, usize, usize) {
        let (a, b): (Pos, Pos) = (self.anchor, self.buf().cursor);
        (a.line.min(b.line), a.line.max(b.line), a.col.min(b.col), a.col.max(b.col))
    }

    /// `d`, `x`, `y`, `c` on the block; the register keeps it as a column.
    pub(super) fn block_cut(&mut self, delete: bool, change: bool) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        let mut pieces: Vec<String> = Vec::with_capacity(bottom - top + 1);
        for y in top..=bottom {
            let len: usize = buf.line_len(y);
            let (start, end): (Pos, Pos) = (Pos::new(y, left.min(len)), Pos::new(y, (right + 1).min(len)));
            pieces.push(buf.text(start, end));
            if delete && start != end {
                buf.remove(start, end);
            }
        }
        buf.cursor = Pos::new(top, left.min(buf.line_len(top)));
        self.mode = Mode::Normal;
        self.store_register(Register { text: pieces.join("\n"), linewise: false, block: true }, !delete);
        if change {
            self.block_insert = Some(BlockInsert { top, bottom, col: left, pad: false });
            self.mode = Mode::Insert;
        }
    }

    /// `I` / `A` in block mode.
    pub(super) fn block_insert_start(&mut self, append: bool) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let col: usize = if append { right + 1 } else { left };
        let buf: &mut Buffer = self.buf_mut();
        let len: usize = buf.line_len(top);
        if col > len {
            buf.insert(Pos::new(top, len), &" ".repeat(col - len));
        }
        buf.cursor = Pos::new(top, col);
        self.block_insert = Some(BlockInsert { top, bottom, col, pad: append });
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
            let len: usize = buf.line_len(y);
            if len < block.col {
                if !block.pad {
                    continue;
                }
                buf.insert(Pos::new(y, len), &" ".repeat(block.col - len));
            }
            buf.insert(Pos::new(y, block.col), &typed);
        }
    }

    /// `r{c}` in block mode.
    pub(super) fn block_replace(&mut self, ch: char) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        for y in top..=bottom {
            let len: usize = buf.line_len(y);
            if left >= len {
                continue;
            }
            let end: usize = (right + 1).min(len);
            buf.remove(Pos::new(y, left), Pos::new(y, end));
            buf.insert(Pos::new(y, left), &ch.to_string().repeat(end - left));
        }
        buf.cursor = Pos::new(top, left);
        self.mode = Mode::Normal;
    }

    /// `~`, `u`, `U` in block mode.
    pub(super) fn block_case(&mut self, case: Case) {
        let (top, bottom, left, right): (usize, usize, usize, usize) = self.block_bounds();
        let buf: &mut Buffer = self.buf_mut();
        for y in top..=bottom {
            let len: usize = buf.line_len(y);
            let (start, end): (Pos, Pos) = (Pos::new(y, left.min(len)), Pos::new(y, (right + 1).min(len)));
            let converted: String = convert(&buf.text(start, end), case);
            buf.remove(start, end);
            buf.insert(start, &converted);
        }
        buf.cursor = Pos::new(top, left);
        self.mode = Mode::Normal;
    }

    /// `p` / `P` of a block register: one piece per line, starting at the cursor column.
    pub(super) fn put_block(&mut self, reg: &Register, after: bool) {
        let buf: &mut Buffer = self.buf_mut();
        let here: Pos = buf.cursor;
        let col: usize = if after && buf.line_len(here.line) > 0 { here.col + 1 } else { here.col };
        for (i, piece) in reg.text.split('\n').enumerate() {
            let y: usize = here.line + i;
            if y > buf.last_line() {
                let end: usize = buf.line_len(buf.last_line());
                buf.insert(Pos::new(buf.last_line(), end), "\n");
            }
            let len: usize = buf.line_len(y);
            if len < col {
                buf.insert(Pos::new(y, len), &" ".repeat(col - len));
            }
            buf.insert(Pos::new(y, col), piece);
        }
        buf.cursor = Pos::new(here.line, col);
    }
}
