//! Tabline (heirline bufferline), NvChad-style statusline, command line and notifications.

use super::Context;
use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::buffer::{Buffer, Pos, text};
use crate::editor::{Editor, Message, Mode, Rect};

type Segment = (Style, String);

fn file_icon(buf: &Buffer) -> (char, Option<Rgb>) {
    buf.lang.map_or(('\u{f15b}', None), |l| (l.icon, Some(Rgb(l.icon_color))))
}

/// `left` truncated to fit, then `right` flush against the right edge.
fn bar(grid: &mut Grid, y: usize, left: &[Segment], right: &[Segment], fill: Style) {
    let width = grid.width;
    let right_width: usize = right.iter().map(|(_, s)| text::str_width(s)).sum();
    let (right, avail) = if right_width > width { (&[][..], width) } else { (right, width - right_width) };
    let mut row = Row::new(width);
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
    grid.put(0, y, &row.finish(fill));
}

pub fn tabline(ed: &Editor, grid: &mut Grid, y: usize) {
    let t = theme();
    let icons = ed.options.icons;
    let mut segments = Vec::with_capacity(ed.buffers.len() * 4);
    // While picking (`<Leader>bb`), heirline swaps each icon for a red jump label.
    let labels = matches!(ed.mode, Mode::BufferPick(_)).then(|| ed.pick_labels());
    for (i, buf) in ed.buffers.iter().enumerate() {
        let active = i == ed.current;
        let name = t.style(if active { Group::TabLineSel } else { Group::TabLine });
        let plain = Style { bold: false, italic: false, ..name };
        let indicator = if active { t.on(Group::TabLineIndicator, name.bg) } else { Style { fg: name.bg, ..name } };
        segments.push((indicator, "▎".into()));
        let (icon, icon_fg) = file_icon(buf);
        match labels.as_ref().and_then(|l| l.get(i)) {
            Some(label) => segments.push((t.on(Group::BufferPicker, name.bg), format!(" {label} "))),
            None if icons => {
                let fg = if active { icon_fg.unwrap_or(plain.fg) } else { plain.fg };
                segments.push((Style { fg, ..plain }, format!(" {icon} ")));
            }
            None => segments.push((plain, " ".into())),
        }
        segments.push((name, buf.name()));
        let (mark, style) = match (buf.is_dirty(), icons) {
            (true, true) => (" \u{f111} ", t.on(Group::TabLineModified, name.bg)),
            (true, false) => (" [+] ", t.on(Group::TabLineModified, name.bg)),
            (false, true) => (" \u{f00d} ", plain),
            (false, false) => ("  ", plain),
        };
        segments.push((style, mark.into()));
    }
    bar(grid, y, &segments, &[], t.style(Group::TabLineFill));
}

pub fn statusline(ed: &Editor, ctx: &Context, grid: &mut Grid, y: usize) {
    let t = theme();
    let icons = ed.options.icons;
    let glyph = |g: &'static str| if icons { g } else { "" };
    let buf = ed.buf();
    let fill = t.style(Group::StatusLine);
    let section = t.style(Group::StatusSection);
    let mode = t.style(t.mode(ed.mode));
    let (icon, icon_fg) = file_icon(buf);
    let mut left = vec![
        (mode, format!(" {}{} ", if icons { "\u{e7c5} " } else { "" }, ed.mode.label())),
        (Style::new(mode.bg, section.bg), glyph("\u{e0b4}").into()),
    ];
    if icons {
        left.push((Style { fg: icon_fg.unwrap_or(section.fg), ..section }, format!(" {icon}")));
    }
    left.push((section, format!(" {}{} ", buf.name(), if buf.is_dirty() { " [+]" } else { "" })));
    left.push((Style::new(section.bg, fill.bg), glyph("\u{e0b4}").into()));
    if let Some(branch) = &ctx.branch {
        left.push((fill, format!(" {}{branch}", glyph("\u{e725} "))));
    }
    let mut right = Vec::new();
    if let Some(reg) = ed.recording() {
        right.push((t.style(Group::StatusRecording), format!("{}@{reg}  ", glyph("\u{f111} "))));
    }
    let Pos { line, col } = buf.cursor;
    let progress = match line {
        0 => "Top".into(),
        l if l == buf.last_line() => "Bot".into(),
        l => format!("{}%", (l + 1) * 100 / buf.line_count()),
    };
    let cwd = t.style(Group::StatusCwd);
    let position = t.style(Group::StatusPosition);
    right.extend([
        (Style::new(cwd.bg, fill.bg), glyph("\u{e0b6}").into()),
        (cwd, if icons { "\u{f07b} ".into() } else { " ".into() }),
        (section, format!(" {} ", ctx.cwd)),
        (Style::new(position.bg, section.bg), glyph("\u{e0b6}").into()),
        (position, if icons { "\u{e612} ".into() } else { " ".into() }),
        (section, format!(" {}:{} {progress} ", line + 1, col + 1)),
    ]);
    bar(grid, y, &left, &right, fill);
}

/// The `:` / `/` prompt, or (with `cmdheight=1`) the last message.
pub fn cmdline(ed: &Editor, grid: &mut Grid, y: usize) {
    let t = theme();
    let normal = t.style(Group::MsgArea);
    let (style, text) = match (ed.mode, &ed.message) {
        (Mode::Command, _) => (normal, format!(":{}", ed.cmdline)),
        (Mode::Search, _) => (normal, format!("/{}", ed.cmdline)),
        (_, Some(Message::Error(m))) => (t.style(Group::ErrorMsg), m.clone()),
        (_, Some(Message::Info(m))) => (normal, m.clone()),
        (_, None) => (normal, String::new()),
    };
    bar(grid, y, &[(style, text)], &[], normal);
}

/// snacks.notifier-style toast in the top-right corner (`cmdheight=0`).
pub fn notification(ed: &Editor, grid: &mut Grid, top: usize) {
    let t = theme();
    let (text, group, icon) = match &ed.message {
        Some(Message::Error(m)) if !m.is_empty() => (m, Group::NotifyError, '\u{f057}'),
        Some(Message::Info(m)) if !m.is_empty() => (m, Group::NotifyInfo, '\u{f05a}'),
        _ => return,
    };
    let fill = t.style(Group::NormalFloat);
    let border = t.on(group, fill.bg);
    let max = (grid.width / 2).max(20).min(grid.width.saturating_sub(2));
    let body = if ed.options.icons { format!("{icon} {text}") } else { text.clone() };
    let width = (text::str_width(&body) + 4).min(max);
    if width < 6 || grid.height < top + 3 {
        return;
    }
    let x = grid.width - width - 1;
    grid.float(Rect { x, y: top, w: width, h: 3 }, border, fill, "");
    grid.text(x + 2, top + 1, width - 4, Style { fg: border.fg, ..fill }, &body);
}
