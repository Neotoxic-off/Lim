//! Markdown preview (render-markdown.nvim / glow): the current Markdown buffer drawn as formatted text in a
//! panel on the right, following the cursor as you type. Headings, emphasis, inline code, links, lists and task
//! boxes, quotes, fenced code, tables and rules; paragraphs wrap to the panel.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, Theme, theme};
use crate::buffer::{Buffer, text};
use crate::editor::{Editor, Rect};
use crate::syntax::Kind;

/// How a piece of text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Text,
    /// `#` to `######`.
    Heading(u8),
    Bold,
    Italic,
    Code,
    Link,
    Quote,
    Marker,
    Rule,
    CodeBlock,
    Dim,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub look: Look,
}

/// One screen line of the preview and the source line it comes from (to follow the cursor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub spans: Vec<Span>,
    pub source: usize,
    /// Code blocks paint their background across the whole panel.
    pub block: bool,
}

fn span(text: impl Into<String>, look: Look) -> Span {
    Span { text: text.into(), look }
}

/// Whether the buffer is Markdown, so the preview has something to show.
pub fn is_markdown(buf: &Buffer) -> bool {
    buf.lang.is_some_and(|l| l.kind == Kind::Markdown)
}

/// Word-wraps `spans` into lines of `width` cells: the first starts with `first`, the rest with `rest`.
fn wrap(spans: Vec<Span>, width: usize, first: Vec<Span>, rest: &[Span], source: usize) -> Vec<Line> {
    let prefix_width = |p: &[Span]| p.iter().map(|s| text::str_width(&s.text)).sum::<usize>();
    let mut lines: Vec<Line> = Vec::new();
    let mut line: Vec<Span> = first;
    let mut used: usize = prefix_width(&line);
    let mut fresh: bool = true;
    // A word is the run of pieces between spaces, so `[link](…).` keeps its period; each piece keeps its look.
    let mut words: Vec<(bool, Vec<Span>)> = Vec::new();
    let mut space: bool = false;
    for s in spans {
        let mut parts = s.text.split(' ').peekable();
        while let Some(part) = parts.next() {
            if !part.is_empty() {
                match words.last_mut() {
                    Some((_, pieces)) if !space => pieces.push(span(part, s.look)),
                    _ => words.push((space, vec![span(part, s.look)])),
                }
                space = false;
            }
            if parts.peek().is_some() {
                space = true;
            }
        }
    }
    for (space, word) in words {
        let gap: usize = usize::from(space && !fresh);
        let w: usize = word.iter().map(|p| text::str_width(&p.text)).sum();
        if !fresh && used + gap + w > width {
            lines.push(Line { spans: std::mem::take(&mut line), source, block: false });
            line = rest.to_vec();
            used = prefix_width(rest);
            fresh = true;
        }
        if space && !fresh {
            line.push(span(" ", word[0].look));
            used += 1;
        }
        used += w;
        line.extend(word);
        fresh = false;
    }
    lines.push(Line { spans: line, source, block: false });
    lines
}

fn rule(width: usize, ch: &str, source: usize) -> Line {
    Line { spans: vec![span(ch.repeat(width), Look::Rule)], source, block: false }
}

/// A list item being read: its first line's prefix (indent and bullet) until used, then the hanging indent.
struct Item {
    first: Option<Vec<Span>>,
    rest: Vec<Span>,
}

/// A table being read: its rows of cells (the first is the header) and the source line of each row.
#[derive(Default)]
struct Table {
    rows: Vec<(usize, Vec<Vec<Span>>)>,
    cell: Vec<Span>,
}

/// The preview being built from pulldown-cmark's events.
struct Renderer {
    width: usize,
    out: Vec<Line>,
    /// Byte offset where each source line starts.
    starts: Vec<usize>,
    /// Inline text of the current block, and the look its text gets.
    spans: Vec<Span>,
    base: Look,
    bold: usize,
    italic: usize,
    dim: usize,
    link: usize,
    /// The alt text of an image being read.
    image: Option<String>,
    quote: usize,
    /// Each open list: its next number when ordered.
    lists: Vec<Option<u64>>,
    items: Vec<Item>,
    code: bool,
    table: Option<Table>,
    /// The source line where the current block starts, and where the last one ended.
    start: usize,
    end: Option<usize>,
}

impl Renderer {
    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&s| s <= offset).saturating_sub(1)
    }

    /// What text looks like now: struck-through wins over bold, bold over italic, italic over a link's look.
    fn look(&self) -> Look {
        match (self.dim, self.bold, self.italic, self.link) {
            (1.., _, _, _) => Look::Dim,
            (_, 1.., _, _) => Look::Bold,
            (_, _, 1.., _) => Look::Italic,
            (_, _, _, 1..) => Look::Link,
            _ => self.base,
        }
    }

    fn blank(&mut self) {
        if self.out.last().is_some_and(|l| !l.spans.is_empty()) {
            self.out.push(Line { spans: Vec::new(), source: self.start, block: false });
        }
    }

    /// A block starts at source line `line`: a blank line before it when the source has one (or `always`).
    fn open(&mut self, line: usize, always: bool) {
        self.start = line;
        if always || self.end.is_some_and(|end| line > end + 1) {
            self.blank();
        }
    }

    fn push_text(&mut self, text: &str, look: Look) {
        match (&mut self.image, &mut self.table) {
            (Some(alt), _) => alt.push_str(text),
            (None, Some(table)) => table.cell.push(span(text, look)),
            (None, None) => self.spans.push(span(text, look)),
        }
    }

    /// Wraps the inline text read so far, after a list bullet or a quote bar when inside one.
    fn flush(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        let bar: Vec<Span> = if self.quote > 0 { vec![span("▌ ", Look::Quote)] } else { Vec::new() };
        let (mut first, mut rest): (Vec<Span>, Vec<Span>) = match self.items.last_mut() {
            Some(item) => (item.first.take().unwrap_or_else(|| item.rest.clone()), item.rest.clone()),
            None => (Vec::new(), Vec::new()),
        };
        first.splice(0..0, bar.iter().cloned());
        rest.splice(0..0, bar);
        let spans: Vec<Span> = std::mem::take(&mut self.spans);
        self.out.extend(wrap(spans, self.width, first, &rest, self.start));
    }

    fn start(&mut self, tag: Tag<'_>, line: usize) {
        match tag {
            Tag::Paragraph => self.open(line, false),
            Tag::Heading { level, .. } => {
                self.open(line, true);
                self.base = Look::Heading(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
            }
            Tag::BlockQuote(_) => {
                self.open(line, false);
                self.quote += 1;
                self.base = Look::Quote;
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                self.open(line, true);
                self.code = true;
                if let CodeBlockKind::Fenced(info) = kind
                    && let Some(lang) = info.split_whitespace().next()
                {
                    self.out.push(Line { spans: vec![span(format!(" {lang}"), Look::Dim)], source: line, block: true });
                }
            }
            Tag::List(number) => {
                self.flush();
                self.lists.push(number);
            }
            Tag::Item => {
                self.open(line, false);
                let depth: usize = self.lists.len().saturating_sub(1);
                let pad: String = "  ".repeat(depth);
                let bullet: String = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{}.", *n - 1)
                    }
                    _ => ["•", "◦", "▪"][depth % 3].to_owned(),
                };
                let hang: String = format!("{pad}{}", " ".repeat(text::str_width(&bullet) + 1));
                let first: Vec<Span> = vec![span(pad, Look::Text), span(format!("{bullet} "), Look::Marker)];
                self.items.push(Item { first: Some(first), rest: vec![span(hang, Look::Text)] });
            }
            Tag::Table(_) => {
                self.open(line, true);
                self.table = Some(Table::default());
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push((line, Vec::new()));
                }
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.dim += 1,
            Tag::Link { .. } => self.link += 1,
            Tag::Image { .. } => self.image = Some(String::new()),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(level) => {
                self.flush();
                match level {
                    HeadingLevel::H1 => self.out.push(rule(self.width, "━", self.start)),
                    HeadingLevel::H2 => self.out.push(rule(self.width, "─", self.start)),
                    _ => {}
                }
                self.base = Look::Text;
            }
            TagEnd::BlockQuote(_) => {
                self.quote = self.quote.saturating_sub(1);
                self.base = if self.quote > 0 { Look::Quote } else { Look::Text };
            }
            TagEnd::CodeBlock => {
                self.code = false;
                self.blank();
            }
            TagEnd::List(_) => {
                self.lists.pop();
            }
            TagEnd::Item => {
                self.flush();
                self.items.pop();
            }
            TagEnd::TableCell => {
                if let Some(table) = &mut self.table {
                    let cell: Vec<Span> = std::mem::take(&mut table.cell);
                    if let Some((_, row)) = table.rows.last_mut() {
                        row.push(cell);
                    }
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.out.extend(table_lines(table.rows));
                }
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.dim = self.dim.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            TagEnd::Image => {
                if let Some(alt) = self.image.take() {
                    self.push_text(&format!("▣ {alt}"), Look::Dim);
                }
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        let line: usize = self.line_of(range.start);
        let last: usize = self.line_of(range.end.saturating_sub(1).max(range.start));
        // Where content last ended (a block's own range can take in the blank lines after it).
        let content: bool =
            matches!(event, Event::Text(_) | Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) | Event::Rule);
        match event {
            Event::Start(tag) => self.start(tag, line),
            Event::End(tag) => self.end(tag),
            Event::Text(t) if self.code => {
                for (i, code) in t.lines().enumerate() {
                    let code: String = code.replace('\t', "    ");
                    let source: usize = line + i;
                    self.out.push(Line { spans: vec![span(format!(" {code}"), Look::CodeBlock)], source, block: true });
                }
            }
            Event::Text(t) => self.push_text(&t, self.look()),
            Event::Code(t) => self.push_text(&t, Look::Code),
            Event::SoftBreak | Event::HardBreak => self.push_text(" ", self.look()),
            Event::TaskListMarker(done) => {
                if let Some(item) = self.items.last_mut() {
                    let pad: String =
                        item.first.as_ref().and_then(|f| f.first()).map(|s| s.text.clone()).unwrap_or_default();
                    item.first =
                        Some(vec![span(pad.clone(), Look::Text), span(if done { "☑ " } else { "☐ " }, Look::Marker)]);
                    item.rest = vec![span(format!("{pad}  "), Look::Text)];
                }
            }
            Event::Rule => {
                self.open(line, false);
                self.out.push(rule(self.width, "─", line));
            }
            Event::Html(t) => {
                self.open(line, false);
                self.out.push(Line { spans: vec![span(t.trim_end(), Look::Dim)], source: line, block: false });
            }
            Event::InlineHtml(t) => self.push_text(&t, Look::Dim),
            _ => {}
        }
        if content {
            self.end = Some(last);
        }
    }
}

/// A table with box borders: column widths fit the widest cell, and the header row is bold.
fn table_lines(rows: Vec<(usize, Vec<Vec<Span>>)>) -> Vec<Line> {
    let cell_width = |cell: &[Span]| cell.iter().map(|s| text::str_width(&s.text)).sum::<usize>();
    let columns: usize = rows.iter().map(|(_, cells)| cells.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|c| rows.iter().map(|(_, cells)| cells.get(c).map_or(0, |cell| cell_width(cell))).max().unwrap_or(0))
        .collect();
    let start: usize = rows.first().map_or(0, |(line, _)| *line);
    let border = |left: &str, mid: &str, right: &str| -> Line {
        let body: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        Line { spans: vec![span(format!("{left}{}{right}", body.join(mid)), Look::Rule)], source: start, block: false }
    };
    let mut out: Vec<Line> = vec![border("┌", "┬", "┐")];
    for (r, (source, cells)) in rows.into_iter().enumerate() {
        let mut spans: Vec<Span> = vec![span("│", Look::Rule)];
        for (c, w) in widths.iter().enumerate() {
            let cell: &[Span] = cells.get(c).map_or(&[], Vec::as_slice);
            let header = |look: Look| if r == 0 && look == Look::Text { Look::Bold } else { look };
            let pad: Look = header(Look::Text);
            spans.push(span(" ", pad));
            spans.extend(cell.iter().map(|s| span(s.text.clone(), header(s.look))));
            spans.push(span(" ".repeat(w - cell_width(cell) + 1), pad));
            spans.push(span("│", Look::Rule));
        }
        out.push(Line { spans, source, block: false });
        if r == 0 {
            out.push(border("├", "┼", "┤"));
        }
    }
    out.push(border("└", "┴", "┘"));
    out
}

/// The whole document as preview lines `width` cells wide.
pub fn render(lines: &[String], width: usize) -> Vec<Line> {
    let source: String = lines.join("\n");
    let mut starts: Vec<usize> = vec![0];
    starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
    let mut r: Renderer = Renderer {
        width: width.max(10),
        out: Vec::new(),
        starts,
        spans: Vec::new(),
        base: Look::Text,
        bold: 0,
        italic: 0,
        dim: 0,
        link: 0,
        image: None,
        quote: 0,
        lists: Vec::new(),
        items: Vec::new(),
        code: false,
        table: None,
        start: 0,
        end: None,
    };
    let options: Options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for (event, range) in Parser::new_ext(&source, options).into_offset_iter() {
        r.event(event, range);
    }
    r.flush();
    while r.out.last().is_some_and(|l| l.spans.is_empty()) {
        r.out.pop();
    }
    r.out
}

fn style_of(t: &Theme, look: Look, bg: Rgb) -> Style {
    let on = |group: Group| t.on(group, bg);
    match look {
        Look::Text | Look::CodeBlock => on(Group::Normal),
        Look::Heading(1) => Style { bold: true, ..on(Group::Function) },
        Look::Heading(2) => Style { bold: true, ..on(Group::Keyword) },
        Look::Heading(3) => Style { bold: true, ..on(Group::Type) },
        Look::Heading(_) => Style { bold: true, ..on(Group::String) },
        Look::Bold => Style { bold: true, ..on(Group::Normal) },
        Look::Italic => Style { italic: true, ..on(Group::Normal) },
        Look::Code => Style { bg: t.bg(Group::CursorLine), ..on(Group::String) },
        Look::Link => Style { underline: true, ..on(Group::Function) },
        Look::Quote => Style { italic: true, ..on(Group::Comment) },
        Look::Marker => on(Group::Directory),
        Look::Rule => on(Group::WinSeparator),
        Look::Dim => Style { italic: false, ..on(Group::Comment) },
    }
}

/// (buffer revision, width) and the lines rendered for them.
type Cached = ((u64, usize), Rc<Vec<Line>>);

/// The preview of the current buffer in `area`, scrolled so the cursor's line sits a third of the way down.
pub fn preview(ed: &Editor, grid: &mut Grid, area: Rect) {
    thread_local! {
        static CACHE: RefCell<Option<Cached>> = const { RefCell::new(None) };
    }
    let buf: &Buffer = ed.buf();
    let width: usize = area.w.saturating_sub(2);
    let key: (u64, usize) = (buf.revision(), width);
    let lines: Rc<Vec<Line>> = CACHE.with(|cache| {
        if let Some((cached, lines)) = &*cache.borrow()
            && *cached == key
        {
            return Rc::clone(lines);
        }
        let lines: Rc<Vec<Line>> = Rc::new(render(buf.lines(), width));
        *cache.borrow_mut() = Some((key, Rc::clone(&lines)));
        lines
    });
    let t: Arc<Theme> = theme();
    let base: Style = t.style(Group::Normal);
    grid.fill(area, base);
    let at: usize = lines.iter().position(|l| l.source >= buf.cursor.line).unwrap_or(lines.len());
    let top: usize = at.saturating_sub(area.h / 3).min(lines.len().saturating_sub(area.h));
    let code_bg: Rgb = t.bg(Group::CursorLine);
    for (y, line) in lines.iter().skip(top).take(area.h).enumerate() {
        let bg: Rgb = if line.block { code_bg } else { base.bg };
        let plain: Style = Style { bg, ..base };
        let mut row: Row = Row::new(area.w);
        row.text(plain, " ");
        for s in &line.spans {
            row.text(style_of(&t, s.look, bg), &s.text);
        }
        grid.put(area.x, area.y + y, &row.finish(plain));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>().trim_end().to_owned())
            .collect()
    }

    fn doc(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn blocks() {
        let md: Vec<String> = doc(
            "# Title\nSome **bold** and `code` with a [link](http://x).\n\n- one\n- [x] done\n  1. nested\n\n> quoted\n\n```rs\nfn x() {}\n```\n\n| a | bb |\n|---|---|\n| 1 | 2 |\n\n---",
        );
        let out: Vec<Line> = render(&md, 30);
        let rule1: String = "━".repeat(30);
        let rule2: String = "─".repeat(30);
        let want: Vec<&str> = vec![
            "Title",
            &rule1,
            "Some bold and code with a",
            "link.",
            "",
            "• one",
            "☑ done",
            "  1. nested",
            "",
            "▌ quoted",
            "",
            " rs",
            " fn x() {}",
            "",
            "┌───┬────┐",
            "│ a │ bb │",
            "├───┼────┤",
            "│ 1 │ 2  │",
            "└───┴────┘",
            "",
            &rule2,
        ];
        assert_eq!(plain(&out), want);
        assert!(out[2].spans.iter().any(|s| s.text == "bold" && s.look == Look::Bold));
        assert!(out[2].spans.iter().any(|s| s.text == "code" && s.look == Look::Code));
        assert!(out[3].spans.iter().any(|s| s.text == "link" && s.look == Look::Link));
        assert!(out.iter().any(|l| l.block), "the code block paints its background");
    }

    #[test]
    fn inline_rules() {
        // The parser may cut a run of text into several events: neighbours with the same look are joined.
        let looks = |s: &str| {
            render(&doc(s), 80).remove(0).spans.into_iter().fold(Vec::<(String, Look)>::new(), |mut runs, s| {
                match runs.last_mut() {
                    Some((text, look)) if *look == s.look => text.push_str(&s.text),
                    _ => runs.push((s.text, s.look)),
                }
                runs
            })
        };
        assert_eq!(looks("snake_case_name"), vec![("snake_case_name".into(), Look::Text)]);
        assert_eq!(looks("*it*"), vec![("it".into(), Look::Italic)]);
        assert_eq!(looks("\\*not\\*"), vec![("*not*".into(), Look::Text)]);
        assert_eq!(looks("![logo](a.png)"), vec![("▣ logo".into(), Look::Dim)]);
    }

    #[test]
    fn wraps_with_hanging_indent() {
        let out: Vec<Line> = render(&doc("- alpha beta gamma delta"), 14);
        assert_eq!(plain(&out), ["• alpha beta", "  gamma delta"]);
    }
}
