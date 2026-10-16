//! One window: winbar, then buffer text with AstroNvim's options
//! (number + relativenumber, signcolumn=yes, cursorline, nowrap).


use super::row::Row;
use super::theme::{
    self, BASE, CURSOR_LINE, INACTIVE_BASE, LINE_NR, SELECTION, Style, TEXT, TEXT_INACTIVE, UI_PURPLE, WINBAR,
};
use crate::buffer::{Buffer, Pos, text};
use crate::editor::{Editor, Rect, Selection};
use crate::syntax::Hl;
use crate::term::ColorDepth;

/// A rendered window: exactly `rect.h` rows of `rect.w` cells, and the cursor cell when focused.
pub struct Drawn {
    pub rows: Vec<String>,
    pub cursor: Option<(usize, usize)>,
}

pub fn window(ed: &mut Editor, index: usize, rect: Rect, depth: ColorDepth) -> Drawn {
    let active = index == ed.active;
    let selection = if active { ed.selection() } else { None };
    let pattern = if ed.highlight_search { ed.search.clone() } else { String::new() };
    let bg = if active { BASE } else { INACTIVE_BASE };
    let drawn = ed.with_view(index, |buf| {
        let mut rows = Vec::with_capacity(rect.h);
        let has_winbar = rect.h > 1;
        if has_winbar {
            rows.push(winbar(buf, rect.w, depth, active, bg));
        }
        let height = rect.h - usize::from(has_winbar);
        let cursor = text_rows(buf, rect.w, height, depth, bg, selection, &pattern, &mut rows);
        let cursor = active.then_some((rect.y + usize::from(has_winbar) + cursor.0, rect.x + cursor.1));
        Drawn { rows, cursor }
    });
    drawn.unwrap_or_else(|| Drawn { rows: (0..rect.h).map(|_| Row::new(depth, rect.w).finish(Style::new(TEXT, bg))).collect(), cursor: None })
}

/// heirline winbar: the file's path relative to the working directory, dimmed when unfocused.
fn winbar(buf: &Buffer, width: usize, depth: ColorDepth, active: bool, bg: theme::Rgb) -> String {
    let style = Style::new(if active { WINBAR } else { TEXT_INACTIVE }, bg);
    let mut row = Row::new(depth, width);
    if let Some(path) = &buf.path {
        let cwd = std::env::current_dir().unwrap_or_default();
        let shown = path.strip_prefix(&cwd).unwrap_or(path);
        let parts: Vec<String> = shown.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        row.text(style, " ");
        for (i, part) in parts.iter().enumerate() {
            if i + 1 == parts.len() {
                let (icon, color) = buf.lang.map_or(('\u{f15b}', WINBAR), |l| (l.icon, theme::Rgb(l.icon_color)));
                row.text(Style::new(if active { color } else { TEXT_INACTIVE }, bg), &format!("{icon} "));
                row.text(style.bold(), part);
            } else {
                row.text(style, part);
                row.text(style, " \u{f105} ");
            }
        }
    }
    row.finish(Style::new(TEXT, bg))
}

/// Pushes `height` text rows; returns the cursor's (row, col) inside them.
#[allow(clippy::too_many_arguments)]
fn text_rows(
    buf: &mut Buffer,
    width: usize,
    height: usize,
    depth: ColorDepth,
    bg: theme::Rgb,
    selection: Option<Selection>,
    pattern: &str,
    rows: &mut Vec<String>,
) -> (usize, usize) {
    let number_width = buf.line_count().to_string().len().max(3);
    let gutter = number_width + 3; // fold space, numbers, 2-cell sign column
    let cursor = buf.cursor;
    let cursor_x = text::display_col(buf.line(cursor.line), cursor.col);
    scroll(buf, cursor_x, height.max(1), width.saturating_sub(gutter).max(1));
    for y in buf.top..buf.top + height {
        let mut row = Row::new(depth, width);
        if y < buf.line_count() {
            let current = y == cursor.line;
            let number = if current { y + 1 } else { y.abs_diff(cursor.line) };
            row.text(Style::new(TEXT, bg), " ");
            row.text(Style::new(if current { TEXT } else { LINE_NR }, bg), &format!("{number:>number_width$}"));
            row.text(Style::new(TEXT, bg), "  ");
            let groups = buf.highlight_line(y);
            draw_line(&mut row, buf, y, groups.as_deref(), selection, pattern, bg);
        }
        rows.push(row.finish(Style::new(TEXT, bg)));
    }
    (cursor.line.saturating_sub(buf.top), (gutter + cursor_x).saturating_sub(buf.left))
}

fn scroll(buf: &mut Buffer, cursor_x: usize, height: usize, width: usize) {
    let line = buf.cursor.line;
    if line < buf.top {
        buf.top = line;
    } else if line >= buf.top + height {
        buf.top = line + 1 - height;
    }
    if cursor_x < buf.left {
        buf.left = cursor_x;
    } else if cursor_x >= buf.left + width {
        buf.left = cursor_x + 1 - width;
    }
}

// shortcut: highlights the whole line even when only part is visible; slice to the view for huge lines.
fn draw_line(
    row: &mut Row,
    buf: &Buffer,
    y: usize,
    groups: Option<&[Hl]>,
    selection: Option<Selection>,
    pattern: &str,
    base: theme::Rgb,
) {
    let line = buf.line(y);
    let matches = text::find_all(line, pattern);
    let cursor = buf.cursor;
    let bg = if y == cursor.line { CURSOR_LINE } else { base };
    let mut next_match = 0;
    let mut screen_x = 0;
    for (x, c) in line.chars().enumerate() {
        let cells = if c == '\t' { text::TAB_WIDTH - screen_x % text::TAB_WIDTH } else { text::char_width(c) };
        let end = screen_x + cells;
        if end <= buf.left && cells > 0 {
            screen_x = end;
            continue;
        }
        while matches.get(next_match).is_some_and(|m| m.end <= x) {
            next_match += 1;
        }
        let hit = matches.get(next_match).filter(|m| m.contains(&x));
        let group = groups.and_then(|g| g.get(x)).copied().unwrap_or(Hl::Text);
        let mut style = theme::syntax(group, bg);
        if hit.is_some() || selection.is_some_and(|s| s.contains(Pos::new(y, x))) {
            style.bg = SELECTION;
        }
        if hit.is_some_and(|m| y == cursor.line && m.contains(&cursor.col)) {
            style = Style::new(BASE, UI_PURPLE); // CurSearch
        }
        if c == '\t' || screen_x < buf.left {
            // Tabs, and wide chars cut by the left edge, show as blanks over their visible cells.
            for _ in screen_x.max(buf.left)..end {
                row.cell(style, ' ');
            }
        } else if cells > 0 || screen_x > buf.left {
            row.cell(style, c);
        }
        screen_x = end;
        if row.is_full() {
            break;
        }
    }
    row.pad_to(Style::new(TEXT, bg), usize::MAX);
}
