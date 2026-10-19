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
        // A popup is bare, as zen mode: no numbers, signs or winbar.
        options: if ed.zen || ed.windows.get(index).is_some_and(|w| w.floating) {
            zen_options(&ed.options)
        } else {
            ed.options.clone()
        },
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
    let o: &Options = &ctx.options;
    let gutter: Gutter = Gutter::of(buf, o);
    let cursor: Pos = buf.cursor;
    let cursor_x: usize = text::display_col(buf.line(cursor.line), cursor.col);
    scroll(buf, cursor_x, area.h.max(1), area.w.saturating_sub(gutter.width).max(1), o);
    let bottom: usize = (buf.top + area.h).min(buf.line_count());
    let git: Vec<Option<Sign>> = if o.gitsigns && o.signcolumn {
        let top: usize = buf.top;
        buf.git_signs().get(top..bottom).map(<[_]>::to_vec).unwrap_or_default()
    } else {
        Vec::new()
    };
    // What the whole window highlights: the bracket pair and the word under the cursor.
    let parens: [Option<Pos>; 2] = if o.matchparen && ctx.active { matching_pair(buf, area.h) } else { [None, None] };
    let word: Option<String> =
        if o.wordhighlight && ctx.active && ctx.normal_mode { word_under_cursor(buf) } else { None };
    let t: Arc<Theme> = theme();
    let blank: Style = Style::new(t.fg(Group::Normal), ctx.base);
    for (r, y) in (buf.top..buf.top + area.h).enumerate() {
        let mut row: Row = Row::new(area.w);
        if y < buf.line_count() {
            gutter.draw(&t, &mut row, (y, cursor.line), git.get(r).copied().flatten(), ctx);
            let decor: Decor = decor_for(buf, y, buf.left + area.w, o, word.as_deref(), parens);
            draw_line(&mut row, buf, y, &decor, ctx);
        }
        grid.put(area.x, area.y + r, &row.finish(blank));
    }
    (cursor.line.saturating_sub(buf.top), (gutter.width + cursor_x).saturating_sub(buf.left), gutter.width)
}

/// `AstroNvim`'s statuscolumn: fold space, line numbers, then the sign column.
struct Gutter {
    fold: usize,
    /// 0 without numbers.
    number_width: usize,
    width: usize,
}

impl Gutter {
    fn of(buf: &Buffer, o: &Options) -> Self {
        let numbers: bool = o.number || o.relativenumber;
        let number_width: usize = if numbers { buf.line_count().to_string().len().max(3) } else { 0 };
        let signs: usize = if o.signcolumn { 2 } else { usize::from(numbers) };
        let fold: usize = usize::from(o.foldcolumn);
        Self { fold, number_width, width: fold + number_width + signs }
    }

    /// Line `y`'s number (absolute, or relative to the cursor's) and its gitsigns mark.
    fn draw(&self, t: &Theme, row: &mut Row, (y, cursor): (usize, usize), sign: Option<Sign>, ctx: &Context) {
        let o: &Options = &ctx.options;
        let blank: Style = Style::new(t.fg(Group::Normal), ctx.base);
        row.pad_to(blank, self.fold);
        if self.number_width > 0 {
            let current: bool = y == cursor;
            let number: usize = match (o.number, o.relativenumber, current) {
                (true, _, true) | (true, false, false) => y + 1,
                _ => y.abs_diff(cursor),
            };
            let group: Group = if current { Group::CursorLineNr } else { Group::LineNr };
            let width: usize = self.number_width;
            row.text(t.on(group, ctx.base), &format!("{number:>width$}"));
        }
        if let Some(sign) = sign {
            let group: Group = match sign {
                Sign::Add => Group::GitSignsAdd,
                Sign::Change => Group::GitSignsChange,
                Sign::Delete => Group::GitSignsDelete,
            };
            row.text(t.on(group, ctx.base), "\u{258e}");
        }
        row.pad_to(blank, self.width);
    }
}

/// Line `y`'s highlights over the syntax colors, found in the part shown left of screen column `right_edge` (and a
/// little past it, for tokens cut by the edge): a long line is not scanned to its end.
fn decor_for(
    buf: &mut Buffer,
    y: usize,
    right_edge: usize,
    o: &Options,
    word: Option<&str>,
    parens: [Option<Pos>; 2],
) -> Decor {
    let shown_chars: usize = text::col_at_display(buf.line(y), right_edge) + LOOKAHEAD;
    let shown: usize = text::byte_index(buf.line(y), shown_chars);
    let groups: Option<Vec<Hl>> = if o.syntax { buf.highlight_line(y, shown_chars) } else { None };
    let line: &str = &buf.line(y)[..shown];
    Decor {
        groups,
        shown,
        guide_to: if o.indentguides { guide_indent(buf, y) } else { 0 },
        indent_width: o.indent_width(),
        urls: if o.highlighturl { text::find_urls(line) } else { Vec::new() },
        words: word.map(|w| text::find_word(line, w)).unwrap_or_default(),
        parens: parens.map(|p| p.filter(|p| p.line == y).map(|p| p.col)),
        colors: if o.colorhighlight { text::find_hex_colors(line) } else { Vec::new() },
        todo: if o.todocomments { syntax::todo(line) } else { None },
    }
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

/// The indent guide's glyph.
const GUIDE: char = '\u{258f}';

fn draw_line(row: &mut Row, buf: &Buffer, y: usize, decor: &Decor, ctx: &Context) {
    let line: &str = &buf.line(y)[..decor.shown];
    let mut paint: LinePaint<'_> = LinePaint::new(buf, y, decor, ctx, text::find_all(line, &ctx.pattern));
    let leading: usize = text::char_len(text::indent(line));
    let mut screen_x: usize = 0;
    for (x, c) in line.chars().enumerate() {
        let cells: usize = if c == '\t' { text::tab_cells(screen_x) } else { text::char_width(c) };
        let end: usize = screen_x + cells;
        // Scrolled off to the left.
        if end <= buf.left && cells > 0 {
            screen_x = end;
            continue;
        }
        let style: Style = paint.style_at(x, screen_x);
        if c == '\t' || c == ' ' || screen_x < buf.left {
            paint.blanks(row, style, screen_x.max(buf.left)..end, x < leading && c.is_whitespace());
        } else {
            row.cell(style, c);
        }
        screen_x = end;
        if row.is_full() {
            break;
        }
    }
    paint.guides_past_text(row, screen_x);
    row.pad_to(Style::new(paint.t.fg(Group::Normal), paint.bg), usize::MAX);
}

/// What drawing one line needs, worked out once for the line.
struct LinePaint<'a> {
    t: Arc<Theme>,
    buf: &'a Buffer,
    y: usize,
    decor: &'a Decor,
    ctx: &'a Context,
    /// The line's background: the cursorline's, or the window's.
    bg: Rgb,
    guide: Style,
    /// Search matches, and the first one not yet passed.
    matches: Vec<Range<usize>>,
    next_match: usize,
}

impl<'a> LinePaint<'a> {
    fn new(buf: &'a Buffer, y: usize, decor: &'a Decor, ctx: &'a Context, matches: Vec<Range<usize>>) -> Self {
        let t: Arc<Theme> = theme();
        let bg: Rgb = match t.get(Group::CursorLine).bg {
            Some(cursorline) if y == buf.cursor.line && ctx.options.cursorline => cursorline,
            _ => ctx.base,
        };
        let guide: Style = t.on(Group::IndentGuide, bg);
        Self { t, buf, y, decor, ctx, bg, guide, matches, next_match: 0 }
    }

    /// The style of char `x` (at screen column `screen_x`): its syntax color, then each highlight drawn over it.
    #[inline]
    fn style_at(&mut self, x: usize, screen_x: usize) -> Style {
        let group: Hl = group_at(self.decor, x).unwrap_or(Hl::Text);
        let mut style: Style = self.t.syntax(group, self.bg);
        self.url(&mut style, x);
        self.word(&mut style, x);
        self.todo(&mut style, x);
        self.hex_color(&mut style, x);
        self.paren(&mut style, x);
        self.selection_or_search(&mut style, x, screen_x);
        style
    }

    /// `highlighturl`: underlined.
    #[inline]
    fn url(&self, style: &mut Style, x: usize) {
        if self.decor.urls.iter().any(|u| u.contains(&x)) {
            style.underline |= self.t.get(Group::Url).underline;
        }
    }

    /// The other places of the word under the cursor, as LSP document highlights.
    #[inline]
    fn word(&self, style: &mut Style, x: usize) {
        if self.decor.words.iter().any(|w| w.contains(&x))
            && let Some(word_bg) = self.t.get(Group::WordHighlight).bg
        {
            style.bg = word_bg;
        }
    }

    /// todo-comments: the keyword on its color, the rest of the comment in it. Only inside comments, or anywhere
    /// when there is no highlighter.
    #[inline]
    fn todo(&self, style: &mut Style, x: usize) {
        let Some((col, len, kind)): Option<(usize, usize, Todo)> = self.decor.todo else { return };
        if x + 1 < col || group_at(self.decor, col).is_some_and(|g| g != Hl::Comment) {
            return;
        }
        let color: Rgb = self.t.fg(todo_group(kind));
        if x < col + len + 1 {
            *style = Style { fg: self.ctx.base, bg: color, bold: true, ..*style };
        } else {
            style.fg = color;
        }
    }

    /// `#RRGGBB` drawn on its own color.
    #[inline]
    fn hex_color(&self, style: &mut Style, x: usize) {
        if let Some((_, value)) = self.decor.colors.iter().find(|(r, _)| r.contains(&x)) {
            *style = Style { fg: contrast(*value), bg: Rgb(*value), ..*style };
        }
    }

    /// `matchparen`: the bracket under the cursor and its partner.
    #[inline]
    fn paren(&self, style: &mut Style, x: usize) {
        if self.decor.parens.contains(&Some(x)) {
            let paren: Highlight = self.t.get(Group::MatchParen);
            style.fg = paren.fg.unwrap_or(style.fg);
            style.bold |= paren.bold;
        }
    }

    /// The visual selection, else a search match; the match under the cursor stands out from the others.
    #[inline]
    fn selection_or_search(&mut self, style: &mut Style, x: usize, screen_x: usize) {
        while self.matches.get(self.next_match).is_some_and(|m| m.end <= x) {
            self.next_match += 1;
        }
        let hit: Option<&Range<usize>> = self.matches.get(self.next_match).filter(|m| m.contains(&x));
        if let Some(selection) = self.ctx.selection
            && selection.contains(Pos::new(self.y, x), screen_x)
        {
            style.bg = self.t.bg(Group::Visual);
        } else if hit.is_some() {
            style.bg = self.t.bg(Group::Search);
        }
        let cursor: Pos = self.buf.cursor;
        if hit.is_some_and(|m| self.y == cursor.line && m.contains(&cursor.col)) {
            *style = self.t.style(Group::CurSearch);
        }
    }

    #[inline]
    fn is_guide(&self, col: usize) -> bool {
        let width: usize = self.decor.indent_width;
        width > 0 && col < self.decor.guide_to && col.is_multiple_of(width)
    }

    /// Blank cells (a tab, a space, a wide char cut by the left edge), with indent guides in the indentation.
    #[inline]
    fn blanks(&self, row: &mut Row, style: Style, cols: Range<usize>, indentation: bool) {
        for col in cols {
            if indentation && self.is_guide(col) {
                row.cell(Style { bg: style.bg, ..self.guide }, GUIDE);
            } else {
                row.cell(style, ' ');
            }
        }
    }

    /// A blank line inside a block still shows the block's guides, up to its depth.
    fn guides_past_text(&self, row: &mut Row, mut screen_x: usize) {
        while screen_x < self.decor.guide_to && !row.is_full() {
            if screen_x >= self.buf.left {
                if self.is_guide(screen_x) {
                    row.cell(self.guide, GUIDE);
                } else {
                    row.cell(Style::new(self.guide.fg, self.bg), ' ');
                }
            }
            screen_x += 1;
        }
    }
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
