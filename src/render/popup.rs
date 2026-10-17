//! Floating popups drawn over the windows: which-key, the prompt and the cheat sheet.

use std::sync::Arc;

use super::Theme;
use super::icons::Icons;
use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::buffer::text;
use crate::editor::{Card, Cheatsheet, CmdCompletion, Completion, Editor, MenuEntry, Prompt, Rect, SETTINGS, Setup};

/// Most rows a which-key menu takes before it truncates.
const MAX_ROWS: usize = 12;

/// which-key's classic preset: a bordered panel along the bottom of `area`, entries in columns.
pub fn which_key(ed: &Editor, grid: &mut Grid, area: Rect) {
    let Some((title, entries)): Option<(String, Vec<MenuEntry>)> = ed.which_key() else { return };
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let arrow: String = format!(" {} ", super::icons::icons(&ed.options).arrow);
    let arrow: &str = arrow.as_str();
    let cell_width: usize = entries
        .iter()
        .map(|e| text::str_width(&e.key) + text::str_width(arrow) + text::str_width(&e.desc) + 3)
        .max()
        .unwrap_or(0)
        .min(48);
    let inner: usize = area.w.saturating_sub(4);
    if inner < 8 || cell_width == 0 || area.h < 4 {
        return;
    }
    let columns: usize = (inner / cell_width).max(1);
    let rows: usize = entries.len().div_ceil(columns).min(MAX_ROWS).min(area.h.saturating_sub(2));
    let height: usize = rows + 2;
    let top: usize = area.y + area.h - height;
    grid.float(Rect { x: area.x, y: top, w: area.w, h: height }, border, fill, &title);
    for (i, entry) in entries.iter().enumerate().take(rows * columns) {
        let (column, row): (usize, usize) = (i / rows, i % rows);
        let mut cell: Row = Row::new(cell_width.min(inner));
        cell.text(t.on(Group::WhichKey, fill.bg), &entry.key);
        cell.text(t.on(Group::WhichKeySeparator, fill.bg), arrow);
        let desc: Group = if entry.group { Group::WhichKeyPrefix } else { Group::WhichKeyDesc };
        cell.text(t.on(desc, fill.bg), &entry.desc);
        grid.put(area.x + 2 + column * cell_width, top + 1 + row, &cell.finish(Style { ..fill }));
    }
}

/// snacks.input: the prompt float, a third of the way down; returns the cursor cell.
pub fn prompt(ed: &Editor, grid: &mut Grid) -> Option<(usize, usize)> {
    let prompt: &Prompt = ed.prompt.as_ref()?;
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let width: usize = (text::str_width(&prompt.text) + 6).max(40).min(grid.width.saturating_sub(4));
    if width < 10 || grid.height < 4 {
        return None;
    }
    let (x, y): (usize, usize) = ((grid.width - width) / 2, grid.height / 3);
    grid.float(Rect { x, y, w: width, h: 3 }, border, fill, &prompt.title);
    let inner: usize = width - 4;
    // Keep the end of long input visible.
    let before: String = prompt.text.chars().take(prompt.cursor).collect();
    let skip: usize = text::str_width(&before).saturating_sub(inner.saturating_sub(1));
    let shown: String = prompt.text.chars().skip(skip).collect();
    grid.text(x + 2, y + 1, inner, fill, &shown);
    Some((y + 1, x + 2 + text::str_width(&before) - skip))
}

/// The cheat sheet: cards laid out in columns (shortest column first), scrolled as one.
pub fn cheatsheet(ed: &Editor, grid: &mut Grid) {
    let Some(sheet): &Option<Cheatsheet> = &ed.cheatsheet else { return };
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let (w, h): (usize, usize) =
        ((grid.width * 9 / 10).max(grid.width.min(30)), (grid.height * 9 / 10).max(grid.height.min(8)));
    if w < 24 || h < 6 {
        return;
    }
    let (x, y): (usize, usize) = ((grid.width - w) / 2, (grid.height - h) / 2);
    let title: String =
        if sheet.filter.is_empty() { "Cheatsheet".to_owned() } else { format!("Cheatsheet: {}", sheet.filter) };
    grid.float(Rect { x, y, w, h }, border, fill, &title);
    let inner: Rect = Rect { x: x + 2, y: y + 1, w: w - 4, h: h - 2 };
    // As many columns as fit at 46 cells, sharing the width.
    let columns: usize = (inner.w / 46).max(1);
    let column_width: usize = inner.w / columns;
    let accents: [Group; 6] = [
        Group::StatusModeNormal,
        Group::StatusModeInsert,
        Group::StatusModeVisual,
        Group::StatusModeCommand,
        Group::StatusCwd,
        Group::StatusModeReplace,
    ];
    // Lay every card into the shortest column, as virtual rows.
    let mut heights: Vec<usize> = vec![0usize; columns];
    let mut placed: Vec<(usize, usize, usize, Card)> = Vec::new();
    for (i, card) in ed.cheatsheet_cards().into_iter().enumerate() {
        let column: usize = (0..columns).min_by_key(|&c| heights[c]).unwrap_or(0);
        placed.push((column, heights[column], i, card));
        heights[column] += placed.last().map_or(0, |p: &(usize, usize, usize, Card)| p.3.rows.len() + 3);
    }
    let scroll: usize = sheet.scroll.min(heights.iter().max().copied().unwrap_or(0).saturating_sub(inner.h));
    for (column, top, i, card) in placed {
        let cx: usize = inner.x + column * column_width;
        let width: usize = column_width.saturating_sub(2);
        let row_at = |r: usize| (top + r).checked_sub(scroll).filter(|&v| v < inner.h).map(|v| inner.y + v);
        if let Some(ry) = row_at(0) {
            let chip: Style = t.style(accents[i % accents.len()]);
            let mut row: Row = Row::new(width);
            row.text(chip, &format!(" {} ", card.title));
            grid.put(cx, ry, &row.finish(fill));
        }
        for (r, (keys, desc)) in card.rows.iter().enumerate() {
            let Some(ry): Option<usize> = row_at(r + 2) else { continue };
            let mut row: Row = Row::new(width);
            row.text(t.on(Group::CheatsheetKey, fill.bg), keys);
            row.pad_to(fill, 18.min(width));
            row.text(fill, " ");
            row.text(t.on(Group::CheatsheetDesc, fill.bg), desc);
            grid.put(cx, ry, &row.finish(fill));
        }
    }
}

/// The settings screen over the whole window, as VS Code's settings tab; it is drawn in the theme it changes.
pub fn setup(ed: &Editor, grid: &mut Grid) {
    let Some(setup): &Option<Setup> = &ed.setup else { return };
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let (w, h): (usize, usize) = (grid.width, grid.height);
    if w < 30 || h < 7 {
        return;
    }
    let area: Rect = Rect { x: 0, y: 0, w, h };
    // Float borders keep the background under them; here that would be the bars' colors.
    grid.fill(area, fill);
    grid.float(area, border, fill, "Settings");
    let hint: Style = Style { fg: t.fg(Group::Comment), ..fill };
    let search: String = if setup.typing {
        format!(" / {}█", setup.query)
    } else if setup.query.is_empty() {
        " / Search settings".into()
    } else {
        format!(" / {}", setup.query)
    };
    let field: Style = if setup.typing || !setup.query.is_empty() { fill } else { hint };
    grid.text(area.x + 2, area.y + 1, w - 4, field, &search);
    let inner: Rect = Rect { x: area.x + 2, y: area.y + 3, w: w - 4, h: h.saturating_sub(5) };
    // A section title row before each new section, then one row per setting.
    let shown: Vec<usize> = setup.shown();
    let current: Option<usize> = shown.get(setup.selected).copied();
    let mut rows: Vec<(Option<usize>, &str)> = Vec::new();
    for (n, &i) in shown.iter().enumerate() {
        if n == 0 || SETTINGS[shown[n - 1]].section != SETTINGS[i].section {
            rows.push((None, SETTINGS[i].section));
        }
        rows.push((Some(i), SETTINGS[i].name));
    }
    if rows.is_empty() {
        grid.text(inner.x, inner.y, inner.w, hint, " No setting matches");
    }
    let at: usize = rows.iter().position(|(i, _)| *i == current).unwrap_or(0);
    let scroll: usize = (at + 1).saturating_sub(inner.h);
    let selected: Style = t.on(Group::PmenuSel, fill.bg);
    let selected: Style = Style { bg: t.bg(Group::PmenuSel), ..selected };
    for (y, (index, text)) in rows.iter().skip(scroll).take(inner.h).enumerate() {
        let mut row: Row = Row::new(inner.w);
        let base: Style = if index.is_some() && *index == current { selected } else { fill };
        match index {
            None => row.text(t.style(Group::StatusModeNormal), &format!(" {text} ")),
            Some(i) => {
                row.text(base, &format!(" {text}"));
                row.pad_to(base, 20);
                row.text(Style { fg: t.fg(Group::CheatsheetKey), ..base }, &format!(" {}", ed.setup_value(text)));
                row.pad_to(base, 40);
                row.text(Style { fg: t.fg(Group::Comment), italic: false, ..base }, &format!(" {}", SETTINGS[*i].desc));
            }
        }
        grid.put(inner.x, inner.y + y, &row.finish(base));
    }
    grid.text(
        inner.x,
        area.y + h - 2,
        inner.w,
        hint,
        " / search · j/k move · h/l change · w save to config · Esc close",
    );
}

/// blink.cmp's menu under (or above) the cursor: kind icon, label with the typed prefix, source.
pub fn completion(ed: &Editor, grid: &mut Grid, cursor: Option<(usize, usize)>) {
    let (Some(menu), Some((row, col))): (&Option<Completion>, Option<(usize, usize)>) = (&ed.completion, cursor) else {
        return;
    };
    let t: Arc<Theme> = theme();
    let set: &Icons = super::icons::icons(&ed.options);
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let shown: usize = menu.items.len().min(crate::editor::MENU_HEIGHT);
    let first: usize = menu.selected.map_or(0, |s| s.saturating_sub(shown - 1));
    let label_width: usize = menu.items.iter().map(|w| text::str_width(w)).max().unwrap_or(0);
    let icon: String = Icons::spaced(set.word);
    let width: usize = (label_width + text::str_width(&icon) + " Buffer ".len() + 4).min(grid.width);
    let height: usize = shown + 2;
    let x: usize = col
        .saturating_sub(text::str_width(&menu.prefix) + 1 + text::str_width(&icon))
        .min(grid.width.saturating_sub(width));
    // Below the cursor when it fits, else above.
    let y: usize = if row + 1 + height <= grid.height { row + 1 } else { row.saturating_sub(height) };
    if width < " Buffer ".len() + 6 || height > grid.height {
        return;
    }
    grid.float(Rect { x, y, w: width, h: height }, border, fill, "");
    let prefix_len: usize = text::char_len(&menu.prefix);
    for (i, word) in menu.items.iter().enumerate().skip(first).take(shown) {
        let bg: Rgb = if menu.selected == Some(i) { t.bg(Group::PmenuSel) } else { fill.bg };
        let base: Style = Style { bg, ..fill };
        let mut cell: Row = Row::new(width - 2);
        cell.text(t.on(Group::CompletionKind, bg), &format!(" {icon}"));
        for (k, c) in word.chars().enumerate() {
            cell.cell(if k < prefix_len { t.on(Group::CompletionMatch, bg) } else { base }, c);
        }
        cell.pad_to(base, width - 2 - " Buffer ".len());
        cell.text(t.on(Group::CompletionSource, bg), " Buffer ");
        grid.put(x + 1, y + 1 + i - first, &cell.finish(base));
    }
}

/// The `:` completion menu, just above the command line, under the word being completed.
pub fn cmd_completion(ed: &Editor, grid: &mut Grid) {
    let Some(menu): &Option<CmdCompletion> = &ed.cmd_completion else { return };
    let t: Arc<Theme> = theme();
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.on(Group::FloatBorder, fill.bg);
    let shown: usize = menu.items.len().min(crate::editor::MENU_HEIGHT);
    let first: usize = menu.selected.map_or(0, |s| s.saturating_sub(shown - 1));
    let label: usize = menu.items.iter().map(|(c, _)| text::str_width(c)).max().unwrap_or(0);
    let desc: usize = menu.items.iter().map(|(_, d)| text::str_width(d)).max().unwrap_or(0);
    let width: usize = (label + desc + 6).min(grid.width);
    let height: usize = shown + 2;
    // Above the command line and the statusline.
    let below: usize = 1 + usize::from(ed.options.laststatus);
    if width < 8 || grid.height < height + below + 1 {
        return;
    }
    let before: String = ed.cmdline.chars().take(menu.start).collect();
    let x: usize = (1 + text::str_width(&before)).saturating_sub(2).min(grid.width - width);
    let y: usize = grid.height - below - height;
    grid.float(Rect { x, y, w: width, h: height }, border, fill, "");
    let typed: usize = text::char_len(&ed.cmdline) - menu.start;
    for (i, (item, about)) in menu.items.iter().enumerate().skip(first).take(shown) {
        let bg: Rgb = if menu.selected == Some(i) { t.bg(Group::PmenuSel) } else { fill.bg };
        let base: Style = Style { bg, ..fill };
        let mut row: Row = Row::new(width - 2);
        row.text(base, " ");
        for (k, c) in item.chars().enumerate() {
            let matched: bool = menu.selected.is_none() && k < typed;
            row.cell(if matched { t.on(Group::CompletionMatch, bg) } else { base }, c);
        }
        row.pad_to(base, label + 3);
        row.text(t.on(Group::CompletionSource, bg), about);
        grid.put(x + 1, y + 1 + i - first, &row.finish(base));
    }
}
