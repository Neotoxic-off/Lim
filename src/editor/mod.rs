//! Modal editing: the state machine that turns keys into buffer edits.

mod bindings;
mod block;
mod cheatsheet;
mod clipboard;
mod cmdhistory;
mod command;
mod complete;
mod confirm;
mod explorer;
mod extras;
mod global;
mod insert;
mod keymap;
mod lines;
mod manage;
mod motion;
mod mouse;
mod normal;
mod operator;
mod picker;
mod prompt;
mod re;
mod recent;
mod registers;
mod repeat;
mod session;
mod setup;
mod sources;
#[cfg(test)]
mod tests;
mod textops;
mod ui;
mod usercmd;
mod vim;
mod visual;
mod wildmenu;
mod window;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::buffer::{Buffer, Pos, text};
use crate::config::{Config, Keymap, Options};
use crate::error::Error;
use crate::render::palettes;
use crate::term::Key;
pub use cheatsheet::{Card, Cheatsheet};
use cmdhistory::CmdHistory;
pub use complete::{Completion, MENU_HEIGHT, words_of};
pub use explorer::{Explorer, GitStatus, WIDTH as EXPLORER_WIDTH};
use motion::Motion;
pub use mouse::Pane;
use operator::Operator;
pub use picker::{Item, Picker, Target};
pub use prompt::Prompt;
pub use setup::{SETTINGS, Setup};
pub use ui::MenuEntry;
pub use usercmd::pipe;
pub use wildmenu::CmdCompletion;
pub use window::{Layout, Rect, Separator, View, Window, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    Search,
    /// Waiting for a tabline label (`<Leader>bb`, `<Leader>bd`, …).
    BufferPick(PickAction),
    /// The fuzzy finder has the keyboard.
    Picker,
    /// The file tree has the keyboard.
    Explorer,
    /// `R`: typed characters overwrite.
    Replace,
    /// `<C-v>`
    VisualBlock,
    /// A one-line input (rename, new file, indent width…).
    Prompt,
    /// The cheat sheet has the keyboard.
    Cheatsheet,
    /// The setup screen has the keyboard.
    Setup,
    /// `:s///c` asks about each match.
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickAction {
    Go,
    Close,
    SplitBelow,
    SplitRight,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal
            | Self::BufferPick(_)
            | Self::Picker
            | Self::Explorer
            | Self::Prompt
            | Self::Cheatsheet
            | Self::Setup => "NORMAL",
            Self::Insert => "INSERT",
            Self::Replace => "REPLACE",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
            Self::VisualBlock => "V-BLOCK",
            Self::Command | Self::Search | Self::Confirm => "COMMAND",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Info(String),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Chars(Pos, Pos),
    Lines(usize, usize),
    /// Lines `top..=bottom`, char columns `left..=right`.
    Block {
        top: usize,
        bottom: usize,
        left: usize,
        right: usize,
    },
}

impl Selection {
    pub fn contains(self, p: Pos) -> bool {
        match self {
            Self::Chars(start, end) => start <= p && p <= end,
            Self::Lines(first, last) => (first..=last).contains(&p.line),
            Self::Block { top, bottom, left, right } => {
                (top..=bottom).contains(&p.line) && (left..=right).contains(&p.col)
            }
        }
    }
}

/// Randomised tests type every key: they may only touch files under the temp dir.
pub(crate) fn may_touch(path: &Path) -> bool {
    !cfg!(test) || std::path::absolute(path).is_ok_and(|p| p.starts_with(std::env::temp_dir()))
}

/// …and may not start programs (git switch, browsers, terminals).
pub(crate) fn may_spawn() -> bool {
    !cfg!(test)
}

/// snacks.notifier's default timeout.
pub const NOTIFY_TIMEOUT: Duration = Duration::from_secs(3);

/// Keys of a normal command typed so far, e.g. the `3d` of `3dw`.
#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    count: Option<usize>,
    op: Option<Operator>,
    prefix: Option<char>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        self.count.is_none() && self.op.is_none() && self.prefix.is_none()
    }
}

#[expect(clippy::struct_excessive_bools, reason = "independent UI flags, not a state machine")]
pub struct Editor {
    pub buffers: Vec<Buffer>,
    pub current: usize,
    pub mode: Mode,
    pub cmdline: String,
    pub message: Option<Message>,
    /// When `message` was set: notifications (cmdheight=0) fade after `NOTIFY_TIMEOUT`.
    pub message_at: Instant,
    pub options: Options,
    /// The loaded config, kept so a colorscheme switch re-applies its palette and highlight overrides.
    config: Config,
    /// Name of the colorscheme in use.
    pub colorscheme: String,
    keymaps: Vec<Keymap>,
    /// Keys typed toward a user mapping, waiting for more or for `timeoutlen`.
    typeahead: Vec<Key>,
    typeahead_at: Instant,
    pub search: String,
    pub highlight_search: bool,
    /// Text rows on screen, kept up to date by the renderer for page motions.
    pub view_height: usize,
    /// Where each window's text was drawn in the last frame, for the mouse.
    pub panes: Vec<Pane>,
    /// The window and position a drag selects from; any key forgets it.
    mouse_anchor: Option<(usize, Pos)>,
    pub quit: bool,
    pub windows: Vec<Window>,
    pub layout: Layout,
    /// Index into `windows` of the focused window; its view lives in its buffer.
    pub active: usize,
    /// Screen area of the windows, kept up to date by the renderer for `<C-h/j/k/l>`.
    pub area: Rect,
    next_window_id: u32,
    previous_buffer: Option<usize>,
    pub picker: Option<Picker>,
    pub explorer: Option<Explorer>,
    /// The start screen is showing (no file given, or `<Leader>h`).
    pub dashboard: bool,
    pub started: Instant,
    /// Set by `<F7>` / `<Leader>t…`: the main loop suspends the editor and runs `$SHELL`.
    pub shell_requested: Option<Vec<String>>,
    last_picker: Option<Picker>,
    recent: Vec<PathBuf>,
    pub registers: registers::Registers,
    /// `"x` typed before the next yank, delete or put.
    register_name: Option<char>,
    /// OSC 52 clipboard sequence for the main loop to write when no clipboard tool exists.
    pub osc52: Option<String>,
    /// Last `f`/`F`/`t`/`T`, for `;` and `,`.
    last_find: Option<(char, bool, bool)>,
    /// `<C-o>` history: file and position before each jump.
    /// File, anchor id in that file's buffer, and the position when recorded (if the buffer is gone).
    jumps: Vec<(Option<PathBuf>, u64, Pos)>,
    next_anchor: u64,
    /// A `:g` is running; Vim refuses to nest them.
    in_global: bool,
    /// `:norm` is feeding keys: they count as replayed, not typed.
    in_norm: bool,
    /// For `gv`: the last visual selection and its mode.
    last_visual: Option<(Pos, Pos, Mode)>,
    /// `/` searches forward, `?` backward; `n` follows, `N` reverses.
    pub search_forward: bool,
    last_substitute: Option<(String, String, bool)>,
    pub prompt: Option<Prompt>,
    pub cheatsheet: Option<Cheatsheet>,
    pub setup: Option<Setup>,
    /// The `:s///c` session in progress.
    confirm: Option<confirm::Confirm>,
    /// The insert-mode completion menu, when open.
    pub completion: Option<Completion>,
    /// Typing changed the word before the cursor: the menu is recomputed when next needed (a frame, a menu
    /// key), so keys that arrive together (a paste, fast typing) cost one lookup, not one each.
    completion_stale: bool,
    /// The `:` completion menu, when open.
    pub cmd_completion: Option<CmdCompletion>,
    /// `:` lines and `/` `?` patterns typed before, for `<Up>` / `<Down>`.
    cmd_history: CmdHistory,
    search_history: CmdHistory,
    /// `<C-o>` in insert mode: one normal command, then back to insert.
    insert_once: bool,
    /// Line as it was when `R` started, so backspace can restore it.
    replace_original: String,
    /// Every notification shown, for `<Leader>fn`.
    pub history: Vec<Message>,
    /// `<Leader>uZ`: only the focused window, centred, without chrome.
    pub zen: bool,
    /// `<Leader>um`: the rendered Markdown beside Markdown buffers.
    pub markdown_preview: bool,
    block_insert: Option<block::BlockInsert>,
    /// `$` in visual block: every line's block reaches its own end.
    block_to_eol: bool,
    /// `<C-r>` in insert mode: the next key names the register to insert.
    insert_register: bool,
    anchor: Pos,
    pending: Pending,
    /// Line whose indent was inserted automatically; it is dropped again if left blank.
    auto_indent: Option<usize>,
    repeat: repeat::Repeat,
}

impl Editor {
    pub fn new(paths: &[String]) -> Self {
        let mut editor: Editor = Self {
            buffers: Vec::new(),
            current: 0,
            mode: Mode::Normal,
            cmdline: String::new(),
            message: None,
            message_at: Instant::now(),
            options: Options::default(),
            config: Config::default(),
            colorscheme: palettes::DEFAULT.into(),
            keymaps: Vec::new(),
            typeahead: Vec::new(),
            typeahead_at: Instant::now(),
            search: String::new(),
            highlight_search: false,
            view_height: 1,
            panes: Vec::new(),
            mouse_anchor: None,
            quit: false,
            windows: vec![Window::new(0, 0, View::default())],
            layout: Layout::Leaf(WindowId::first()),
            active: 0,
            area: Rect { x: 0, y: 1, w: 80, h: 20 },
            next_window_id: 1,
            previous_buffer: None,
            picker: None,
            explorer: None,
            dashboard: paths.is_empty(),
            started: Instant::now(),
            shell_requested: None,
            last_picker: None,
            recent: recent::load(),
            registers: registers::Registers::default(),
            register_name: None,
            osc52: None,
            last_find: None,
            jumps: Vec::new(),
            next_anchor: 0,
            in_global: false,
            in_norm: false,
            last_visual: None,
            search_forward: true,
            last_substitute: None,
            prompt: None,
            cheatsheet: None,
            confirm: None,
            setup: None,
            completion: None,
            completion_stale: false,
            cmd_completion: None,
            cmd_history: CmdHistory::default(),
            search_history: CmdHistory::default(),
            insert_once: false,
            replace_original: String::new(),
            history: Vec::new(),
            zen: false,
            markdown_preview: false,
            block_insert: None,
            block_to_eol: false,
            insert_register: false,
            anchor: Pos::default(),
            pending: Pending::default(),
            auto_indent: None,
            repeat: repeat::Repeat::default(),
        };
        for path in paths {
            editor.open(Path::new(path));
        }
        if editor.buffers.is_empty() {
            editor.buffers.push(Buffer::scratch());
        }
        editor.current = 0;
        editor.previous_buffer = None;
        editor
    }

    pub fn buf(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    pub fn buf_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    pub fn selection(&self) -> Option<Selection> {
        let (a, b): (Pos, Pos) = (self.anchor, self.buf().cursor);
        let (start, end): (Pos, Pos) = if a <= b { (a, b) } else { (b, a) };
        match self.mode {
            Mode::Visual => Some(Selection::Chars(start, end)),
            Mode::VisualLine => Some(Selection::Lines(start.line, end.line)),
            Mode::VisualBlock => Some(Selection::Block {
                top: start.line,
                bottom: end.line,
                left: a.col.min(b.col),
                right: if self.block_to_eol { usize::MAX - 1 } else { a.col.max(b.col) },
            }),
            _ => None,
        }
    }

    /// Applies the config: options, keymaps and the text settings they drive.
    pub fn configure(&mut self, config: &Config) {
        self.options.clone_from(&config.options);
        self.keymaps.clone_from(&config.keymaps);
        self.config.clone_from(config);
        self.dashboard &= self.options.dashboard;
        self.apply_options();
        if self.options.gitsigns {
            for buf in &mut self.buffers {
                buf.load_git_base();
            }
        }
    }

    pub(super) fn apply_options(&mut self) {
        let o: &Options = &self.options;
        text::configure(o.tabstop, o.ignorecase, o.smartcase);
    }

    /// One key as typed: goes through user keymaps first, then the modes.
    pub fn handle(&mut self, key: Key) {
        if let Key::Mouse(event) = key {
            return self.mouse(event);
        }
        self.mouse_anchor = None;
        let key: Key = self.translate_leader(key);
        if self.map_key(key) {
            return;
        }
        self.dispatch(key);
    }

    /// One key, after mappings.
    fn dispatch(&mut self, key: Key) {
        self.track_before(key);
        let mode: Mode = self.mode;
        let was_visual: bool = matches!(mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock);
        let insert_once: bool = self.insert_once && mode == Mode::Normal;
        let motion_key: bool = matches!(mode, Mode::Normal | Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
            && self.pending.prefix.is_none();
        match mode {
            Mode::Normal => self.normal(key),
            Mode::Insert => self.insert(key),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => self.visual(key),
            Mode::Command | Mode::Search => self.command_line(key),
            Mode::BufferPick(action) => self.pick(key, action),
            Mode::Picker => self.picker_key(key),
            Mode::Explorer => self.explorer_key(key),
            Mode::Replace => self.replace_key(key),
            Mode::Prompt => self.prompt_key(key),
            Mode::Cheatsheet => self.cheatsheet_key(key),
            Mode::Setup => self.setup_key(key),
            Mode::Confirm => self.confirm_key(key),
        }
        if let Some(window) = self.windows.get_mut(self.active) {
            window.buffer = self.current;
        }
        let motion: Option<Motion> = motion_key.then(|| Motion::from_key(key)).flatten();
        let past_end: bool = matches!(self.mode, Mode::Insert | Mode::Replace);
        let buf: &mut Buffer = self.buf_mut();
        buf.clamp(past_end);
        // j/k keep aiming at the column they started from; anything else resets it.
        let vertical: bool =
            matches!(key, Key::Up | Key::Down | Key::PageUp | Key::PageDown) || motion.is_some_and(Motion::is_vertical);
        if !vertical {
            buf.want_col = if motion == Some(Motion::LineEnd) { usize::MAX } else { buf.cursor.col };
        }
        // Leaving visual mode: remember the selection for `gv` and the `'<` / `'>` marks.
        if was_visual && !matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
            let (anchor, cursor): (Pos, Pos) = (self.anchor, self.buf().cursor);
            self.last_visual = Some((anchor, cursor, mode));
            let buf: &mut Buffer = self.buf_mut();
            buf.marks.insert('<', anchor.min(cursor));
            buf.marks.insert('>', anchor.max(cursor));
        }
        if self.mode != Mode::Insert {
            self.completion = None;
            self.completion_stale = false;
        }
        if self.mode != Mode::Command {
            self.cmd_completion = None;
        }
        // `<C-o>` from insert mode: back to insert once the normal command is complete.
        if insert_once && self.mode == Mode::Normal && self.pending.is_empty() {
            self.insert_once = false;
            self.mode = Mode::Insert;
        }
        self.track_after();
    }

    /// Brings the editor back to a usable state after an internal error and backs up unsaved buffers.
    pub fn recover(&mut self, detail: &str) {
        self.mode = Mode::Normal;
        self.pending = Pending::default();
        self.cmdline.clear();
        self.auto_indent = None;
        self.reset_repeat();
        if self.buffers.is_empty() {
            self.buffers.push(Buffer::scratch());
        }
        self.current = self.current.min(self.buffers.len() - 1);
        self.repair_windows();
        let mut backups: Vec<String> = Vec::new();
        for (i, buf) in self.buffers.iter_mut().enumerate() {
            buf.repair();
            if buf.is_dirty() {
                let name: String =
                    buf.name().chars().map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' }).collect();
                let path: PathBuf = std::env::temp_dir().join(format!("lim-recover-{}-{i}-{name}", std::process::id()));
                if buf.write_copy(&path).is_ok() {
                    backups.push(path.display().to_string());
                }
            }
        }
        let saved: String = if backups.is_empty() {
            String::new()
        } else {
            format!("; unsaved buffers copied to {}", backups.join(", "))
        };
        self.typeahead.clear();
        self.set_message(Message::Error(format!("internal error recovered ({detail}){saved}")));
    }

    pub fn message_text(&self) -> &str {
        match &self.message {
            Some(Message::Info(m) | Message::Error(m)) => m,
            None => "",
        }
    }

    pub fn set_message(&mut self, message: Message) {
        self.history.push(message.clone());
        if self.history.len() > 200 {
            self.history.remove(0);
        }
        self.message = Some(message);
        self.message_at = Instant::now();
    }

    /// When the screen must be redrawn without a key: a notification fading or a mapping timing out.
    pub fn deadline(&self) -> Option<Instant> {
        let notify: Option<Instant> =
            (self.message.is_some() && self.options.cmdheight == 0).then(|| self.message_at + NOTIFY_TIMEOUT);
        let mapping: Option<Instant> =
            (!self.typeahead.is_empty()).then(|| self.typeahead_at + Duration::from_millis(self.options.timeoutlen));
        notify.into_iter().chain(mapping).min()
    }

    /// Called when `deadline` passes.
    pub fn tick(&mut self) {
        if self.notification_expired() && self.options.cmdheight == 0 {
            self.message = None;
        }
        if !self.typeahead.is_empty() && self.typeahead_at.elapsed() >= Duration::from_millis(self.options.timeoutlen) {
            self.flush_typeahead(true);
        }
    }

    fn notification_expired(&self) -> bool {
        self.message_at.elapsed() >= NOTIFY_TIMEOUT
    }

    fn info(&mut self, message: impl Into<String>) {
        self.set_message(Message::Info(message.into()));
    }

    fn error(&mut self, error: &Error) {
        self.set_message(Message::Error(error.to_string()));
    }
}
