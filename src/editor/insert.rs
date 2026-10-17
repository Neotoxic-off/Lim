//! Insert mode, with nvim-autopairs-style bracket and quote pairing.

use super::{Editor, Mode};
use crate::buffer::{Buffer, Pos, text};
use crate::term::Key;

const PAIRS: [(char, char); 6] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`')];
/// nvim-autopairs' `ignored_next_char`: no pair is added before these.
const BLOCKS_PAIR: &str = "%'[\".`$";

fn closing(open: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == open).map(|&(_, c)| c)
}

impl Editor {
    #[expect(clippy::too_many_lines, reason = "flat key dispatch: one arm per key")]
    pub(super) fn insert(&mut self, key: Key) {
        // `<C-r>{register}`: the key after `<C-r>` names the register to insert.
        if std::mem::take(&mut self.insert_register) {
            if let Key::Char(name) = key {
                self.register_name = Some(name);
                if let Some(reg) = self.fetch_register() {
                    let text: String = if reg.linewise { format!("{}\n", reg.text) } else { reg.text };
                    let buf: &mut Buffer = self.buf_mut();
                    let at: Pos = buf.cursor;
                    buf.cursor = buf.insert(at, &text);
                }
            }
            return;
        }
        if self.completion_key(key) {
            return;
        }
        match key {
            // Copy a block insert to the other lines while the cursor is still after the text.
            Key::Esc | Key::Ctrl('c') => self.finish_block_insert(),
            Key::Ctrl('r') => {
                self.insert_register = true;
                return;
            }
            Key::Ctrl('o') => {
                self.insert_once = true;
                self.mode = Mode::Normal;
                return;
            }
            Key::Ctrl('t' | 'd') => {
                let line: usize = self.buf().cursor.line;
                let before: usize = self.buf().line_len(line);
                self.shift_keep_cursor(key == Key::Ctrl('t'), line);
                let buf: &mut Buffer = self.buf_mut();
                let after: usize = buf.line_len(line);
                buf.cursor.col = (buf.cursor.col + after).saturating_sub(before);
                return;
            }
            _ => {}
        }
        let page: usize = self.view_height.max(1);
        let buf: &mut Buffer = &mut self.buffers[self.current];
        let here: Pos = buf.cursor;
        let line: &str = buf.line(here.line);
        let byte: usize = text::byte_index(line, here.col);
        let prev: Option<char> = line[..byte].chars().next_back();
        let next: Option<char> = line[byte..].chars().next();
        let between_pair: bool = self.options.autopairs && prev.and_then(closing).is_some_and(|c| next == Some(c));
        let left_blank: bool = self.auto_indent == Some(here.line) && line.trim().is_empty();
        match key {
            Key::F(7) => {
                buf.cursor.col = here.col.saturating_sub(1);
                self.mode = Mode::Normal;
                self.shell_requested = Some(Vec::new());
            }
            Key::Esc | Key::Ctrl('c') => {
                if left_blank {
                    buf.set_line(here.line, String::new());
                }
                buf.marks.insert('^', here);
                buf.cursor.col = here.col.saturating_sub(1);
                self.auto_indent = None;
                self.mode = Mode::Normal;
            }
            Key::Char(c) => {
                self.auto_indent = None;
                self.type_char(c, prev, next);
                self.completion_stale = true;
            }
            Key::Enter | Key::Ctrl('j') => {
                let indent: String = text::indent(line).to_owned();
                let opens: bool = matches!(prev, Some('(' | '[' | '{'));
                let indent: String = if self.options.autoindent { indent } else { String::new() };
                let step: String =
                    if self.options.expandtab { " ".repeat(self.options.indent_width()) } else { "\t".to_owned() };
                let inner: String =
                    if opens && self.options.autoindent { format!("{indent}{step}") } else { indent.clone() };
                let at: Pos = if left_blank {
                    buf.set_line(here.line, String::new());
                    Pos::new(here.line, 0)
                } else {
                    here
                };
                let split: String =
                    if opens && between_pair { format!("\n{inner}\n{indent}") } else { format!("\n{inner}") };
                buf.insert(at, &split);
                buf.cursor = Pos::new(here.line + 1, text::char_len(&inner));
                self.auto_indent = (!inner.is_empty()).then_some(here.line + 1);
            }
            Key::Backspace | Key::Ctrl('h') if here.col > 0 => {
                let end: Pos = Pos::new(here.line, here.col + usize::from(between_pair));
                buf.remove(Pos::new(here.line, here.col - 1), end);
                buf.cursor.col -= 1;
                self.completion_stale |= self.completion.is_some();
            }
            Key::Backspace | Key::Ctrl('h') if here.line > 0 => {
                let col: usize = buf.line_len(here.line - 1);
                buf.remove(Pos::new(here.line - 1, col), here);
                buf.cursor = Pos::new(here.line - 1, col);
            }
            Key::Delete if next.is_some() => {
                buf.remove(here, Pos::new(here.line, here.col + 1));
            }
            Key::Delete if here.line < buf.last_line() => {
                buf.remove(here, Pos::new(here.line + 1, 0));
            }
            Key::Tab => {
                let typed: String = if self.options.expandtab {
                    " ".repeat(text::tab_cells(text::display_col(line, here.col)))
                } else {
                    "\t".to_owned()
                };
                buf.insert(here, &typed);
                buf.cursor.col += text::char_len(&typed);
            }
            Key::Ctrl('w') if here.col > 0 => {
                let back: Pos = buf.word_back(here, false);
                let start: Pos = if back.line == here.line { back } else { Pos::new(here.line, 0) };
                buf.remove(start, here);
                buf.cursor = start;
            }
            Key::Ctrl('u') => {
                buf.remove(Pos::new(here.line, 0), here);
                buf.cursor.col = 0;
            }
            Key::Left => buf.cursor.col = here.col.saturating_sub(1),
            Key::Right => buf.cursor.col += 1,
            Key::Up => buf.cursor = Pos::new(here.line.saturating_sub(1), buf.want_col),
            Key::Down => buf.cursor = Pos::new((here.line + 1).min(buf.last_line()), buf.want_col),
            Key::ShiftLeft => buf.cursor = buf.word_back(here, false),
            Key::ShiftRight => buf.cursor = buf.word_forward(here, false),
            Key::ShiftUp => buf.cursor = Pos::new(here.line.saturating_sub(page), buf.want_col),
            Key::ShiftDown => buf.cursor = Pos::new((here.line + page).min(buf.last_line()), buf.want_col),
            Key::Home => buf.cursor.col = 0,
            Key::End => buf.cursor.col = usize::MAX,
            _ => {}
        }
    }

    fn type_char(&mut self, c: char, prev: Option<char>, next: Option<char>) {
        let autopairs: bool = self.options.autopairs;
        let buf: &mut Buffer = self.buf_mut();
        let quotes: &str = buf.lang.map_or("\"'`", |l| l.quotes);
        let closer: bool = matches!(c, ')' | ']' | '}') || quotes.contains(c);
        if autopairs && closer && next == Some(c) {
            buf.cursor.col += 1;
            return;
        }
        let mut typed: String = String::from(c);
        if let Some(close) = closing(c).filter(|_| autopairs) {
            let is_quote: bool = close == c;
            let blocked: bool = next.is_some_and(|n| n.is_alphanumeric() || BLOCKS_PAIR.contains(n))
                || (is_quote && (!quotes.contains(c) || prev.is_some_and(char::is_alphanumeric)));
            if !blocked {
                typed.push(close);
            }
        }
        let here: Pos = buf.cursor;
        buf.insert(here, &typed);
        buf.cursor.col += 1;
    }
}
