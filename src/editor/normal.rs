//! Normal mode: counts, operators, two-key prefixes and single-key commands.

use super::bindings::{self, Action};
use super::command::Command;
use super::motion::{Motion, Span};
use super::operator::Operator;
use super::picker::PickerKind;
use super::prompt::PromptAction;
use super::registers::Register;
use super::session;
use super::ui::{CTRL_W, LEADER_F};
use super::window::Direction;
use super::{Editor, Mode, Pending};
use crate::buffer::{Buffer, CharClass, Pos, text};
use crate::config::keys::LEADER;
use crate::error::Error;
use crate::term::Key;

/// Keys that wait for one more: `gg`, `f{c}`, `r{c}`, `ZZ`, `]b`, and the leader.
const PREFIXES: &str = "gfFtTrZq@[]\"m'`z\u{F8FF}";

/// Prefixes visual mode takes: motions, text objects (`iw`), `gc` and the leader.
pub(super) const VISUAL_PREFIXES: &str = "gfFtTiar\"'`\u{F8FF}";

impl Editor {
    #[expect(clippy::too_many_lines, reason = "flat key dispatch: one arm per key")]
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
            self.shell_requested = Some(Vec::new());
            return;
        }
        if key == Key::F(1) {
            return self.open_cheatsheet();
        }
        // snacks.dashboard: its own keys; everything that edits text is ignored while it shows.
        if self.dashboard && self.pending.is_empty() {
            let picker: Option<PickerKind> = match key {
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
        if self.repeat_find_key(key) {
            return;
        }
        // `i`/`a` after an operator start a text object (`diw`, `ca(`), otherwise they insert.
        let objects: &str = if self.pending.op.is_some() { "ia" } else { "" };
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
        // AstroNvim's `gco` / `gcO`: a new comment line below / above.
        if self.pending.op == Some(Operator::Comment)
            && let Key::Char(c @ ('o' | 'O')) = key
        {
            self.pending = Pending::default();
            return self.add_comment_line(c == 'o');
        }
        if let Some(motion) = Motion::from_key(key) {
            return self.run_motion(motion);
        }
        let count: Option<usize> = self.pending.count.take();
        let n: usize = count.unwrap_or(1);
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
        let here: Pos = self.buf().cursor;
        let len: usize = self.buf().line_len(here.line);
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
            Key::Char('J') => self.join(n, true),
            Key::Char('~') => self.toggle_case(n),
            Key::Char('.') => self.repeat_change(n),
            Key::Char('*') => self.search_word(true),
            Key::Char('#') => self.search_word(false),
            Key::Char('v') => self.start_visual(Mode::Visual),
            Key::Char('V') => self.start_visual(Mode::VisualLine),
            Key::Char(':') => self.start_cmdline(Mode::Command),
            Key::Char('/') => {
                self.search_forward = true;
                self.start_cmdline(Mode::Search);
            }
            Key::Char('?') => {
                self.search_forward = false;
                self.start_cmdline(Mode::Search);
            }
            Key::Char('R') => self.start_replace(),
            Key::Char('&') => self.repeat_substitute(),
            Key::Ctrl('a') => self.increment(i64::try_from(n).unwrap_or(i64::MAX)),
            Key::Ctrl('x') => self.increment(i64::try_from(n).map_or(i64::MIN, |n| -n)),
            Key::Ctrl('o') => self.jump_back(n),
            Key::Ctrl('e') => self.scroll_view(true, n),
            Key::Ctrl('y') => self.scroll_view(false, n),
            Key::Ctrl('v') => self.start_visual(Mode::VisualBlock),
            Key::CtrlUp => self.resize_window(false, 2),
            Key::CtrlDown => self.resize_window(false, -2),
            Key::CtrlLeft => self.resize_window(true, -2),
            Key::CtrlRight => self.resize_window(true, 2),
            Key::Esc => self.register_name = None,
            Key::Char('|') => self.split(true),
            Key::Char('\\') => self.split(false),
            Key::Ctrl('h') => self.focus_direction(Direction::Left),
            Key::Ctrl('j') => self.focus_direction(Direction::Down),
            Key::Ctrl('k') => self.focus_direction(Direction::Up),
            Key::Ctrl('l') => self.focus_direction(Direction::Right),
            Key::Ctrl('w') => self.pending.prefix = Some(CTRL_W),
            Key::Tab => self.cycle(true, n),
            Key::BackTab => self.cycle(false, n),
            Key::Ctrl('s') => self.run(Command::Write { path: None, force: false }),
            Key::Ctrl('q') => self.run(Command::Quit { force: true }),
            _ => {}
        }
    }

    pub(super) fn push_count(&mut self, key: Key) -> bool {
        let Key::Char(c @ '0'..='9'): Key = key else { return false };
        if c == '0' && self.pending.count.is_none() {
            return false;
        }
        let digit: usize = c as usize - '0' as usize;
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
        let c: char = match key {
            Key::Char(c) => c,
            Key::Ctrl(c) if prefix == CTRL_W => c,
            // Vim: `<C-w><Left>` is `<C-w>h`, and so on.
            Key::Left if prefix == CTRL_W => 'h',
            Key::Down if prefix == CTRL_W => 'j',
            Key::Up if prefix == CTRL_W => 'k',
            Key::Right if prefix == CTRL_W => 'l',
            Key::Enter if prefix == LEADER_F => '\r',
            _ => {
                self.pending = Pending::default();
                return;
            }
        };
        let n: usize = self.pending.count.unwrap_or(1);
        // Keys that take an argument rather than naming a command.
        match (prefix, c) {
            ('f' | 'F' | 't' | 'T', ch) => {
                let till: bool = matches!(prefix, 't' | 'T');
                self.last_find = Some((ch, prefix.is_ascii_lowercase(), till));
                return self.run_motion(Motion::Find { ch, forward: prefix.is_ascii_lowercase(), till });
            }
            ('"', name) => {
                self.register_name = Some(name);
                return;
            }
            ('m', name) => self.set_mark(name),
            ('\'' | '`', name) => {
                let name: char = if matches!(name, '\'' | '`') { '\'' } else { name };
                return self.run_motion(Motion::Mark { name, exact: prefix == '`' });
            }
            ('r', ch) => self.replace_chars(ch, n),
            ('i' | 'a', obj) => self.select_object(prefix == 'a', obj),
            ('q', reg) => self.start_recording(reg),
            ('@', reg) => self.play_macro(reg, n),
            _ => {
                if let Some(action) = bindings::lookup(prefix, c).filter(|&a| bindings::allowed_in(self.mode, a))
                    && self.run_action(action, n)
                {
                    return;
                }
            }
        }
        self.pending = Pending::default();
    }

    /// Runs a binding from the table; true when it left a prefix or operator waiting.
    fn run_action(&mut self, action: Action, n: usize) -> bool {
        let visual: bool = matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock);
        match action {
            Action::Group(sub) => {
                self.pending.prefix = Some(sub);
                return true;
            }
            Action::Motion(motion) => {
                self.run_motion(motion);
                return true;
            }
            // In visual mode an operator applies to the selection at once.
            Action::Operator(op) if visual => {
                let span: Span = if self.mode == Mode::Visual { Span::Inclusive } else { Span::Linewise };
                let cursor: Pos = self.buf().cursor;
                self.mode = Mode::Normal;
                self.buf_mut().seal();
                self.apply(op, self.anchor, cursor, span);
            }
            Action::Operator(op) => {
                self.pending.op = Some(op);
                return true;
            }
            Action::CommentLines if visual => return self.run_action(Action::Operator(Operator::Comment), n),
            Action::CommentLines => self.apply_count_lines(Operator::Comment, n),
            Action::Save => self.run(Command::Write { path: None, force: false }),
            Action::Quit => self.run(Command::Quit { force: false }),
            Action::QuitAll => self.run(Command::QuitAll { force: false }),
            Action::WriteQuit => self.run(Command::WriteQuit { path: None, force: false, only_if_dirty: true }),
            Action::ForceQuit => self.run(Command::Quit { force: true }),
            Action::NewFile => self.run(Command::New),
            Action::CloseBuffer { force } => self.run(Command::BufferDelete { force }),
            Action::RenameFile => {
                let current: String = self.buf().path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                self.open_prompt("New Filename", &current, PromptAction::RenameFile);
            }
            Action::ToggleExplorer => self.toggle_explorer(),
            Action::FocusExplorer => self.focus_explorer(),
            Action::Home => self.dashboard = !self.dashboard,
            Action::Cheatsheet => self.open_cheatsheet(),
            Action::Settings => self.open_setup(),
            Action::Picker(kind) => self.open_picker(kind),
            Action::ResumePicker => self.resume_picker(),
            Action::GrepWord => self.grep_word(),
            Action::BufferPick(pick) => self.mode = Mode::BufferPick(pick),
            Action::CloseBuffers(set) => self.close_buffers(set),
            Action::PreviousBuffer => self.go_previous_buffer(),
            Action::SortBuffers(by) => self.sort_buffers(by),
            Action::CycleBuffer { forward } => self.cycle(forward, n),
            Action::Shell => self.shell_requested = Some(Vec::new()),
            Action::Tool(argv) => self.run_tool(argv),
            Action::GitBrowse => self.git_browse(),
            Action::Hunk { forward } => self.jump_hunk(forward),
            Action::HunkEdge { last } => self.jump_hunk_edge(last),
            Action::Reference { forward } => self.jump_reference(forward, n),
            Action::Todo { forward } => self.jump_todo(forward),
            Action::Toggle(toggle) => self.toggle(toggle),
            Action::SessionLoadLast => self.load_session(session::LAST, false),
            Action::SessionSave => self.open_prompt("Session name", "", PromptAction::SessionSave),
            Action::SessionSaveDir => self.save_dirsession(),
            Action::SessionLoadDir => self.load_dirsession(),
            Action::Split { vertical } => self.split(vertical),
            Action::CloseWindow => self.run(Command::CloseWindow),
            Action::OnlyWindow => self.only_window(),
            Action::NextWindow => self.focus_next_window(),
            Action::Focus(direction) => self.focus_direction(direction),
            Action::ScrollTo(align) => self.scroll_cursor_to(align),
            Action::Reselect => self.reselect(),
            Action::InsertLast => {
                let at: Option<Pos> = self.buf().marks.get(&'^').copied();
                if let Some(at) = at {
                    self.buf_mut().cursor = at;
                }
                self.mode = Mode::Insert;
            }
            Action::InsertColumnZero => self.start_insert(0),
            Action::JoinRaw => self.join(n, false),
            Action::OpenUrl => self.open_url_under_cursor(),
        }
        false
    }

    /// `;` and `,`: the last `f`/`t` again, same or opposite direction.
    pub(super) fn repeat_find_key(&mut self, key: Key) -> bool {
        let (Key::Char(c @ (';' | ',')), Some((ch, forward, till))): (Key, Option<(char, bool, bool)>) =
            (key, self.last_find)
        else {
            return false;
        };
        self.run_motion(Motion::Find { ch, forward: forward == (c == ';'), till });
        true
    }

    /// Moves the cursor, or applies the pending operator up to the motion target.
    pub(super) fn run_motion(&mut self, motion: Motion) {
        let Pending { count, op, .. }: Pending = std::mem::take(&mut self.pending);
        // `?` searches backward: `n` keeps that direction, `N` reverses it.
        let motion: Motion = match motion {
            Motion::SearchNext { forward } => Motion::SearchNext { forward: forward == self.search_forward },
            m => m,
        };
        let buf: &Buffer = self.buf();
        let from: Pos = buf.cursor;
        let Some(mut target): Option<Pos> = motion.target(buf, count, self.view_height, &self.search) else {
            if let Motion::SearchNext { .. } = motion {
                let error: Error =
                    if self.search.is_empty() { Error::NoPattern } else { Error::PatternNotFound(self.search.clone()) };
                self.error(&error);
            }
            return;
        };
        let mut span: Span = motion.span();
        // Vim quirks: `cw` on a word acts like `ce`; `dw` on a line's last word keeps the line break.
        if let Some(op) = op
            && motion == Motion::WordForward
        {
            if op == Operator::Change && buf.class_at(from) != CharClass::Blank {
                target = (0..count.unwrap_or(1))
                    .fold(from, |p, i| if i == 0 && buf.at_word_end(p) { p } else { buf.word_end(p, false) });
                span = Span::Inclusive;
            } else if target.line > from.line {
                target = Pos::new(from.line, buf.line_len(from.line));
            }
        }
        if let Motion::SearchNext { .. } = motion {
            self.highlight_search = true;
        }
        // Recorded after the target is known, so `''` still reads the previous jump.
        if op.is_none() && motion.is_jump() {
            self.push_jump();
        }
        match op {
            Some(op) => self.apply(op, from, target, span),
            None => self.buf_mut().cursor = target,
        }
    }

    /// Text object under the cursor: operated on when an operator waits, else selected (visual mode).
    fn select_object(&mut self, around: bool, obj: char) {
        let buf: &Buffer = self.buf();
        let at: Pos = buf.cursor;
        let range: Option<(Pos, Pos)> = match obj {
            'w' => buf.word_object(at, around),
            '(' | ')' | 'b' => buf.pair_object(at, '(', ')', around),
            '[' | ']' => buf.pair_object(at, '[', ']', around),
            '{' | '}' | 'B' => buf.pair_object(at, '{', '}', around),
            '<' | '>' => buf.pair_object(at, '<', '>', around),
            '"' | '\'' | '`' => buf.quote_object(at, obj, around),
            _ => None,
        };
        let Some((start, end)): Option<(Pos, Pos)> = range else { return };
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
        let buf: &Buffer = self.buf();
        let Some((start, end)): Option<(Pos, Pos)> = buf.word_object(buf.cursor, false) else { return };
        if buf.class_at(start) != CharClass::Word {
            return;
        }
        self.search = format!("\\<{}\\>", buf.text(start, end));
        self.buf_mut().cursor = start;
        self.run_motion(Motion::SearchNext { forward });
    }

    fn jump_hunk(&mut self, forward: bool) {
        let buf: &mut Buffer = self.buf_mut();
        match buf.next_hunk(buf.cursor.line, forward) {
            Some(line) => buf.cursor = Pos::new(line, buf.first_non_blank(line)),
            None => self.info("No hunks"),
        }
    }

    fn apply_count_lines(&mut self, op: Operator, n: usize) {
        let buf: &Buffer = self.buf();
        let first: usize = buf.cursor.line;
        let last: usize = (first + n - 1).min(buf.last_line());
        self.apply(op, Pos::new(first, 0), Pos::new(last, 0), Span::Linewise);
    }

    pub(super) fn start_insert(&mut self, col: usize) {
        self.buf_mut().cursor.col = col;
        self.mode = Mode::Insert;
    }

    pub(super) fn start_visual(&mut self, mode: Mode) {
        self.anchor = self.buf().cursor;
        self.mode = mode;
    }

    pub(super) fn start_cmdline(&mut self, mode: Mode) {
        self.cmdline.clear();
        self.mode = mode;
        self.cmdline_history().reset();
    }

    pub(super) fn open_line(&mut self, below: bool) {
        let buf: &mut Buffer = self.buf_mut();
        let line: usize = buf.cursor.line;
        let indent: String = text::indent(buf.line(line)).to_owned();
        let col: usize = text::char_len(&indent);
        let at: usize = line + usize::from(below);
        buf.splice(at..at, vec![indent]);
        buf.cursor = Pos::new(at, col);
        self.auto_indent = (col > 0).then_some(at);
        self.mode = Mode::Insert;
    }

    fn paste(&mut self, after: bool, n: usize) {
        let Some(reg): Option<Register> = self.fetch_register() else { return };
        if reg.text.is_empty() && !reg.linewise {
            return;
        }
        if reg.block {
            return self.put_block(&reg, after);
        }
        let Register { text, linewise, .. }: Register = reg;
        let buf: &mut Buffer = self.buf_mut();
        let here: Pos = buf.cursor;
        if linewise {
            let at: usize = here.line + usize::from(after);
            let lines: Vec<String> = (0..n).flat_map(|_| text.split('\n').map(str::to_owned)).collect();
            buf.splice(at..at, lines);
            buf.cursor = Pos::new(at, buf.first_non_blank(at));
        } else {
            let col: usize = if after && buf.line_len(here.line) > 0 { here.col + 1 } else { here.col };
            let end: Pos = buf.insert(Pos::new(here.line, col), &text.repeat(n));
            buf.cursor = if text.contains('\n') { Pos::new(here.line, col) } else { Pos::new(end.line, end.col - 1) };
        }
    }

    fn undo(&mut self, n: usize, redo: bool) {
        self.repeat.skip = true;
        for _ in 0..n {
            let buf: &mut Buffer = self.buf_mut();
            if !(if redo { buf.redo() } else { buf.undo() }) {
                self.info(if redo { "Already at newest change" } else { "Already at oldest change" });
                break;
            }
        }
    }

    /// `J` (one space between, trimmed) or `gJ` (`spaces = false`: lines glued as they are).
    pub(super) fn join(&mut self, n: usize, spaces: bool) {
        let buf: &mut Buffer = self.buf_mut();
        for _ in 0..n.max(2) - 1 {
            let line: usize = buf.cursor.line;
            if line >= buf.last_line() {
                break;
            }
            let (left, right): (&str, &str) = if spaces {
                (buf.line(line).trim_end(), buf.line(line + 1).trim_start())
            } else {
                (buf.line(line), buf.line(line + 1))
            };
            let sep: &str =
                if !spaces || left.is_empty() || right.is_empty() || right.starts_with(')') { "" } else { " " };
            let col: usize = text::char_len(left);
            let joined: String = format!("{left}{sep}{right}");
            buf.splice(line..line + 2, vec![joined]);
            buf.cursor = Pos::new(line, col);
        }
    }

    fn toggle_case(&mut self, n: usize) {
        let buf: &mut Buffer = self.buf_mut();
        let Pos { line, col }: Pos = buf.cursor;
        let end: usize = (col + n).min(buf.line_len(line));
        if col >= end {
            return;
        }
        let mut toggled: String = String::with_capacity(buf.line(line).len());
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
        let buf: &mut Buffer = self.buf_mut();
        let Pos { line, col }: Pos = buf.cursor;
        if col + n > buf.line_len(line) {
            return;
        }
        let replaced: String =
            buf.line(line).chars().enumerate().map(|(i, c)| if (col..col + n).contains(&i) { ch } else { c }).collect();
        buf.set_line(line, replaced);
        buf.cursor.col = col + n - 1;
    }
}
