//! One window: winbar, then the buffer with `AstroNvim`'s statuscolumn (fold space, numbers,
//! sign column), cursorline, search and selection highlights. No wrapping (`wrap = false`).

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::Theme;
use super::icons::icons;
use super::row::{Cell, Grid, Row};
use super::theme::{Group, Highlight, Rgb, Style, theme};
use crate::buffer::git::Sign;
use crate::buffer::{Buffer, CharClass, Pos, text};
use crate::config::Options;
use crate::editor::{Editor, Mode, Pane, Rect, Selection};
use crate::syntax::{self, Hl, Todo};

/// Draws window `index` into `rect`; returns the cursor's screen cell when it is the focused window.
pub fn window(ed: &mut Editor, index: usize, rect: Rect, grid: &mut Grid) -> Option<(usize, usize)> {
    let active: bool = index == ed.active;
    let ctx: Context = Context {
        active,
        normal_mode: ed.mode == Mode::Normal,
        selection: if active { ed.selection() } else { None },
        pattern: if ed.highlight_search && ed.options.hlsearch { ed.search.clone() } else { String::new() },
        options: if ed.zen { zen_options(&ed.options) } else { ed.options.clone() },
        base: theme().bg(if active { Group::Normal } else { Group::NormalNC }),
    };
    ed.with_view(index, |buf| {
        let winbar: bool = ctx.options.winbar && rect.h > 1;
        if winbar {
            grid.put(rect.x, rect.y, &winbar_cells(buf, rect.w, active, &ctx));
        }
        let top: usize = rect.y + usize::from(winbar);
        let area: Rect = Rect { y: top, h: rect.h - usize::from(winbar), ..rect };
        let (row, col, gutter): (usize, usize, usize) = text_rows(buf, area, &ctx, grid);
        let pane: Pane = Pane { window: index, area, gutter, top: buf.top, left: buf.left };
        (pane, active.then_some((top + row, rect.x + col)))
    })
    .and_then(|(pane, cursor)| {
        ed.panes.push(pane);
        cursor
    })
}

/// snacks.zen's window options: no numbers, signs, fold column or winbar.
fn zen_options(options: &Options) -> Options {
    Options {
        number: false,
        relativenumber: false,
        signcolumn: false,
        foldcolumn: false,
        winbar: false,
        ..options.clone()
    }
}

struct Context {
    active: bool,
    normal_mode: bool,
    selection: Option<Selection>,
    pattern: String,
    options: Options,
    base: Rgb,
}

/// heirline winbar: the file's path from the working directory, dimmed when unfocused.
fn winbar_cells(buf: &Buffer, width: usize, active: bool, ctx: &Context) -> Vec<Cell> {
    let t: Arc<Theme> = theme();
    let style: Style = t.on(if active { Group::WinBar } else { Group::WinBarNC }, ctx.base);
    let mut row: Row = Row::new(width);
    if let Some(path) = &buf.path {
        let cwd: PathBuf = std::env::current_dir().unwrap_or_default();
        let shown: &Path = path.strip_prefix(&cwd).unwrap_or(path);
        let parts: Vec<String> = shown.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        row.text(style, " ");
        for (i, part) in parts.iter().enumerate() {
            if i + 1 < parts.len() {
                row.text(style, part);
                row.text(style, &format!(" {} ", icons(&ctx.options).path_separator));
                continue;
            }
            let (icon, color): (String, Option<u32>) = icons(&ctx.options).for_file(buf.lang);
            if !icon.is_empty() {
                let fg: Rgb = if active { color.map_or(style.fg, Rgb) } else { style.fg };
                row.text(Style { fg, ..style }, &format!("{icon} "));
            }
            row.text(style.bold(), part);
        }
    }
    row.finish(Style::new(style.fg, ctx.base))
}

/// Text rows inside `area`; returns the cursor's (row, col) relative to it and the gutter's width.
fn text_rows(buf: &mut Buffer, area: Rect, ctx: &Context, grid: &mut Grid) -> (usize, usize, usize) {
    let t: Arc<Theme> = theme();
    let o: &Options = &ctx.options;
    let numbers: bool = o.number || o.relativenumber;
    let number_width: usize = if numbers { buf.line_count().to_string().len().max(3) } else { 0 };
    let signs: usize = if o.signcolumn { 2 } else { usize::from(numbers) };
    let fold: usize = usize::from(o.foldcolumn);
    let gutter: usize = fold + number_width + signs;
    let cursor: Pos = buf.cursor;
    let cursor_x: usize = text::display_col(buf.line(cursor.line), cursor.col);
    scroll(buf, cursor_x, area.h.max(1), area.w.saturating_sub(gutter).max(1), o);
    let bottom: usize = (buf.top + area.h).min(buf.line_count());
    let git: Vec<Option<Sign>> = if o.gitsigns && o.signcolumn {
        let top: usize = buf.top;
        buf.git_signs().get(top..bottom).map(<[_]>::to_vec).unwrap_or_default()
    } else {
        Vec::new()
    };
    let parens: [Option<Pos>; 2] = if o.matchparen && ctx.active { matching_pair(buf, area.h) } else { [None, None] };
    let word: Option<String> =
        if o.wordhighlight && ctx.active && ctx.normal_mode { word_under_cursor(buf) } else { None };
    let blank: Style = Style::new(t.fg(Group::Normal), ctx.base);
    for (r, y) in (buf.top..buf.top + area.h).enumerate() {
        let mut row: Row = Row::new(area.w);
        if y < buf.line_count() {
            let current: bool = y == cursor.line;
            row.pad_to(blank, fold);
            if numbers {
                let number: usize = match (o.number, o.relativenumber, current) {
                    (true, _, true) | (true, false, false) => y + 1,
                    _ => y.abs_diff(cursor.line),
                };
                let group: Group = if current { Group::CursorLineNr } else { Group::LineNr };
                row.text(t.on(group, ctx.base), &format!("{number:>number_width$}"));
            }
            if let Some(Some(sign)) = git.get(r) {
                let group: Group = match sign {
                    Sign::Add => Group::GitSignsAdd,
                    Sign::Change => Group::GitSignsChange,
                    Sign::Delete => Group::GitSignsDelete,
                };
                row.text(t.on(group, ctx.base), "\u{258e}");
            }
            row.pad_to(blank, gutter);
            // Only the shown part of a line (and a little past it, for tokens cut by the right edge) is scanned.
            let shown_chars: usize = text::col_at_display(buf.line(y), buf.left + area.w) + LOOKAHEAD;
            let shown: usize = text::byte_index(buf.line(y), shown_chars);
            let decor: Decor = Decor {
                groups: if o.syntax { buf.highlight_line(y, shown_chars) } else { None },
                shown,
                guide_to: if o.indentguides { guide_indent(buf, y) } else { 0 },
                indent_width: o.indent_width(),
                urls: if o.highlighturl { text::find_urls(&buf.line(y)[..shown]) } else { Vec::new() },
                words: word.as_deref().map(|w| text::find_word(&buf.line(y)[..shown], w)).unwrap_or_default(),
                parens: parens.map(|p| p.filter(|p| p.line == y).map(|p| p.col)),
                colors: if o.colorhighlight { text::find_hex_colors(&buf.line(y)[..shown]) } else { Vec::new() },
                todo: if o.todocomments { syntax::todo(&buf.line(y)[..shown]) } else { None },
            };
            draw_line(&mut row, buf, y, &decor, ctx);
        }
        grid.put(area.x, area.y + r, &row.finish(blank));
    }
    (cursor.line.saturating_sub(buf.top), (gutter + cursor_x).saturating_sub(buf.left), gutter)
}

/// Per-line extras drawn over the syntax colors.
struct Decor {
    groups: Option<Vec<Hl>>,
    /// Byte length of the line's scanned part.
    shown: usize,
    /// Indent guides are drawn in leading blanks left of this screen column.
    guide_to: usize,
    indent_width: usize,
    urls: Vec<Range<usize>>,
    words: Vec<Range<usize>>,
    parens: [Option<usize>; 2],
    colors: Vec<(Range<usize>, u32)>,
    todo: Option<(usize, usize, Todo)>,
}

/// todo-comments' colors per keyword kind.
fn todo_group(kind: Todo) -> Group {
    match kind {
        Todo::Fix => Group::TodoFix,
        Todo::Task => Group::TodoTodo,
        Todo::Hack => Group::TodoHack,
        Todo::Warn => Group::TodoWarn,
        Todo::Perf => Group::TodoPerf,
        Todo::Note => Group::TodoNote,
        Todo::Test => Group::TodoTest,
    }
}

/// Black or white, whichever reads better on `bg`.
fn contrast(bg: u32) -> Rgb {
    let [_, r, g, b]: [f32; 4] = bg.to_be_bytes().map(f32::from);
    if 0.299 * r + 0.587 * g + 0.114 * b > 140.0 { Rgb(0x000000) } else { Rgb(0xFFFFFF) }
}

/// `matchparen`: the bracket under the cursor and its partner, searched within a screenful.
fn matching_pair(buf: &Buffer, height: usize) -> [Option<Pos>; 2] {
    let at: Pos = buf.cursor;
    let Some(c): Option<char> = buf.line(at.line).chars().nth(at.col).filter(|c| "()[]{}".contains(*c)) else {
        return [None, None];
    };
    match buf.bracket_partner(at, c, height) {
        Some(partner) => [Some(at), Some(partner)],
        None => [None, None],
    }
}

/// The keyword under the cursor, highlighted elsewhere like LSP document highlights.
fn word_under_cursor(buf: &Buffer) -> Option<String> {
    let (start, end): (Pos, Pos) = buf.word_object(buf.cursor, false)?;
    (buf.class_at(start) == CharClass::Word).then(|| buf.text(start, end))
}

/// Screen columns of indentation to guide on line `y`; a blank line takes its neighbours' depth.
fn guide_indent(buf: &Buffer, y: usize) -> usize {
    const LOOK: usize = 100;
    let depth = |l: &str| text::display_col(l, text::char_len(text::indent(l)));
    let line: &str = buf.line(y);
    if !line.trim().is_empty() {
        return depth(line);
    }
    let lines: &[String] = buf.lines();
    let before: Option<&String> = lines[y.saturating_sub(LOOK)..y].iter().rev().find(|l| !l.trim().is_empty());
    let after: Option<&String> =
        lines.get(y + 1..(y + 1 + LOOK).min(lines.len())).and_then(|ls| ls.iter().find(|l| !l.trim().is_empty()));
    match (before, after) {
        (Some(b), Some(a)) => depth(b).min(depth(a)),
        _ => 0,
    }
}

fn scroll(buf: &mut Buffer, cursor_x: usize, height: usize, width: usize, o: &Options) {
    let line: usize = buf.cursor.line;
    let margin: usize = o.scrolloff.min(height.saturating_sub(1) / 2);
    if line < buf.top + margin {
        buf.top = line.saturating_sub(margin);
    } else if line + margin >= buf.top + height {
        buf.top = (line + margin + 1).saturating_sub(height).min(line);
    }
    let side: usize = o.sidescrolloff.min(width.saturating_sub(1) / 2);
    if cursor_x < buf.left + side {
        buf.left = cursor_x.saturating_sub(side);
    } else if cursor_x + side >= buf.left + width {
        buf.left = (cursor_x + side + 1).saturating_sub(width);
    }
}

/// Chars scanned past the right edge, so a word, URL, color or match cut by it is still recognized.
const LOOKAHEAD: usize = 256;

fn draw_line(row: &mut Row, buf: &Buffer, y: usize, decor: &Decor, ctx: &Context) {
    let t: Arc<Theme> = theme();
    let line: &str = &buf.line(y)[..decor.shown];
    let matches: Vec<Range<usize>> = text::find_all(line, &ctx.pattern);
    let cursor: Pos = buf.cursor;
    let bg: Rgb = match t.get(Group::CursorLine).bg {
        Some(cursorline) if y == cursor.line && ctx.options.cursorline => cursorline,
        _ => ctx.base,
    };
    let guide: Style = t.on(Group::IndentGuide, bg);
    let is_guide =
        |col: usize| decor.indent_width > 0 && col < decor.guide_to && col.is_multiple_of(decor.indent_width);
    let leading: usize = text::char_len(text::indent(line));
    let mut next_match: usize = 0;
    let mut screen_x: usize = 0;
    for (x, c) in line.chars().enumerate() {
        let cells: usize = if c == '\t' { text::tab_cells(screen_x) } else { text::char_width(c) };
        let end: usize = screen_x + cells;
        if end <= buf.left && cells > 0 {
            screen_x = end;
            continue;
        }
        while matches.get(next_match).is_some_and(|m| m.end <= x) {
            next_match += 1;
        }
        let hit: Option<&Range<usize>> = matches.get(next_match).filter(|m| m.contains(&x));
        let group: Hl = decor.groups.as_ref().and_then(|g| g.get(x)).copied().unwrap_or(Hl::Text);
        let mut style: Style = t.syntax(group, bg);
        if decor.urls.iter().any(|u| u.contains(&x)) {
            style.underline |= t.get(Group::Url).underline;
        }
        if decor.words.iter().any(|w| w.contains(&x))
            && let Some(word_bg) = t.get(Group::WordHighlight).bg
        {
            style.bg = word_bg;
        }
        // todo-comments only inside comments (or anywhere when there is no highlighter).
        if let Some((col, len, kind)) = decor.todo
            && x >= col.saturating_sub(1)
            && group_at(decor, col).is_none_or(|g| g == Hl::Comment)
        {
            let color: Rgb = t.fg(todo_group(kind));
            if x < col + len + 1 {
                style = Style { fg: ctx.base, bg: color, bold: true, ..style };
            } else {
                style.fg = color;
            }
        }
        if let Some((_, value)) = decor.colors.iter().find(|(r, _)| r.contains(&x)) {
            style = Style { fg: contrast(*value), bg: Rgb(*value), ..style };
        }
        if decor.parens.contains(&Some(x)) {
            let paren: Highlight = t.get(Group::MatchParen);
            style.fg = paren.fg.unwrap_or(style.fg);
            style.bold |= paren.bold;
        }
        if let Some(selection) = ctx.selection
            && selection.contains(Pos::new(y, x), screen_x)
        {
            style.bg = t.bg(Group::Visual);
        } else if hit.is_some() {
            style.bg = t.bg(Group::Search);
        }
        if hit.is_some_and(|m| y == cursor.line && m.contains(&cursor.col)) {
            style = t.style(Group::CurSearch);
        }
        if c == '\t' || c == ' ' || screen_x < buf.left {
            // Blanks (with indent guides), and wide chars cut by the left edge, cell by cell.
            for col in screen_x.max(buf.left)..end {
                let guided: bool = x < leading && is_guide(col) && c.is_whitespace();
                row.cell(
                    if guided { Style { bg: style.bg, ..guide } } else { style },
                    if guided { '\u{258f}' } else { ' ' },
                );
            }
        } else {
            row.cell(style, c);
        }
        screen_x = end;
        if row.is_full() {
            break;
        }
    }
    // Blank lines inside a block still show the block's guides.
    while screen_x < decor.guide_to && !row.is_full() {
        if screen_x >= buf.left {
            row.cell(
                if is_guide(screen_x) { guide } else { Style::new(guide.fg, bg) },
                if is_guide(screen_x) { '\u{258f}' } else { ' ' },
            );
        }
        screen_x += 1;
    }
    row.pad_to(Style::new(t.fg(Group::Normal), bg), usize::MAX);
}

fn group_at(decor: &Decor, col: usize) -> Option<Hl> {
    decor.groups.as_ref().and_then(|g| g.get(col)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_picks_readable_text() {
        assert_eq!(contrast(0xFFFFFF), Rgb(0x000000));
        assert_eq!(contrast(0xFFFF00), Rgb(0x000000));
        assert_eq!(contrast(0x000000), Rgb(0xFFFFFF));
        assert_eq!(contrast(0x0000FF), Rgb(0xFFFFFF));
    }
}
