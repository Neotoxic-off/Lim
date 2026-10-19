//! snacks.picker's default layout: input and list on the left, preview on the right.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::Theme;
use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::buffer::text;
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
    let list_w: usize = if w >= 80 { w * 45 / 100 } else { w };
    let fill: Style = t.style(Group::NormalFloat);
    let border: Style = t.style(Group::FloatBorder);

    // Input, on its own shade like NvChad's telescope prompt.
    let prompt: Style = t.style(Group::PickerInput);
    grid.float(Rect { x, y, w: list_w, h: 3 }, border, prompt, picker.kind.title());
    let mut input: Row = Row::new(list_w - 2);
    input.text(Style { fg: t.bg(Group::FloatTitle), bold: true, ..prompt }, &format!(" {} ", set.search));
    input.text(prompt, &picker.query);
    let cursor_col: usize = x + 1 + input.used();
    let count: String = format!("{}/{} ", picker.matches.len(), picker.items.len());
    input.pad_to(prompt, (list_w - 2).saturating_sub(text::str_width(&count)));
    input.text(t.on(Group::LineNr, prompt.bg), &count);
    grid.put(x + 1, y + 1, &input.finish(prompt));

    // Results.
    let list: Rect = Rect { x, y: y + 3, w: list_w, h: h - 3 };
    grid.float(list, border, fill, "");
    let rows: usize = list.h.saturating_sub(2);
    let first: usize = picker.selected.saturating_sub(rows.saturating_sub(1));
    for (r, m) in picker.matches.iter().enumerate().skip(first).take(rows) {
        let Some(item): Option<&Item> = picker.items.get(m.item) else { continue };
        let selected: bool = r == picker.selected;
        let bg: Rgb = if selected { t.bg(Group::PmenuSel) } else { fill.bg };
        let base: Style = Style { bg, bold: selected, ..fill };
        let mut row: Row = Row::new(list.w - 2);
        row.text(Style { fg: t.fg(Group::BufferPicker), ..base }, if selected { "\u{258c}" } else { " " });
        if let Target::File(path, _) = &item.target {
            let (icon, color): (String, Option<u32>) = set.for_file(syntax::lang_for(path));
            if !icon.is_empty() {
                row.text(Style { fg: color.map_or(base.fg, Rgb), ..base }, &format!("{icon} "));
            }
        }
        let dir_end: usize = match &item.target {
            Target::File(..) => item.label.rfind('/').map_or(0, |i| text::char_len(&item.label[..=i])),
            _ => 0,
        };
        for (i, c) in item.label.chars().enumerate() {
            let style: Style = if m.positions.contains(&i) {
                Style { bg, ..t.on(Group::PickerMatch, bg) }
            } else if i < dir_end {
                Style { fg: t.fg(Group::PickerDir), ..base }
            } else {
                base
            };
            row.cell(style, c);
        }
        grid.put(list.x + 1, list.y + 1 + r - first, &row.finish(base));
    }

    // Preview.
    if list_w < w {
        let area: Rect = Rect { x: x + list_w, y, w: w - list_w, h };
        preview(ed, grid, area, fill, border);
    }
    Some((y + 1, cursor_col))
}

fn preview(ed: &Editor, grid: &mut Grid, area: Rect, fill: Style, border: Style) {
    let t: Arc<Theme> = theme();
    let Some(item): Option<&Item> = ed.picker.as_ref().and_then(|p| p.selected_item()) else {
        return grid.float(area, border, fill, "");
    };
    let (title, lines, focus, path): (String, Vec<String>, usize, Option<&Path>) = match &item.target {
        Target::File(path, pos) => {
            let content: Option<Vec<u8>> = fs::read(path).ok().filter(|b| !b.iter().take(8000).any(|&c| c == 0));
            let content: Option<String> = content.map(|mut b| {
                b.truncate(MAX_PREVIEW_BYTES);
                String::from_utf8_lossy(&b).into_owned()
            });
            let lines: Vec<String> = content.unwrap_or_default().lines().map(str::to_owned).collect();
            (path.display().to_string(), lines, pos.map_or(0, |p| p.line), Some(path))
        }
        Target::Buffer(i) => match ed.buffers.get(*i) {
            Some(buf) => (buf.name(), buf.lines().to_vec(), buf.cursor.line, buf.path.as_deref()),
            None => return,
        },
        Target::Line(y) => (ed.buf().name(), ed.buf().lines().to_vec(), *y, ed.buf().path.as_deref()),
        // The screen is already redrawn in the theme under the cursor; the sample shows each of its colors.
        Target::Theme(name) => {
            (name.clone(), THEME_SAMPLE.lines().map(str::to_owned).collect(), 0, Some(Path::new("sample.rs")))
        }
        // Icon sets preview on the current buffer, already redrawn in them.
        Target::IconSet(name) => {
            (name.clone(), ed.buf().lines().to_vec(), ed.buf().cursor.line, ed.buf().path.as_deref())
        }
        _ => match &ed.picker.as_ref().and_then(|p| p.preview.clone()) {
            Some(preview) => (preview.title.clone(), preview.lines.clone(), 0, None),
            None => return grid.float(area, border, fill, ""),
        },
    };
    grid.float(area, border, fill, &title);
    let rows: usize = area.h.saturating_sub(2);
    let first: usize = focus.saturating_sub(rows / 3);
    let lang: Option<&syntax::Lang> = path.and_then(syntax::lang_for);
    let number_width: usize = (first + rows).to_string().len();
    for (r, line) in lines.iter().enumerate().skip(first).take(rows) {
        let bg: Rgb = if r == focus && focus > 0 { t.bg(Group::CursorLine) } else { fill.bg };
        let mut row: Row = Row::new(area.w - 2);
        row.text(t.on(Group::LineNr, bg), &format!("{:>number_width$} ", r + 1));
        let chars: Vec<char> = line.chars().collect();
        let groups: Option<Vec<syntax::Hl>> = lang.map(|l| syntax::highlight(&chars, l, State::default()).0);
        let mut col: usize = 0;
        for (i, &c) in chars.iter().enumerate() {
            let style: Style =
                groups.as_ref().and_then(|g| g.get(i)).map_or(Style { bg, ..fill }, |&g| t.syntax(g, bg));
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
        grid.put(area.x + 1, area.y + 1 + r - first, &row.finish(Style { bg, ..fill }));
    }
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
