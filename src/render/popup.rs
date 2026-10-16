//! Floating popups drawn over the windows: the which-key menu.

use super::row::{Grid, Row};
use super::theme::{Group, Style, theme};
use crate::buffer::text;
use crate::editor::{Editor, Rect};

/// Most rows a which-key menu takes before it truncates.
const MAX_ROWS: usize = 12;

/// which-key's classic preset: a bordered panel along the bottom of `area`, entries in columns.
pub fn which_key(ed: &Editor, grid: &mut Grid, area: Rect) {
    let Some((title, entries)) = ed.which_key() else { return };
    let t = theme();
    let fill = t.style(Group::NormalFloat);
    let border = t.on(Group::FloatBorder, fill.bg);
    let arrow = if ed.options.icons { " \u{279c} " } else { " -> " };
    let cell_width = entries
        .iter()
        .map(|e| text::str_width(&e.key) + text::str_width(arrow) + text::str_width(&e.desc) + 3)
        .max()
        .unwrap_or(0)
        .min(48);
    let inner = area.w.saturating_sub(4);
    if inner < 8 || cell_width == 0 || area.h < 4 {
        return;
    }
    let columns = (inner / cell_width).max(1);
    let rows = entries.len().div_ceil(columns).min(MAX_ROWS).min(area.h.saturating_sub(2));
    let height = rows + 2;
    let top = area.y + area.h - height;
    grid.float(Rect { x: area.x, y: top, w: area.w, h: height }, border, fill, &title);
    for (i, entry) in entries.iter().enumerate().take(rows * columns) {
        let (column, row) = (i / rows, i % rows);
        let mut cell = Row::new(cell_width.min(inner));
        cell.text(t.on(Group::WhichKey, fill.bg), &entry.key);
        cell.text(t.on(Group::WhichKeySeparator, fill.bg), arrow);
        let desc = if entry.group { Group::WhichKeyPrefix } else { Group::WhichKeyDesc };
        cell.text(t.on(desc, fill.bg), &entry.desc);
        grid.put(area.x + 2 + column * cell_width, top + 1 + row, &cell.finish(Style { ..fill }));
    }
}
