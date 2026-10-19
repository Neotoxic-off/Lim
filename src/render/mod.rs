//! Screen composition: tabline, text, statusline, command line. Only changed rows are rewritten.

mod bars;
mod finder;
#[cfg(test)]
mod gallery;
mod home;
#[cfg(test)]
pub use home::SPINNER;
pub mod icons;
pub mod markdown;
pub mod palettes;
mod popup;
mod row;
mod theme;
mod tree;

pub use theme::Theme;
use theme::install;
mod view;

use std::fmt::{self, Write};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

use crate::buffer::{Buffer, text};
use crate::config::Options;
use crate::editor::{EXPLORER_WIDTH, Editor, Mode, Rect, Separator, WindowId};
use crate::term::{ColorDepth, Size};
use row::Grid;
use theme::{Group, Rgb, Style, theme};

fn colorscheme_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(crate::config::state_dir()?.join("colorscheme"))
}

/// The colorscheme to start with: the config's, else the last one picked, else nvchad.
pub fn initial_colorscheme(config: &crate::config::Config) -> String {
    config
        .options
        .colorscheme
        .clone()
        .or_else(|| colorscheme_file().and_then(|f| fs::read_to_string(f).ok()).map(|s| s.trim().to_owned()))
        .filter(|name| palettes::THEMES.iter().any(|t| t.name == name))
        .unwrap_or_else(|| palettes::DEFAULT.into())
}

/// Builds and installs a colorscheme; returns config warnings about it.
pub fn apply_colorscheme(name: &str, config: &crate::config::Config) -> Vec<String> {
    let (theme, warnings): (Theme, Vec<String>) = Theme::build(config, name);
    install(theme);
    warnings
}

pub fn save_colorscheme(name: &str) {
    if let Some(file) = colorscheme_file() {
        if let Some(dir) = file.parent() {
            let _: io::Result<()> = fs::create_dir_all(dir);
        }
        let _: io::Result<()> = fs::write(file, name);
    }
}

/// Workspace facts shown in the statusline.
pub struct Context {
    pub branch: Option<String>,
    pub cwd: String,
}

impl Context {
    fn detect() -> Self {
        let dir: Option<PathBuf> = env::current_dir().ok();
        let cwd: String =
            dir.as_deref().and_then(|d| d.file_name()).map_or_else(|| "/".into(), |n| n.to_string_lossy().into());
        Self { branch: dir.and_then(git_branch), cwd }
    }
}

fn git_branch(mut dir: PathBuf) -> Option<String> {
    loop {
        if let Ok(head) = fs::read_to_string(dir.join(".git/HEAD")) {
            let head: &str = head.trim();
            return Some(
                head.strip_prefix("ref: refs/heads/").map_or_else(|| head.chars().take(7).collect(), str::to_owned),
            );
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// A tool's popup, border included: room for a few questions and answers, not a window's worth.
const POPUP_WIDTH: usize = 56;
const POPUP_HEIGHT: usize = 10;

pub struct Screen {
    /// The terminal's own depth, used while `truecolor` is "auto".
    detected: ColorDepth,
    depth: ColorDepth,
    ctx: Context,
    size: Size,
    rows: Vec<String>,
    /// Whether the terminal reports the mouse; None until the first frame tells it.
    mouse: Option<bool>,
}

impl Screen {
    pub fn new(detected: ColorDepth) -> Self {
        Self { detected, depth: detected, ctx: Context::detect(), size: Size::default(), rows: Vec::new(), mouse: None }
    }

    /// Forces the next frame to repaint everything.
    pub fn invalidate(&mut self) {
        self.size = Size::default();
        self.rows.clear();
        self.mouse = None;
    }

    /// Escape sequences taking the terminal from the previous frame to this one.
    pub fn draw(&mut self, ed: &mut Editor, size: Size) -> String {
        let mut out: String = String::from("\x1b[?25l");
        if size != self.size {
            self.size = size;
            self.rows.clear();
            out.push_str("\x1b[0m\x1b[2J");
        }
        let (width, height): (usize, usize) = (size.cols.max(1), size.rows.max(1));
        let options: Options = ed.options.clone();
        if self.mouse != Some(options.mouse) {
            self.mouse = Some(options.mouse);
            out.push_str(if options.mouse { "[?1002h[?1006h" } else { "[?1002l[?1006l" });
        }
        // `truecolor` can change in `:settings`; every row is re-encoded in the new depth.
        let depth: ColorDepth = match options.truecolor {
            Some(true) => ColorDepth::TrueColor,
            Some(false) => ColorDepth::Ansi256,
            None => self.detected,
        };
        if depth != self.depth {
            self.depth = depth;
            self.rows.clear();
        }
        let (grid, cursor): (Grid, Option<(usize, usize)>) = self.compose(ed, width, height);
        let rows: Vec<String> = (0..height).map(|y| row::encode(grid.row(y), self.depth)).collect();
        for (i, line) in rows.iter().enumerate() {
            if self.rows.get(i) != Some(line) {
                let _: fmt::Result = write!(out, "\x1b[{};1H{line}", i + 1);
            }
        }
        self.rows = rows;
        let Some((row, col)): Option<(usize, usize)> = cursor else { return out };
        let shape: i32 = match ed.mode {
            Mode::Insert | Mode::Command | Mode::Search | Mode::Picker | Mode::Prompt => 6,
            Mode::Replace => 4,
            _ => 2,
        };
        let _: fmt::Result =
            write!(out, "\x1b[{};{}H\x1b[{shape} q\x1b[?25h", row.min(height - 1) + 1, col.min(width - 1) + 1);
        out
    }

    /// The frame as cells, and the cursor's screen cell if it shows. Drawn back to front: the tabline, the side
    /// panels, the windows, the bars at the bottom, then the floats over everything.
    fn compose(&mut self, ed: &mut Editor, width: usize, height: usize) -> (Grid, Option<(usize, usize)>) {
        ed.refresh_completion();
        ed.panes.clear();
        ed.tab_hits.clear();
        let options: Options = ed.options.clone();
        let mut grid: Grid = Grid::new(width, height, theme().style(Group::Normal));
        let rows: Rows = Rows::of(&options, ed.mode, height);
        // Columns taken by the file tree and its border, shared by the tabline and the windows.
        let tree_width: usize = if ed.explorer.is_some() && width >= 20 { EXPLORER_WIDTH.min(width / 2) } else { 0 };
        if rows.tabline {
            ed.tab_hits = bars::tabline(ed, &mut grid, 0, if tree_width > 0 { tree_width + 1 } else { 0 });
        }
        let screen: Rect = Rect { x: 0, y: rows.top(), w: width, h: rows.text };
        let (area, tree_cursor): (Rect, Option<(usize, usize)>) = side_panels(ed, &mut grid, screen, tree_width);
        let mut cursor: Option<(usize, usize)> = text_area(ed, &mut grid, area, &options, tree_cursor);
        popup::completion(ed, &mut grid, cursor);
        popup::which_key(ed, &mut grid, area);
        self.bottom_bars(ed, &mut grid, &rows, &options);
        if rows.prompt {
            cursor = Some((height - 1, 1 + text::str_width(&ed.cmdline)));
        }
        let cursor: Option<(usize, usize)> = overlays(ed, &mut grid, cursor);
        (grid, cursor)
    }

    /// The statusline, then the command line (or the last message, or a notification).
    fn bottom_bars(&mut self, ed: &mut Editor, grid: &mut Grid, rows: &Rows, options: &Options) {
        if rows.status {
            // The branch can change under us (a shell, another terminal): re-read HEAD, a tiny file.
            self.ctx.branch = env::current_dir().ok().and_then(git_branch);
            // The counts come with the signs, cached per buffer version; the gutter may not have asked for them.
            if options.gitsigns {
                ed.buf_mut().git_signs();
            }
            bars::statusline(ed, &self.ctx, grid, rows.top() + rows.text);
        }
        if rows.cmd || rows.prompt {
            bars::cmdline(ed, grid, rows.height - 1);
        } else if options.notifications {
            bars::notification(ed, grid, rows.top());
        }
    }

    /// Every window and split border inside `area`, then the popups over them; returns the focused cursor's cell.
    fn windows(ed: &mut Editor, area: Rect, grid: &mut Grid) -> Option<(usize, usize)> {
        ed.area = area;
        let mut rects: Vec<(WindowId, Rect)> = Vec::new();
        ed.layout.rects(area, &mut rects);
        let mut cursor: Option<(usize, usize)> = None;
        for (id, rect) in rects {
            let Some(index): Option<usize> = ed.windows.iter().position(|w| w.id == id) else { continue };
            if index == ed.active {
                ed.view_height = rect.h.saturating_sub(usize::from(ed.options.winbar)).max(1);
            }
            cursor = cursor.or(view::window(ed, index, rect, grid));
        }
        separators(ed, area, grid);
        let popups: Vec<usize> = (0..ed.windows.len()).filter(|&i| ed.windows[i].floating).collect();
        for index in popups {
            cursor = popup_window(ed, index, area, grid).or(cursor);
        }
        cursor
    }
}

/// How the screen's rows are shared: tabline, text, statusline and command line.
#[expect(clippy::struct_excessive_bools, reason = "one per bar the screen may show")]
struct Rows {
    height: usize,
    tabline: bool,
    status: bool,
    /// A command line row: `cmdheight` asks for it, or a `:` / `/` prompt borrows it (as Neovim does with
    /// `cmdheight=0`), pushing the statusline up.
    cmd: bool,
    prompt: bool,
    /// The rows left for the windows.
    text: usize,
}

impl Rows {
    fn of(options: &Options, mode: Mode, height: usize) -> Self {
        // Below four rows only the text shows.
        let roomy: bool = height > 3;
        let prompt: bool = matches!(mode, Mode::Command | Mode::Search);
        let tabline: bool = options.showtabline && roomy;
        let status: bool = options.laststatus && roomy;
        let cmd: bool = (options.cmdheight > 0 || prompt) && roomy;
        let bars: usize = usize::from(tabline) + usize::from(status) + usize::from(cmd);
        Self { height, tabline, status, cmd, prompt, text: height.saturating_sub(bars).max(1) }
    }

    /// The first text row.
    fn top(&self) -> usize {
        usize::from(self.tabline)
    }
}

/// The file tree on the left and the Markdown preview on the right, around the windows; returns the area left for
/// the windows, and the cursor in the tree when it has focus.
fn side_panels(ed: &mut Editor, grid: &mut Grid, mut area: Rect, tree_width: usize) -> (Rect, Option<(usize, usize)>) {
    let mut cursor: Option<(usize, usize)> = None;
    if tree_width > 0 {
        cursor = tree::explorer(ed, grid, Rect { w: tree_width, ..area });
        // NvChad hides this separator: the tree's color alone sets it apart.
        let tree_bg: Rgb = theme().bg(Group::ExplorerNormal);
        grid.vline(tree_width, area.y..area.y + area.h, Style::new(tree_bg, tree_bg));
        area = Rect { x: tree_width + 1, w: area.w - tree_width - 1, ..area };
    }
    // The Markdown preview takes the right half, past a separator, while a Markdown buffer is shown.
    if ed.markdown_preview && !ed.zen && area.w >= 40 && markdown::is_markdown(ed.buf()) {
        let panel_width: usize = area.w / 2;
        area.w -= panel_width + 1;
        let separator: usize = area.x + area.w;
        grid.vline(separator, area.y..area.y + area.h, theme().style(Group::WinSeparator));
        markdown::preview(ed, grid, Rect { x: separator + 1, w: panel_width, ..area });
    }
    (area, cursor)
}

/// The windows (or the zen window, or the start screen) in `area`; returns where the cursor shows, the tree's
/// while the tree has focus.
fn text_area(
    ed: &mut Editor,
    grid: &mut Grid,
    area: Rect,
    options: &Options,
    tree_cursor: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let windows_cursor: Option<(usize, usize)> = if ed.zen {
        // snacks.zen: the focused window alone, centred, at most 120 columns wide.
        let w: usize = 120.min(area.w * 3 / 4).max(area.w.min(20));
        let zen: Rect = Rect { x: area.x + (area.w - w) / 2, w, ..area };
        ed.area = zen;
        ed.view_height = zen.h;
        view::window(ed, ed.active, zen, grid)
    } else {
        Screen::windows(ed, area, grid)
    };
    if ed.dashboard {
        grid.fill(area, theme().style(Group::Normal));
        home::dashboard(grid, area, options);
        tree_cursor
    } else if ed.mode == Mode::Explorer {
        tree_cursor
    } else {
        windows_cursor
    }
}

/// The floats over everything: pickers, the command-line menu, the cheat sheet, the settings and prompts.
fn overlays(ed: &mut Editor, grid: &mut Grid, mut cursor: Option<(usize, usize)>) -> Option<(usize, usize)> {
    if let Some(input) = finder::picker(ed, grid) {
        cursor = Some(input);
    }
    popup::cmd_completion(ed, grid);
    popup::cheatsheet(ed, grid);
    popup::setup(ed, grid);
    if matches!(ed.mode, Mode::Cheatsheet | Mode::Setup) {
        cursor = None;
    }
    popup::prompt(ed, grid).or(cursor)
}

/// The borders between split windows.
fn separators(ed: &Editor, area: Rect, grid: &mut Grid) {
    let mut separators: Vec<Separator> = Vec::new();
    ed.layout.separators(area, &mut separators);
    let border: Style = theme().style(Group::WinSeparator);
    for sep in separators {
        if sep.vertical {
            grid.vline(sep.x, sep.y..sep.y + sep.len, border);
        } else {
            grid.text(sep.x, sep.y, sep.len, border, &"\u{2500}".repeat(sep.len));
        }
    }
}

/// A tool's popup, small and centred over the windows: drawn as the notifications are, the editor's background in
/// a colored rounded border, the tool's name on top and what to type at the bottom.
fn popup_window(ed: &mut Editor, index: usize, area: Rect, grid: &mut Grid) -> Option<(usize, usize)> {
    let (w, h): (usize, usize) = (POPUP_WIDTH.min(area.w.saturating_sub(4)), POPUP_HEIGHT.min(area.h));
    if w < 10 || h < 3 {
        return None;
    }
    let frame: Rect = Rect { x: area.x + (area.w - w) / 2, y: area.y + (area.h - h) / 2, w, h };
    let buffer: usize = ed.windows[index].buffer;
    let title: String = ed.buffers.get(buffer).map(Buffer::name).unwrap_or_default();
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::Normal);
    grid.float(frame, Style::new(t.bg(Group::FloatTitle), fill.bg), fill, title.trim_matches(['[', ']']));
    if let Some(hint) = ed.popup_hint(buffer) {
        let hint: String = format!(" {hint} ");
        let room: usize = text::str_width(&hint).min(w - 4);
        grid.text(frame.x + 2, frame.y + h - 1, room, t.on(Group::LineNr, fill.bg), &hint);
    }
    // A column of air on each side, inside the border.
    let inside: Rect = Rect { x: frame.x + 2, y: frame.y + 1, w: w - 4, h: h - 2 };
    if index == ed.active {
        ed.view_height = inside.h;
    }
    view::window(ed, index, inside, grid)
}

#[cfg(test)]
mod tests {
    use std::str::Chars;

    use super::*;
    use crate::buffer::Buffer;
    use crate::term::Key;

    /// Every mode and odd sizes render without panicking.
    #[test]
    fn draws_any_size_and_mode() {
        let mut ed: Editor = Editor::new(&[]);
        for k in "ifn main() {\n日本 é\t\"x\"".chars() {
            ed.handle(if k == '\n' { Key::Enter } else { Key::Char(k) });
        }
        ed.handle(Key::Esc);
        let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
        for keys in ["", "|", "\\", " bb", "\x1b:w", "\x1bv", "\x1b/x"] {
            for k in keys.chars() {
                ed.handle(if k == '\x1b' { Key::Esc } else { Key::Char(k) });
            }
            for (cols, rows) in [(80, 24), (1, 1), (3, 2), (10, 4), (200, 3)] {
                let frame: String = screen.draw(&mut ed, Size { cols, rows });
                assert!(frame.starts_with("\x1b[?25l"));
            }
        }
    }

    /// The frame's text with escape sequences removed.
    fn visible(frame: &str) -> String {
        let mut out: String = String::new();
        let mut chars: Chars<'_> = frame.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(char::is_ascii_alphabetic);
            } else {
                out.push(c);
            }
        }
        out
    }

    /// Floats draw their content at full size and give up cleanly when the screen is too small.
    #[test]
    fn draws_floats() {
        let mut ed: Editor = Editor::new(&[]);
        ed.dashboard = false;
        ed.buffers.push(Buffer::from_text(Some("other.rs".into()), "x"));
        let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
        let mut press = |ed: &mut Editor, keys: &[Key], expect: &str| {
            ed.handle(Key::Esc);
            for &key in keys {
                ed.handle(key);
            }
            let frame: String = visible(&screen.draw(ed, Size { cols: 100, rows: 30 }));
            assert!(frame.contains(expect), "{keys:?} should show {expect:?}");
            for (cols, rows) in [(11, 5), (23, 7), (40, 9)] {
                screen.draw(ed, Size { cols, rows });
            }
        };
        press(&mut ed, &[Key::Char(' '), Key::Char('f'), Key::Char('b')], "other.rs");
        press(&mut ed, &[Key::Char(' '), Key::Char('f'), Key::Char('b'), Key::Char('o'), Key::Char('t')], "other.rs");
        press(&mut ed, &[Key::F(1)], "Operators");
        press(&mut ed, &[Key::F(1)], "/ Search keys and commands");
        press(&mut ed, &[Key::F(1), Key::Char('x'), Key::Char('o'), Key::Char('r')], " Reverse engineering ");
        press(&mut ed, &[Key::F(1), Key::Char('x'), Key::Char('o'), Key::Char('r')], ":xor 0x41 :xor key");
        press(&mut ed, &[Key::Char(' '), Key::Char('R')], "other.rs");
        press(&mut ed, &[Key::Char(' ')], "+Find");
        let setup: Vec<Key> = ":settings".chars().map(Key::Char).chain([Key::Enter]).collect();
        press(&mut ed, &setup, "statusline_style");
        // `<Leader>um` beside a Markdown buffer shows it rendered: no `**`, the bullet drawn.
        // Paragraphs first, so the bullet sits below the "enabled" notification in the top right corner.
        let notes: &str = "# Notes\none\n\ntwo\n\nthree\n\n- **bold** point";
        ed.buffers.push(Buffer::from_text(Some("notes.md".into()), notes));
        ed.handle(Key::Esc);
        ed.current = ed.buffers.len() - 1;
        press(&mut ed, &[Key::Char(' '), Key::Char('u'), Key::Char('m')], "• bold point");
    }
}
