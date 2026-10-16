//! snacks.dashboard as AstroNvim configures it: header, keys, startup time.

use std::time::Duration;

use super::row::{Grid, Row};
use super::theme::{Group, Style, theme};
use crate::buffer::text;
use crate::editor::{Editor, Rect};

/// The AstroNvim header's block font, spelling LIM.
const HEADER: &[&str] = &[
    "██      ██ ███    ███",
    "██      ██ ████  ████",
    "██      ██ ██ ████ ██",
    "██      ██ ██  ██  ██",
    "███████ ██ ██      ██",
];

const KEYS: &[(char, &str, &str)] = &[
    ('\u{f0f6}', "New File", "n"),
    ('\u{f002}', "Find File", "f"),
    ('\u{f1da}', "Recents", "o"),
    ('\u{f15c}', "Find Word", "w"),
];

const KEYS_WIDTH: usize = 50;

pub fn dashboard(ed: &Editor, grid: &mut Grid, area: Rect, startup: Duration) {
    let t = theme();
    let base = t.style(Group::Normal);
    let footer = format!("{}lim started in {:.2}ms", if ed.options.icons { "\u{26a1} " } else { "" }, startup.as_secs_f64() * 1000.0);
    // header, padding 5, keys with a blank line between, padding 3, footer: as snacks lays it out.
    let height = HEADER.len() + 5 + KEYS.len() * 2 - 1 + 3 + 1;
    let mut y = area.y + area.h.saturating_sub(height) / 2;
    let centered = |w: usize| area.x + area.w.saturating_sub(w) / 2;
    for line in HEADER {
        let w = text::str_width(line);
        grid.text(centered(w), y, w, t.on(Group::DashboardHeader, base.bg), line);
        y += 1;
    }
    y += 5;
    for (icon, desc, key) in KEYS {
        if y >= area.y + area.h {
            return;
        }
        let mut row = Row::new(KEYS_WIDTH.min(area.w));
        if ed.options.icons {
            row.text(t.on(Group::DashboardDesc, base.bg), &format!("{icon}  "));
        }
        row.text(t.on(Group::DashboardDesc, base.bg), desc);
        row.pad_to(base, KEYS_WIDTH.min(area.w).saturating_sub(1));
        row.text(t.on(Group::DashboardKey, base.bg), key);
        grid.put(centered(KEYS_WIDTH), y, &row.finish(base));
        y += 2;
    }
    y += 2;
    if y < area.y + area.h {
        let w = text::str_width(&footer);
        grid.text(centered(w), y, w, Style { ..t.on(Group::DashboardFooter, base.bg) }, &footer);
    }
}
