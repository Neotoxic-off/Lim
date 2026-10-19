//! NvChad-style tabufline and statusline, command line and notifications.

use super::Context;
use super::home::{SPINNER, SPINNER_FRAME_MS};
use super::icons::{Icons, icons};
use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, mode_group, theme};
use crate::buffer::git::Counts;
use crate::buffer::{Buffer, Pos, text};
use crate::config::StatusStyle;
use crate::editor::{Editor, Message, Mode, Rect, TabClick, TabHit};
use crate::render::Theme;
use std::sync::Arc;

type Segment = (Style, String);

/// `left` truncated to fit, then `right` flush against the right edge.
fn bar(grid: &mut Grid, y: usize, left: &[Segment], right: &[Segment], fill: Style) {
    bar_at(grid, 0, y, left, right, fill);
}

/// `bar` starting at column `x`, filling the rest of the row.
fn bar_at(grid: &mut Grid, x: usize, y: usize, left: &[Segment], right: &[Segment], fill: Style) {
    let width: usize = grid.width.saturating_sub(x);
    let right_width: usize = right.iter().map(|(_, s)| text::str_width(s)).sum();
    let (right, avail): (&[(Style, String)], usize) =
        if right_width > width { (&[][..], width) } else { (right, width - right_width) };
    let mut row: Row = Row::new(width);
    'left: for (style, s) in left {
        for c in s.chars() {
            if row.used() + text::char_width(c) > avail {
                break 'left;
            }
            row.cell(*style, c);
        }
    }
    row.pad_to(fill, avail);
    for (style, s) in right {
        row.text(*style, s);
    }
    grid.put(x, y, &row.finish(fill));
}

/// `NvChad`'s tabufline: each buffer centred in `BUF_WIDTH` columns (its `bufwidth`).
const BUF_WIDTH: usize = 21;

/// Buffer tabs from column `offset`: the part over the file tree takes the tree's color, as `NvChad`'s tree offset.
/// Returns where each tab, close mark and button landed, for the mouse.
pub fn tabline(ed: &Editor, grid: &mut Grid, y: usize, offset: usize) -> Vec<TabHit> {
    let t: Arc<Theme> = theme();
    let set: &Icons = icons(&ed.options);
    let mut tabs: Tabs = Tabs {
        segments: Vec::with_capacity(ed.buffers.len() * 5),
        hits: Vec::with_capacity(ed.buffers.len() * 2 + 2),
        x: offset,
    };
    // While picking (`<Leader>bb`), heirline swaps each icon for a red jump label.
    let labels: Option<Vec<char>> = matches!(ed.mode, Mode::BufferPick(_)).then(|| ed.pick_labels());
    for (i, buf) in ed.buffers.iter().enumerate() {
        let label: Option<char> = labels.as_ref().and_then(|l| l.get(i)).copied();
        tabs.push(&t, set, buf, (i, i == ed.current), label);
    }
    let Tabs { segments, mut hits, .. }: Tabs = tabs;
    let buttons: [Segment; 2] = buttons(&t, set);
    let right_width: usize = buttons.iter().map(|(_, s)| text::str_width(s)).sum();
    let right: &[Segment] = if offset + right_width <= grid.width { &buttons } else { &[] };
    // Tabs cut off by the buttons cannot be clicked.
    let avail: usize = grid.width - if right.is_empty() { 0 } else { right_width };
    hits.retain(|h| h.to <= avail);
    let mut bx: usize = avail;
    for ((_, s), click) in right.iter().zip([TabClick::ToggleTheme, TabClick::CloseAll]) {
        let w: usize = text::str_width(s);
        hits.push(TabHit { from: bx, to: bx + w, click });
        bx += w;
    }
    if offset > 0 {
        grid.text(0, y, offset, t.style(Group::ExplorerNormal), "");
    }
    bar_at(grid, offset, y, &segments, right, t.style(Group::TabLineFill));
    hits
}

/// The tabs drawn so far, where a click lands on them, and the column the next one starts at.
struct Tabs {
    segments: Vec<Segment>,
    hits: Vec<TabHit>,
    x: usize,
}

impl Tabs {
    /// Buffer `i`'s tab, centred in `BUF_WIDTH`: its icon (or jump `label`), name and close or modified mark.
    fn push(&mut self, t: &Theme, set: &Icons, buf: &Buffer, (i, active): (usize, bool), label: Option<char>) {
        let name: Style = t.style(if active { Group::TabLineSel } else { Group::TabLine });
        let (icon_style, icon): Segment = tab_icon(t, set, buf, label, name);
        let (mark_style, mark): Segment = tab_mark(t, set, buf.is_dirty(), active, name);
        let title: String = buf.name();
        let used: usize = text::str_width(&icon) + text::str_width(&title) + text::str_width(&mark);
        let pad: usize = BUF_WIDTH.saturating_sub(used).max(2);
        let end: usize = self.x + pad + used;
        let mark_x: usize = end - text::str_width(&mark);
        self.hits.extend([
            TabHit { from: self.x, to: mark_x, click: TabClick::Buffer(i) },
            TabHit { from: mark_x, to: end, click: TabClick::Close(i) },
        ]);
        self.segments.extend([
            (name, " ".repeat(pad / 2)),
            (icon_style, icon),
            (name, title),
            (name, " ".repeat(pad - pad / 2)),
            (mark_style, mark),
        ]);
        self.x = end;
    }
}

/// A tab's file icon in its color, or the red jump label while picking a buffer.
fn tab_icon(t: &Theme, set: &Icons, buf: &Buffer, label: Option<char>, name: Style) -> Segment {
    let (icon, icon_fg): (String, Option<u32>) = set.for_file(buf.lang);
    match label {
        Some(label) => (t.on(Group::BufferPicker, name.bg), format!("{label} ")),
        None if !icon.is_empty() => (Style { fg: icon_fg.map_or(name.fg, Rgb), ..name }, format!("{icon} ")),
        None => (name, String::new()),
    }
}

/// A tab's right end: the modified mark (green on the current buffer, red on the others, as `NvChad`'s
/// `TbBufOnModified` / `TbBufOffModified`), else the close button (red on the current buffer).
fn tab_mark(t: &Theme, set: &Icons, dirty: bool, active: bool, name: Style) -> Segment {
    match (dirty, active) {
        (true, true) => (t.on(Group::TabLineModified, name.bg), format!("{} ", set.modified)),
        (true, false) => (t.on(Group::ErrorMsg, name.bg), format!("{} ", set.modified)),
        (false, true) => (t.on(Group::ErrorMsg, name.bg), format!("{} ", set.close)),
        (false, false) => (name, format!("{} ", set.close)),
    }
}

/// `NvChad`'s buttons on the right of the tabline: light/dark theme, close all buffers.
fn buttons(t: &Theme, set: &Icons) -> [Segment; 2] {
    [
        (t.style(Group::TabLineButton), format!(" {} ", set.theme)),
        (t.style(Group::TabLineCloseAll), format!(" {} ", set.close)),
    ]
}

/// The statusline's colors, shared by its blocks.
struct StatusColors {
    fill: Style,
    section: Style,
    /// The grey step between a colored block and the next one.
    step: Rgb,
}

/// `NvChad`'s default statusline: mode, file and git on the left; background work, cwd and position on the right.
pub fn statusline(ed: &Editor, ctx: &Context, grid: &mut Grid, y: usize) {
    let t: Arc<Theme> = theme();
    let set: &Icons = icons(&ed.options);
    let colors: StatusColors = StatusColors {
        fill: t.style(Group::StatusLine),
        section: t.style(Group::StatusSection),
        step: t.bg(Group::StatusModeSep),
    };
    let (left_cap, right_cap): (&str, &str) = caps(ed.options.statusline_style, set);
    let mut left: Vec<Segment> = mode_and_file(ed, &t, set, &colors, right_cap);
    left.extend(git(ed, ctx, &t, set));
    let mut right: Vec<Segment> = activity(ed, &t, set);
    right.extend(cwd_and_position(ed.buf().cursor, ctx, &t, set, &colors, left_cap));
    bar(grid, y, &left, &right, colors.fill);
}

/// The ends of the statusline's blocks. Powerline shapes need a Nerd Font: sets without caps stay flat.
fn caps(style: StatusStyle, set: &Icons) -> (&'static str, &'static str) {
    match style {
        _ if set.left_cap.is_empty() => ("", ""),
        StatusStyle::Rounded => (set.left_cap, set.right_cap),
        StatusStyle::Slanted => ("\u{e0ba}", "\u{e0bc}"),
        StatusStyle::Arrow => ("\u{e0b2}", "\u{e0b0}"),
        StatusStyle::Flat => ("", ""),
    }
}

/// The mode block, stepping down through the grey cap to the file block.
fn mode_and_file(ed: &Editor, t: &Theme, set: &Icons, c: &StatusColors, cap: &str) -> Vec<Segment> {
    let buf: &Buffer = ed.buf();
    let mode: Style = t.style(mode_group(ed.mode));
    let mut segments: Vec<Segment> = vec![
        (mode, format!(" {}{} ", Icons::spaced(set.mode), ed.mode.label())),
        (Style::new(mode.bg, c.step), cap.into()),
        (Style::new(c.step, c.section.bg), cap.into()),
    ];
    let (icon, icon_fg): (String, Option<u32>) = set.for_file(buf.lang);
    if !icon.is_empty() {
        segments.push((Style { fg: icon_fg.map_or(c.section.fg, Rgb), ..c.section }, format!(" {icon}")));
    }
    segments.push((c.section, format!(" {}{} ", buf.name(), if buf.is_dirty() { " [+]" } else { "" })));
    segments.push((Style::new(c.section.bg, c.fill.bg), cap.into()));
    segments
}

/// The branch, then gitsigns' added, changed and deleted counts.
fn git(ed: &Editor, ctx: &Context, t: &Theme, set: &Icons) -> Vec<Segment> {
    let Some(branch): Option<&String> = ctx.branch.as_ref() else { return Vec::new() };
    let style: Style = t.style(Group::StatusGit);
    let mut segments: Vec<Segment> = vec![(style, format!(" {}{branch}", Icons::spaced(set.branch)))];
    let Counts { added, changed, removed }: Counts =
        if ed.options.gitsigns { ed.buf().git_counts() } else { Counts::default() };
    for (count, icon) in [(added, set.git_added), (changed, set.git_modified), (removed, set.git_deleted)] {
        if count > 0 {
            segments.push((style, format!("  {}{count}", Icons::spaced(icon))));
        }
    }
    segments
}

/// Background commands (the hydra spinner, where `NvChad` shows LSP progress) and macro recording.
fn activity(ed: &Editor, t: &Theme, set: &Icons) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    if let Some((names, elapsed)) = ed.running_jobs() {
        let tick: usize = usize::try_from(elapsed.as_millis() / SPINNER_FRAME_MS).unwrap_or(0);
        segments.push((t.style(Group::StatusJob), format!("{} {names}  ", SPINNER[tick % SPINNER.len()])));
    }
    if let Some(reg) = ed.recording() {
        segments.push((t.style(Group::StatusRecording), format!("{}@{reg}  ", Icons::spaced(set.recording))));
    }
    segments
}

/// The cwd and position blocks, each rising from the grey step as the mode block does, mirrored.
fn cwd_and_position(at: Pos, ctx: &Context, t: &Theme, set: &Icons, c: &StatusColors, cap: &str) -> [Segment; 8] {
    let cwd: Style = t.style(Group::StatusCwd);
    let position: Style = t.style(Group::StatusPosition);
    [
        (Style::new(c.step, c.fill.bg), cap.into()),
        (Style::new(cwd.bg, c.step), cap.into()),
        (cwd, format!("{} ", set.cwd)),
        (c.section, format!(" {} ", ctx.cwd)),
        (Style::new(c.step, c.section.bg), cap.into()),
        (Style::new(position.bg, c.step), cap.into()),
        (position, format!("{} ", set.position)),
        // NvChad's `%l/%v`, in the position's color.
        (Style { fg: position.bg, ..c.section }, format!(" {}/{} ", at.line + 1, at.col + 1)),
    ]
}

/// The `:` / `/` prompt, or (with `cmdheight=1`) the last message.
pub fn cmdline(ed: &Editor, grid: &mut Grid, y: usize) {
    let t: Arc<Theme> = theme();
    let normal: Style = t.style(Group::MsgArea);
    let (style, text): (Style, String) = match (ed.mode, &ed.message) {
        (Mode::Command, _) => (normal, format!(":{}", ed.cmdline)),
        (Mode::Search, _) => (normal, format!("{}{}", if ed.search_forward { '/' } else { '?' }, ed.cmdline)),
        (_, Some(Message::Error(m))) => (t.style(Group::ErrorMsg), m.clone()),
        (_, Some(Message::Info(m))) => (normal, m.clone()),
        (_, None) => (normal, String::new()),
    };
    bar(grid, y, &[(style, text)], &[], normal);
}

/// snacks.notifier-style toast in the top-right corner (`cmdheight=0`).
pub fn notification(ed: &Editor, grid: &mut Grid, top: usize) {
    let t: Arc<Theme> = theme();
    let (text, group, icon): (&String, Group, &str) = match &ed.message {
        Some(Message::Error(m)) if !m.is_empty() => (m, Group::NotifyError, icons(&ed.options).error),
        Some(Message::Info(m)) if !m.is_empty() => (m, Group::NotifyInfo, icons(&ed.options).info),
        _ => return,
    };
    let max: usize = (grid.width / 2).max(20).min(grid.width.saturating_sub(2));
    let body: String = format!("{}{text}", Icons::spaced(icon));
    let width: usize = (text::str_width(&body) + 4).min(max);
    if width < 6 || grid.height < top + 3 {
        return;
    }
    let x: usize = grid.width - width - 1;
    // The background of what the toast sits on (an unfocused window is darker), not the float's: a block of
    // another shade there clashes.
    let under: Rgb = grid.row(top + 1).get(x).map_or_else(|| t.bg(Group::Normal), |c| c.style.bg);
    let fill: Style = Style { bg: under, ..t.style(Group::Normal) };
    let border: Style = t.on(group, fill.bg);
    grid.float(Rect { x, y: top, w: width, h: 3 }, border, fill, "");
    grid.text(x + 2, top + 1, width - 4, Style { fg: border.fg, ..fill }, &body);
}
