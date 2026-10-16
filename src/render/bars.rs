//! Tabline (heirline bufferline), NvChad-style statusline and the command line.

use super::Context;
use super::row::Row;
use super::theme::*;
use crate::buffer::{Buffer, Pos, text};
use crate::editor::{Editor, Message, Mode};
use crate::term::ColorDepth;

type Segment = (Style, String);

const DEFAULT_ICON: char = '\u{f15b}';
const MODIFIED: &str = " \u{f111} ";
const CLOSE: &str = " \u{f00d} ";

fn file_icon(buf: &Buffer) -> (char, Rgb) {
    buf.lang.map_or((DEFAULT_ICON, UI_TEXT), |l| (l.icon, Rgb(l.icon_color)))
}

/// `left` truncated to fit, then `right` flush against the right edge.
fn bar(depth: ColorDepth, width: usize, left: &[Segment], right: &[Segment], fill: Style) -> String {
    let right_width: usize = right.iter().map(|(_, s)| text::str_width(s)).sum();
    let (right, avail) = if right_width > width { (&[][..], width) } else { (right, width - right_width) };
    let mut row = Row::new(depth, width);
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
    row.finish(fill)
}

pub fn tabline(ed: &Editor, width: usize, depth: ColorDepth) -> String {
    let mut segments = Vec::with_capacity(ed.buffers.len() * 4);
    // While picking (`<Leader>bb`), heirline swaps each icon for a red jump label.
    let labels = matches!(ed.mode, Mode::BufferPick(_)).then(|| ed.pick_labels());
    for (i, buf) in ed.buffers.iter().enumerate() {
        let active = i == ed.current;
        let (bg, fg) = if active { (BASE, TEXT) } else { (TABLINE, TEXT_INACTIVE) };
        let (icon, icon_fg) = file_icon(buf);
        let name = Style::new(fg, bg);
        segments.push((Style::new(if active { UI_BLUE } else { TABLINE }, bg), "▎".into()));
        match labels.as_ref().and_then(|l| l.get(i)) {
            Some(label) => segments.push((Style::new(UI_RED, bg).bold(), format!(" {label} "))),
            None => segments.push((Style::new(if active { icon_fg } else { fg }, bg), format!(" {icon} "))),
        }
        segments.push((if active { name.bold().italic() } else { name }, buf.name()));
        segments.push(if buf.is_dirty() {
            (Style::new(UI_GREEN, bg), MODIFIED.into())
        } else {
            (Style::new(fg, bg), CLOSE.into())
        });
    }
    bar(depth, width, &segments, &[], Style::new(TEXT, TABLINE))
}

pub fn statusline(ed: &Editor, ctx: &Context, width: usize, depth: ColorDepth) -> String {
    let buf = ed.buf();
    let mode = mode_color(ed.mode);
    let (icon, icon_fg) = file_icon(buf);
    let mut left = vec![
        (Style::new(BASE, mode).bold(), format!(" \u{e7c5} {} ", ed.mode.label())),
        (Style::new(mode, PROMPT), "\u{e0b4}".into()),
        (Style::new(icon_fg, PROMPT), format!(" {icon} ")),
        (Style::new(UI_TEXT, PROMPT), format!("{} ", buf.name())),
        (Style::new(PROMPT, STATUSLINE), "\u{e0b4}".into()),
    ];
    if let Some(branch) = &ctx.branch {
        left.push((Style::new(UI_TEXT, STATUSLINE), format!(" \u{e725} {branch}")));
    }
    let Pos { line, col } = buf.cursor;
    let progress = match line {
        0 => "Top".into(),
        l if l == buf.last_line() => "Bot".into(),
        l => format!("{}%", (l + 1) * 100 / buf.line_count()),
    };
    let right = [
        (Style::new(UI_RED, STATUSLINE), "\u{e0b6}".into()),
        (Style::new(BASE, UI_RED), "\u{f07b} ".into()),
        (Style::new(UI_TEXT, PROMPT), format!(" {} ", ctx.cwd)),
        (Style::new(UI_GREEN, PROMPT), "\u{e0b6}".into()),
        (Style::new(BASE, UI_GREEN), "\u{e612} ".into()),
        (Style::new(UI_TEXT, PROMPT), format!(" {}:{} {progress} ", line + 1, col + 1)),
    ];
    bar(depth, width, &left, &right, Style::new(UI_TEXT, STATUSLINE))
}

pub fn cmdline(ed: &Editor, width: usize, depth: ColorDepth) -> String {
    let normal = Style::new(TEXT, BASE);
    let (style, text) = match (ed.mode, &ed.message) {
        (Mode::Command, _) => (normal, format!(":{}", ed.cmdline)),
        (Mode::Search, _) => (normal, format!("/{}", ed.cmdline)),
        (_, Some(Message::Error(m))) => (Style::new(RED, BASE), m.clone()),
        (_, Some(Message::Info(m))) => (normal, m.clone()),
        (_, None) => (normal, String::new()),
    };
    bar(depth, width, &[(style, text)], &[], normal)
}
