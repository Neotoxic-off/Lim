//! snacks.picker's default layout: input and list on the left, preview on the right.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::Theme;
use super::row::{Cell, Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::buffer::{Buffer, text};
use crate::editor::{Editor, Item, Picker, Rect, Target};
use crate::render::icons::Icons;
use crate::syntax::{self, State};

/// Previews read at most this much of a file.
const MAX_PREVIEW_BYTES: usize = 256 * 1024;

/// What the theme picker previews: a bit of every color, whatever buffer is open.
const THEME_SAMPLE: &str = r#"/// A hydra grows two heads for each one cut.
use std::collections::HashMap;

const LIMIT: usize = 9;

#[derive(Debug, Clone)]
struct Hydra {
    name: String,
    heads: u32,
}

fn cut(hydra: &mut Hydra, count: u32) -> bool {
    // TODO: Heracles burns the stumps
    hydra.heads += count * 2;
    println!("{} has {} heads", hydra.name, hydra.heads);
    let seen: HashMap<&str, f64> = HashMap::new();
    hydra.heads as usize > LIMIT && seen.is_empty()
}"#;

/// Draws the open picker over the screen; returns the cursor cell in its input.
pub fn picker(ed: &Editor, grid: &mut Grid) -> Option<(usize, usize)> {
    let picker: &Picker = ed.picker.as_ref()?;
    let t: Arc<Theme> = theme();
    let set: &Icons = super::icons::icons(&ed.options);
    let (sw, sh): (usize, usize) = (grid.width, grid.height);
    let w: usize = (sw * 8 / 10).max(sw.min(40));
    let h: usize = (sh * 8 / 10).max(sh.min(10));
    if w < 12 || h < 6 {
        return None;
    }
    let (x, y): (usize, usize) = ((sw - w) / 2, (sh - h) / 2);
    // The list takes the whole width on a narrow screen, else leaves the right part to the preview.
    let list_w: usize = if w >= 80 { w * 45 / 100 } else { w };
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.style(Group::FloatBorder);
    let cursor_col: usize = input(&t, set, picker, grid, Rect { x, y, w: list_w, h: 3 }, border);
    let list: Rect = Rect { x, y: y + 3, w: list_w, h: h - 3 };
    grid.float(list, border, fill, "");
    results(&t, set, picker, grid, list, fill);
    if list_w < w {
        preview(ed, grid, Rect { x: x + list_w, y, w: w - list_w, h }, fill, border);
    }
    Some((y + 1, cursor_col))
}

/// The query line, on its own shade like `NvChad`'s telescope prompt, with the match count on the right; returns
/// the cursor's column.
fn input(t: &Theme, set: &Icons, picker: &Picker, grid: &mut Grid, area: Rect, border: Style) -> usize {
    let prompt: Style = t.style(Group::PickerInput);
    grid.float(area, border, prompt, picker.kind.title());
    let mut input: Row = Row::new(area.w - 2);
    input.text(Style { fg: t.bg(Group::FloatTitle), bold: true, ..prompt }, &format!(" {} ", set.search));
    input.text(prompt, &picker.query);
    let cursor_col: usize = area.x + 1 + input.used();
    let count: String = format!("{}/{} ", picker.matches.len(), picker.items.len());
    input.pad_to(prompt, (area.w - 2).saturating_sub(text::str_width(&count)));
    input.text(t.on(Group::LineNr, prompt.bg), &count);
    grid.put(area.x + 1, area.y + 1, &input.finish(prompt));
    cursor_col
}

/// The matches that fit in `list`, scrolled to keep the selected one in view.
fn results(t: &Theme, set: &Icons, picker: &Picker, grid: &mut Grid, list: Rect, fill: Style) {
    let rows: usize = list.h.saturating_sub(2);
    let first: usize = picker.selected.saturating_sub(rows.saturating_sub(1));
    for (r, m) in picker.matches.iter().enumerate().skip(first).take(rows) {
        let Some(item): Option<&Item> = picker.items.get(m.item) else { continue };
        let row: Vec<Cell> = result_row(t, set, item, &m.positions, r == picker.selected, fill, list.w - 2);
        grid.put(list.x + 1, list.y + 1 + r - first, &row);
    }
}

/// One match: the selection bar, a file's icon, then the label with its folder dimmed and the matched chars lit.
fn result_row(
    t: &Theme,
    set: &Icons,
    item: &Item,
    positions: &[usize],
    selected: bool,
    fill: Style,
    width: usize,
) -> Vec<Cell> {
    let bg: Rgb = if selected { t.bg(Group::PmenuSel) } else { fill.bg };
    let base: Style = Style { bg, bold: selected, ..fill };
    let mut row: Row = Row::new(width);
    row.text(Style { fg: t.fg(Group::BufferPicker), ..base }, if selected { "\u{258c}" } else { " " });
    let mut dir_end: usize = 0;
    if let Target::File(path, _) = &item.target {
        let (icon, color): (String, Option<u32>) = set.for_file(syntax::lang_for(path));
        if !icon.is_empty() {
            row.text(Style { fg: color.map_or(base.fg, Rgb), ..base }, &format!("{icon} "));
        }
        dir_end = item.label.rfind('/').map_or(0, |i| text::char_len(&item.label[..=i]));
    }
    let matched: Style = Style { bg, ..t.on(Group::PickerMatch, bg) };
    let dir: Style = Style { fg: t.fg(Group::PickerDir), ..base };
    for (i, c) in item.label.chars().enumerate() {
        let style: Style = if positions.contains(&i) {
            matched
        } else if i < dir_end {
            dir
        } else {
            base
        };
        row.cell(style, c);
    }
    row.finish(base)
}

/// What the preview shows: a title, the lines, the line to bring into view and the path that picks the
/// highlighter.
struct Shown<'a> {
    title: String,
    lines: Vec<String>,
    focus: usize,
    path: Option<&'a Path>,
}

/// The selected item's preview: its file or buffer around its line, the theme sample, or the source's own text.
fn preview(ed: &Editor, grid: &mut Grid, area: Rect, fill: Style, border: Style) {
    let Some(shown): Option<Shown<'_>> = preview_source(ed) else {
        return grid.float(area, border, fill, "");
    };
    grid.float(area, border, fill, &shown.title);
    let rows: usize = area.h.saturating_sub(2);
    let first: usize = shown.focus.saturating_sub(rows / 3);
    let lang: Option<&syntax::Lang> = shown.path.and_then(syntax::lang_for);
    let number_width: usize = (first + rows).to_string().len();
    for (r, line) in shown.lines.iter().enumerate().skip(first).take(rows) {
        // The item's own line stands out, unless it is the first (a file opened at its top).
        let focused: bool = r == shown.focus && shown.focus > 0;
        let row: Vec<Cell> = code_row(line, r + 1, number_width, lang, focused, fill, area.w - 2);
        grid.put(area.x + 1, area.y + 1 + r - first, &row);
    }
}

fn preview_source(ed: &Editor) -> Option<Shown<'_>> {
    let item: &Item = ed.picker.as_ref().and_then(|p| p.selected_item())?;
    Some(match &item.target {
        Target::File(path, pos) => Shown {
            title: path.display().to_string(),
            lines: read_preview(path),
            focus: pos.map_or(0, |p| p.line),
            path: Some(path),
        },
        Target::Buffer(i) => {
            let buf: &Buffer = ed.buffers.get(*i)?;
            of_buffer(buf.name(), buf, buf.cursor.line)
        }
        Target::Line(y) => of_buffer(ed.buf().name(), ed.buf(), *y),
        // The screen is already redrawn in the theme under the cursor; the sample shows each of its colors.
        Target::Theme(name) => Shown {
            title: name.clone(),
            lines: THEME_SAMPLE.lines().map(str::to_owned).collect(),
            focus: 0,
            path: Some(Path::new("sample.rs")),
        },
        // Icon sets preview on the current buffer, already redrawn in them.
        Target::IconSet(name) => of_buffer(name.clone(), ed.buf(), ed.buf().cursor.line),
        _ => {
            let preview = ed.picker.as_ref().and_then(|p| p.preview.clone())?;
            Shown { title: preview.title, lines: preview.lines, focus: 0, path: None }
        }
    })
}

/// A buffer's text, brought into view around line `focus`.
fn of_buffer(title: String, buf: &Buffer, focus: usize) -> Shown<'_> {
    Shown { title, lines: buf.lines().to_vec(), focus, path: buf.path.as_deref() }
}

/// The start of a text file, at most `MAX_PREVIEW_BYTES`; nothing for a binary one.
fn read_preview(path: &Path) -> Vec<String> {
    let Some(mut bytes): Option<Vec<u8>> = fs::read(path).ok().filter(|b| !b.iter().take(8000).any(|&c| c == 0)) else {
        return Vec::new();
    };
    bytes.truncate(MAX_PREVIEW_BYTES);
    String::from_utf8_lossy(&bytes).lines().map(str::to_owned).collect()
}

/// A line of code with its number, highlighted for `lang`, tabs expanded.
fn code_row(
    line: &str,
    number: usize,
    number_width: usize,
    lang: Option<&syntax::Lang>,
    focused: bool,
    fill: Style,
    width: usize,
) -> Vec<Cell> {
    let t: Arc<Theme> = theme();
    let bg: Rgb = if focused { t.bg(Group::CursorLine) } else { fill.bg };
    let plain: Style = Style { bg, ..fill };
    let mut row: Row = Row::new(width);
    row.text(t.on(Group::LineNr, bg), &format!("{number:>number_width$} "));
    let chars: Vec<char> = line.chars().collect();
    let groups: Option<Vec<syntax::Hl>> = lang.map(|l| syntax::highlight(&chars, l, State::default()).0);
    let mut col: usize = 0;
    for (i, &c) in chars.iter().enumerate() {
        let style: Style = groups.as_ref().and_then(|g| g.get(i)).map_or(plain, |&g| t.syntax(g, bg));
        if c == '\t' {
            for _ in 0..text::tab_cells(col) {
                row.cell(style, ' ');
            }
            col += text::tab_cells(col);
        } else {
            row.cell(style, c);
            col += text::char_width(c);
        }
    }
    row.finish(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Screen;
    use crate::render::row::Cell;
    use crate::term::{ColorDepth, Key};

    /// The theme picker shows its sample code, highlighted, even over an empty buffer.
    #[test]
    fn the_theme_picker_previews_a_sample() {
        let mut ed: Editor = Editor::new(&[]);
        ed.dashboard = false;
        for c in " ft".chars() {
            ed.handle(Key::Char(c));
        }
        let (grid, _): (Grid, Option<(usize, usize)>) = Screen::new(ColorDepth::TrueColor).compose(&mut ed, 120, 40);
        let row: Option<usize> = (0..grid.height).find(|&y| {
            let line: String = grid.row(y).iter().map(|c| c.ch).collect();
            line.contains("struct Hydra {")
        });
        let y: usize = row.expect("the sample is in the preview");
        let cells: &[Cell] = grid.row(y);
        let at = |word: &str| -> Rgb {
            let chars: Vec<char> = word.chars().collect();
            let start: Option<usize> =
                cells.windows(chars.len()).position(|w| w.iter().map(|c| c.ch).eq(chars.iter().copied()));
            start.map_or(Rgb(0), |i| cells[i].style.fg)
        };
        assert_eq!(at("struct"), theme().fg(Group::Keyword));
        assert_eq!(at("Hydra"), theme().fg(Group::Type));
        ed.handle(Key::Esc);
    }
}
