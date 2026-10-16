//! Normal mode: counts, operators, two-key prefixes and single-key commands.

use super::command::Command;
use super::motion::{Motion, Span};
use super::operator::Operator;
use super::picker::PickerKind;
use super::manage::CloseSet;
use super::ui::{CTRL_W, LEADER_B, LEADER_F, LEADER_G, LEADER_T, LEADER_U};
use super::window::Direction;
use super::{Editor, Mode, Pending, PickAction, Register};
use crate::buffer::{CharClass, Pos, text};
use crate::config::keys::LEADER;
use crate::error::Error;
use crate::term::Key;

/// Keys that wait for one more: `gg`, `f{c}`, `r{c}`, `ZZ`, `]b`, and the leader.
const PREFIXES: &str = "gfFtTrZq@[]\u{F8FF}";

/// Prefixes visual mode takes: motions, text objects (`iw`), `gc` and the leader.
pub(super) const VISUAL_PREFIXES: &str = "gfFtTia\u{F8FF}";

impl Editor {
    pub(super) fn normal(&mut self, key: Key) {
        self.buf_mut().seal();
        // With a command line, a message stays until the next command; as a notification it fades.
        if self.pending.is_empty() && (self.options.cmdheight > 0 || self.notification_expired()) {
            self.message = None;
        }
        if let Some(prefix) = self.pending.prefix.take() {
            return self.prefixed(prefix, key);
        }
        if key == Key::F(7) {
            self.shell_requested = true;
            return;
        }
        // snacks.dashboard: its own keys; everything that edits text is ignored while it shows.
        if self.dashboard && self.pending.is_empty() {
            let picker = match key {
                Key::Char('n') => {
                    self.dashboard = false;
                    return self.new_buffer();
                }
                Key::Char('f') => Some(PickerKind::Files { all: false }),
                Key::Char('o') => Some(PickerKind::Recent),
                Key::Char('w') => Some(PickerKind::Grep { all: false }),
                Key::Char(':' | LEADER) | Key::Ctrl(_) | Key::F(_) | Key::Tab | Key::BackTab => None,
                _ => return,
            };
            if let Some(kind) = picker {
                return self.open_picker(kind);
            }
        }
        if key == Key::Char('q') && self.recording().is_some() {
            return self.stop_recording();
        }
        // `i`/`a` after an operator start a text object (`diw`, `ca(`), otherwise they insert.
        let objects = if self.pending.op.is_some() { "ia" } else { "" };
        if self.push_count(key) || self.push_prefix(key, PREFIXES) || self.push_prefix(key, objects) {
            return;
        }
        // AstroNvim maps `>b` / `<b` to moving the buffer tab, over indent + `b`.
        if let Some(op @ (Operator::Indent | Operator::Dedent)) = self.pending.op
            && key == Key::Char('b')
        {
            self.pending = Pending::default();
            return self.move_buffer(if op == Operator::Indent { 1 } else { -1 });
        }
        if let Some(motion) = Motion::from_key(key) {
            return self.run_motion(motion);
        }
        let count = self.pending.count.take();
        let n = count.unwrap_or(1);
        if let Some(op) = self.pending.op.take() {
            if key == Key::Char(op.key()) {
                self.apply_count_lines(op, n);
            }
            return;
        }
        if let Key::Char(c) = key
            && let Some(op) = Operator::from_char(c)
        {
            self.pending = Pending { count, op: Some(op), prefix: None };
            return;
        }
        let here = self.buf().cursor;
        let len = self.buf().line_len(here.line);
        let at = |col: usize| Pos::new(here.line, col);
        match key {
            Key::Char('i') => self.start_insert(here.col),
            Key::Char('a') => self.start_insert((here.col + 1).min(len)),
            Key::Char('I') => self.start_insert(self.buf().first_non_blank(here.line)),
            Key::Char('A') => self.start_insert(len),
            Key::Char('o') => self.open_line(true),
            Key::Char('O') => self.open_line(false),
            Key::Char('x') | Key::Delete => self.apply(Operator::Delete, here, at(here.col + n), Span::Exclusive),
            Key::Char('X') => self.apply(Operator::Delete, at(here.col.saturating_sub(n)), here, Span::Exclusive),
            Key::Char('s') => self.apply(Operator::Change, here, at(here.col + n), Span::Exclusive),
            Key::Char('D') => self.apply(Operator::Delete, here, at(len), Span::Exclusive),
            Key::Char('C') => self.apply(Operator::Change, here, at(len), Span::Exclusive),
            Key::Char('S') => self.apply_count_lines(Operator::Change, n),
            Key::Char('Y') => self.apply_count_lines(Operator::Yank, n),
            Key::Char('p') => self.paste(true, n),
            Key::Char('P') => self.paste(false, n),
            Key::Char('u') => self.undo(n, false),
            Key::Ctrl('r') => self.undo(n, true),
            Key::Char('J') => self.join(n),
            Key::Char('~') => self.toggle_case(n),
            Key::Char('.') => self.repeat_change(n),
            Key::Char('*') => self.search_word(true),
            Key::Char('#') => self.search_word(false),
            Key::Char('v') => self.start_visual(Mode::Visual),
            Key::Char('V') => self.start_visual(Mode::VisualLine),
            Key::Char(':') => self.start_cmdline(Mode::Command),
            Key::Char('/') => self.start_cmdline(Mode::Search),
            Key::Char('|') => self.split(true),
            Key::Char('\\') => self.split(false),
            Key::Ctrl('h') => self.focus_direction(Direction::Left),
            Key::Ctrl('j') => self.focus_direction(Direction::Down),
            Key::Ctrl('k') => self.focus_direction(Direction::Up),
            Key::Ctrl('l') => self.focus_direction(Direction::Right),
            Key::Ctrl('w') => self.pending.prefix = Some(CTRL_W),
            Key::Tab => self.cycle(n as isize),
            Key::BackTab => self.cycle(-(n as isize)),
            Key::Ctrl('s') => self.run(Command::Write { path: None, force: false }),
            Key::Ctrl('q') => self.run(Command::Quit { force: true }),
            _ => {}
        }
    }

    pub(super) fn push_count(&mut self, key: Key) -> bool {
        let Key::Char(c @ '0'..='9') = key else { return false };
        if c == '0' && self.pending.count.is_none() {
            return false;
        }
        let digit = c as usize - '0' as usize;
        self.pending.count = Some((self.pending.count.unwrap_or(0) * 10 + digit).min(99_999));
        true
    }

    pub(super) fn push_prefix(&mut self, key: Key, allowed: &str) -> bool {
        match key {
            Key::Char(c) if allowed.contains(c) => {
                self.pending.prefix = Some(c);
                true
            }
            _ => false,
        }
    }

    pub(super) fn prefixed(&mut self, prefix: char, key: Key) {
        let c = match key {
            Key::Char(c) => c,
            Key::Ctrl(c) if prefix == CTRL_W => c,
            Key::Enter if prefix == LEADER_F => '\r',
            _ => {
                self.pending = Pending::default();
                return;
            }
        };
        let n = self.pending.count.unwrap_or(1);
        match (prefix, c) {
            ('g', 'g') => return self.run_motion(Motion::FileStart),
            ('f' | 'F' | 't' | 'T', ch) => {
                let till = matches!(prefix, 't' | 'T');
                return self.run_motion(Motion::Find { ch, forward: prefix.is_ascii_lowercase(), till });
            }
            ('r', ch) => self.replace_chars(ch, n),
            ('i' | 'a', obj) => self.select_object(prefix == 'a', obj),
            ('g', 'c') | (LEADER, '/') if self.mode != Mode::Normal => {
                let span = if self.mode == Mode::Visual { Span::Inclusive } else { Span::Linewise };
                let cursor = self.buf().cursor;
                self.mode = Mode::Normal;
                self.apply(Operator::Comment, self.anchor, cursor, span);
            }
            ('g', 'c') => {
                self.pending.op = Some(Operator::Comment);
                return;
            }
            (LEADER, '/') => self.apply_count_lines(Operator::Comment, n),
            ('q', reg) => self.start_recording(reg),
            ('@', reg) => self.play_macro(reg, n),
            (LEADER, sub @ ('b' | 'f' | 'g' | 't' | 'u')) => {
                self.pending.prefix = Some(match sub {
                    'b' => LEADER_B,
                    'f' => LEADER_F,
                    'g' => LEADER_G,
                    't' => LEADER_T,
                    _ => LEADER_U,
                });
                return;
            }
            (LEADER_U, c) => self.toggle(c),
            (LEADER, 'e') => self.toggle_explorer(),
            (LEADER, 'o') => self.focus_explorer(),
            (LEADER, 'h') => self.dashboard = !self.dashboard,
            (LEADER_T, 'f' | 'h' | 'v') => self.shell_requested = true,
            (LEADER_F, 'f') => self.open_picker(PickerKind::Files { all: false }),
            (LEADER_F, 'F') => self.open_picker(PickerKind::Files { all: true }),
            (LEADER_F, 'g') => self.open_picker(PickerKind::GitFiles),
            (LEADER_F, 'w') => self.open_picker(PickerKind::Grep { all: false }),
            (LEADER_F, 'W') => self.open_picker(PickerKind::Grep { all: true }),
            (LEADER_F, 'b') => self.open_picker(PickerKind::Buffers),
            (LEADER_F, 'o') => self.open_picker(PickerKind::Recent),
            (LEADER_F, 'k') => self.open_picker(PickerKind::Keymaps),
            (LEADER_F, 'C') => self.open_picker(PickerKind::Commands),
            (LEADER_F, 'l') => self.open_picker(PickerKind::Lines),
            (LEADER_F, 'c') => self.grep_word(),
            (LEADER_F, '\r') => self.resume_picker(),
            (LEADER_G, 'g') | (']', 'g') => self.jump_hunk(true),
            (LEADER_G, 'G') | ('[', 'g') => self.jump_hunk(false),
            (']', 'b') => self.cycle(n as isize),
            ('[', 'b') => self.cycle(-(n as isize)),
            (LEADER, 'c') => self.run(Command::BufferDelete { force: false }),
            (LEADER, 'C') => self.run(Command::BufferDelete { force: true }),
            (LEADER, 'Q') => self.run(Command::QuitAll { force: false }),
            (LEADER_B, 'b') => self.mode = Mode::BufferPick(PickAction::Go),
            (LEADER_B, 'd') => self.mode = Mode::BufferPick(PickAction::Close),
            (LEADER_B, '\\') => self.mode = Mode::BufferPick(PickAction::SplitBelow),
            (LEADER_B, '|') => self.mode = Mode::BufferPick(PickAction::SplitRight),
            (LEADER_B, 'c') => self.close_buffers(CloseSet::Others),
            (LEADER_B, 'C') => self.close_buffers(CloseSet::All),
            (LEADER_B, 'l') => self.close_buffers(CloseSet::Left),
            (LEADER_B, 'r') => self.close_buffers(CloseSet::Right),
            (LEADER_B, 'p') => self.go_previous_buffer(),
            (CTRL_W, 'v') => self.split(true),
            (CTRL_W, 's') => self.split(false),
            (CTRL_W, 'c' | 'q') => self.run(Command::CloseWindow),
            (CTRL_W, 'o') => self.only_window(),
            (CTRL_W, 'w') => self.focus_next_window(),
            (CTRL_W, 'h') => self.focus_direction(Direction::Left),
            (CTRL_W, 'j') => self.focus_direction(Direction::Down),
            (CTRL_W, 'k') => self.focus_direction(Direction::Up),
            (CTRL_W, 'l') => self.focus_direction(Direction::Right),
            (LEADER, 'w') => self.run(Command::Write { path: None, force: false }),
            (LEADER, 'q') => self.run(Command::Quit { force: false }),
            (LEADER, 'n') => self.run(Command::New),
            ('Z', 'Z') => self.run(Command::WriteQuit { path: None, force: false, only_if_dirty: true }),
            ('Z', 'Q') => self.run(Command::Quit { force: true }),
            _ => {}
        }
        self.pending = Pending::default();
    }

    /// Moves the cursor, or applies the pending operator up to the motion target.
    pub(super) fn run_motion(&mut self, motion: Motion) {
        let Pending { count, op, .. } = std::mem::take(&mut self.pending);
        let buf = self.buf();
        let from = buf.cursor;
        let Some(mut target) = motion.target(buf, count, self.view_height, &self.search) else {
            if let Motion::SearchNext { .. } = motion {
                let error = if self.search.is_empty() {
                    Error::NoPattern
                } else {
                    Error::PatternNotFound(self.search.clone())
                };
                self.error(error);
            }
            return;
        };
        let mut span = motion.span();
        // Vim quirks: `cw` on a word acts like `ce`; `dw` on a line's last word keeps the line break.
        if let Some(op) = op
            && motion == Motion::WordForward
        {
            if op == Operator::Change && buf.class_at(from) != CharClass::Blank {
                target = (0..count.unwrap_or(1))
                    .fold(from, |p, i| if i == 0 && buf.at_word_end(p) { p } else { buf.word_end(p) });
                span = Span::Inclusive;
            } else if target.line > from.line {
                target = Pos::new(from.line, buf.line_len(from.line));
            }
        }
        if let Motion::SearchNext { .. } = motion {
            self.highlight_search = true;
        }
        match op {
            Some(op) => self.apply(op, from, target, span),
            None => self.buf_mut().cursor = target,
        }
    }

    /// Text object under the cursor: operated on when an operator waits, else selected (visual mode).
    fn select_object(&mut self, around: bool, obj: char) {
        let buf = self.buf();
        let at = buf.cursor;
        let range = match obj {
            'w' => buf.word_object(at, around),
            '(' | ')' | 'b' => buf.pair_object(at, '(', ')', around),
            '[' | ']' => buf.pair_object(at, '[', ']', around),
            '{' | '}' | 'B' => buf.pair_object(at, '{', '}', around),
            '<' | '>' => buf.pair_object(at, '<', '>', around),
            '"' | '\'' | '`' => buf.quote_object(at, obj, around),
            _ => None,
        };
        let Some((start, end)) = range else { return };
        match self.pending.op {
            Some(op) => self.apply(op, start, end, Span::Exclusive),
            None if self.mode != Mode::Normal && start != end => {
                self.anchor = start;
                self.buf_mut().cursor = if end.col > 0 { Pos::new(end.line, end.col - 1) } else { end };
            }
            None => {}
        }
    }

    /// `*` / `#`: search the word under the cursor as a whole word.
    fn search_word(&mut self, forward: bool) {
        let buf = self.buf();
        let Some((start, end)) = buf.word_object(buf.cursor, false) else { return };
        if buf.class_at(start) != CharClass::Word {
            return;
        }
        self.search = format!("\\<{}\\>", buf.text(start, end));
        self.buf_mut().cursor = start;
        self.run_motion(Motion::SearchNext { forward });
    }

    fn jump_hunk(&mut self, forward: bool) {
        let buf = self.buf_mut();
        match buf.next_hunk(buf.cursor.line, forward) {
            Some(line) => buf.cursor = Pos::new(line, buf.first_non_blank(line)),
            None => self.info("No hunks"),
        }
    }

    fn apply_count_lines(&mut self, op: Operator, n: usize) {
        let buf = self.buf();
        let first = buf.cursor.line;
        let last = (first + n - 1).min(buf.last_line());
        self.apply(op, Pos::new(first, 0), Pos::new(last, 0), Span::Linewise);
    }

    fn start_insert(&mut self, col: usize) {
        self.buf_mut().cursor.col = col;
        self.mode = Mode::Insert;
    }

    fn start_visual(&mut self, mode: Mode) {
        self.anchor = self.buf().cursor;
        self.mode = mode;
    }

    fn start_cmdline(&mut self, mode: Mode) {
        self.cmdline.clear();
        self.mode = mode;
    }

    fn open_line(&mut self, below: bool) {
        let buf = self.buf_mut();
        let line = buf.cursor.line;
        let indent = text::indent(buf.line(line)).to_owned();
        let col = text::char_len(&indent);
        let at = line + usize::from(below);
        buf.splice(at..at, vec![indent]);
        buf.cursor = Pos::new(at, col);
        self.auto_indent = (col > 0).then_some(at);
        self.mode = Mode::Insert;
    }

    fn paste(&mut self, after: bool, n: usize) {
        let Some(Register { text, linewise }) = self.register.clone() else { return };
        let buf = self.buf_mut();
        let here = buf.cursor;
        if linewise {
            let at = here.line + usize::from(after);
            let lines = (0..n).flat_map(|_| text.split('\n').map(str::to_owned)).collect();
            buf.splice(at..at, lines);
            buf.cursor = Pos::new(at, buf.first_non_blank(at));
        } else {
            let col = if after && buf.line_len(here.line) > 0 { here.col + 1 } else { here.col };
            let end = buf.insert(Pos::new(here.line, col), &text.repeat(n));
            buf.cursor = if text.contains('\n') { Pos::new(here.line, col) } else { Pos::new(end.line, end.col - 1) };
        }
    }

    fn undo(&mut self, n: usize, redo: bool) {
        self.repeat.skip = true;
        for _ in 0..n {
            let buf = self.buf_mut();
            if !(if redo { buf.redo() } else { buf.undo() }) {
                self.info(if redo { "Already at newest change" } else { "Already at oldest change" });
                break;
            }
        }
    }

    fn join(&mut self, n: usize) {
        let buf = self.buf_mut();
        for _ in 0..n.max(2) - 1 {
            let line = buf.cursor.line;
            if line >= buf.last_line() {
                break;
            }
            let left = buf.line(line).trim_end();
            let right = buf.line(line + 1).trim_start();
            let sep = if left.is_empty() || right.is_empty() || right.starts_with(')') { "" } else { " " };
            let col = text::char_len(left);
            let joined = format!("{left}{sep}{right}");
            buf.splice(line..line + 2, vec![joined]);
            buf.cursor = Pos::new(line, col);
        }
    }

    fn toggle_case(&mut self, n: usize) {
        let buf = self.buf_mut();
        let Pos { line, col } = buf.cursor;
        let end = (col + n).min(buf.line_len(line));
        if col >= end {
            return;
        }
        let mut toggled = String::with_capacity(buf.line(line).len());
        for (i, c) in buf.line(line).chars().enumerate() {
            match c {
                c if !(col..end).contains(&i) => toggled.push(c),
                c if c.is_uppercase() => toggled.extend(c.to_lowercase()),
                c => toggled.extend(c.to_uppercase()),
            }
        }
        buf.set_line(line, toggled);
        buf.cursor.col = end;
    }

    fn replace_chars(&mut self, ch: char, n: usize) {
        let buf = self.buf_mut();
        let Pos { line, col } = buf.cursor;
        if col + n > buf.line_len(line) {
            return;
        }
        let replaced = buf
            .line(line)
            .chars()
            .enumerate()
            .map(|(i, c)| if (col..col + n).contains(&i) { ch } else { c })
            .collect();
        buf.set_line(line, replaced);
        buf.cursor.col = col + n - 1;
    }
}
