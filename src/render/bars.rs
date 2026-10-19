//! NvChad-style tabufline and statusline, command line and notifications.

use super::Context;
use super::icons::{Icons, icons};
use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, mode_group, theme};
use crate::buffer::{Buffer, Pos, text};
use crate::config::StatusStyle;
use crate::editor::{Editor, Message, Mode, Rect};
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
pub fn tabline(ed: &Editor, grid: &mut Grid, y: usize, offset: usize) {
    let t: Arc<Theme> = theme();
    let set: &Icons = icons(&ed.options);
    let mut segments: Vec<(Style, String)> = Vec::with_capacity(ed.buffers.len() * 4);
    // While picking (`<Leader>bb`), heirline swaps each icon for a red jump label.
    let labels: Option<Vec<char>> = matches!(ed.mode, Mode::BufferPick(_)).then(|| ed.pick_labels());
    for (i, buf) in ed.buffers.iter().enumerate() {
        let active: bool = i == ed.current;
        let name: Style = t.style(if active { Group::TabLineSel } else { Group::TabLine });
        let (icon, icon_fg): (String, Option<u32>) = set.for_file(buf.lang);
        let (icon, icon_style): (String, Style) = match labels.as_ref().and_then(|l| l.get(i)) {
            Some(label) => (format!("{label} "), t.on(Group::BufferPicker, name.bg)),
            None if !icon.is_empty() => (format!("{icon} "), Style { fg: icon_fg.map_or(name.fg, Rgb), ..name }),
            None => (String::new(), name),
        };
        let (mark, mark_style): (String, Style) = if buf.is_dirty() {
            // Green on the current buffer, red on the others (TbBufOnModified / TbBufOffModified).
            let group: Group = if active { Group::TabLineModified } else { Group::ErrorMsg };
            (format!("{} ", set.modified), t.on(group, name.bg))
        } else if active {
            // NvChad's red close button on the current buffer.
            (format!("{} ", set.close), t.on(Group::ErrorMsg, name.bg))
        } else {
            (format!("{} ", set.close), name)
        };
        let label: String = buf.name();
        let used: usize = text::str_width(&icon) + text::str_width(&label) + text::str_width(&mark);
        let pad: usize = BUF_WIDTH.saturating_sub(used).max(2);
        segments.extend([
            (name, " ".repeat(pad / 2)),
            (icon_style, icon),
            (name, label),
            (name, " ".repeat(pad - pad / 2)),
            (mark_style, mark),
        ]);
    }
    if offset > 0 {
        grid.text(0, y, offset, t.style(Group::ExplorerNormal), "");
    }
    bar_at(grid, offset, y, &segments, &[], t.style(Group::TabLineFill));
}

pub fn statusline(ed: &Editor, ctx: &Context, grid: &mut Grid, y: usize) {
    let t: Arc<Theme> = theme();
    let set: &Icons = icons(&ed.options);
    let buf: &Buffer = ed.buf();
    let fill: Style = t.style(Group::StatusLine);
    let section: Style = t.style(Group::StatusSection);
    let mode: Style = t.style(mode_group(ed.mode));
    let (icon, icon_fg): (String, Option<u32>) = set.for_file(buf.lang);
    // Powerline shapes need a Nerd Font: sets without caps stay flat whatever the style.
    let (left_cap, right_cap): (&str, &str) = match ed.options.statusline_style {
        _ if set.left_cap.is_empty() => ("", ""),
        StatusStyle::Rounded => (set.left_cap, set.right_cap),
        StatusStyle::Slanted => ("\u{e0ba}", "\u{e0bc}"),
        StatusStyle::Arrow => ("\u{e0b2}", "\u{e0b0}"),
        StatusStyle::Flat => ("", ""),
    };
    // NvChad steps from the mode down to the file through a grey cap.
    let step: Rgb = t.bg(Group::StatusModeSep);
    let mut left: Vec<(Style, String)> = vec![
        (mode, format!(" {}{} ", Icons::spaced(set.mode), ed.mode.label())),
        (Style::new(mode.bg, step), right_cap.into()),
        (Style::new(step, section.bg), right_cap.into()),
    ];
    if !icon.is_empty() {
        left.push((Style { fg: icon_fg.map_or(section.fg, Rgb), ..section }, format!(" {icon}")));
    }
    left.push((section, format!(" {}{} ", buf.name(), if buf.is_dirty() { " [+]" } else { "" })));
    left.push((Style::new(section.bg, fill.bg), right_cap.into()));
    if let Some(branch) = &ctx.branch {
        left.push((t.style(Group::StatusGit), format!(" {}{branch}", Icons::spaced(set.branch))));
    }
    let mut right: Vec<(Style, String)> = Vec::new();
    if let Some(reg) = ed.recording() {
        right.push((t.style(Group::StatusRecording), format!("{}@{reg}  ", Icons::spaced(set.recording))));
    }
    let Pos { line, col }: Pos = buf.cursor;
    let cwd: Style = t.style(Group::StatusCwd);
    let position: Style = t.style(Group::StatusPosition);
    right.extend([
        (Style::new(cwd.bg, fill.bg), left_cap.into()),
        (cwd, format!("{} ", if set.cwd.is_empty() { "" } else { set.cwd })),
        (section, format!(" {} ", ctx.cwd)),
        (Style::new(position.bg, section.bg), left_cap.into()),
        (position, format!("{} ", if set.position.is_empty() { "" } else { set.position })),
        // NvChad's `%l/%v`, in the position's color.
        (Style { fg: position.bg, ..section }, format!(" {}/{} ", line + 1, col + 1)),
    ]);
    bar(grid, y, &left, &right, fill);
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
    // The editor's background, not the float's: a toast sits on the text, and a darker block there clashes.
    let fill: Style = t.style(Group::Normal);
    let border: Style = t.on(group, fill.bg);
    let max: usize = (grid.width / 2).max(20).min(grid.width.saturating_sub(2));
    let body: String = format!("{}{text}", Icons::spaced(icon));
    let width: usize = (text::str_width(&body) + 4).min(max);
    if width < 6 || grid.height < top + 3 {
        return;
    }
    let x: usize = grid.width - width - 1;
    grid.float(Rect { x, y: top, w: width, h: 3 }, border, fill, "");
    grid.text(x + 2, top + 1, width - 4, Style { fg: border.fg, ..fill }, &body);
}
