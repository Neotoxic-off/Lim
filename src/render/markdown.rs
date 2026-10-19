//! Markdown preview (render-markdown.nvim / glow): the current Markdown buffer drawn as formatted text in a
//! panel on the right, following the cursor as you type. Headings, emphasis, inline code, links, lists and task
//! boxes, quotes, fenced code, tables and rules; paragraphs wrap to the panel.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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

/// `**bold**`, `*italic*` / `_italic_`, `` `code` ``, `[text](url)`, `![alt](src)`, `~~gone~~`, `\*` escapes.
fn inline(source: &str, base: Look) -> Vec<Span> {
    let chars: Vec<char> = source.chars().collect();
    let mut text: Inline =
        Inline { base, spans: Vec::new(), current: String::new(), bold: false, italic: false, dim: false };
    let mut i: usize = 0;
    while let Some(&c) = chars.get(i) {
        let next: Option<char> = chars.get(i + 1).copied();
        // A code span or a link takes several chars; an unclosed one is plain text.
        let span: Option<usize> = match c {
            '`' => code_span(&chars, i, &mut text),
            '!' | '[' => link(&chars, i, &mut text),
            _ => None,
        };
        if let Some(len) = span {
            i += len;
            continue;
        }
        i += match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                text.current.push(next.unwrap_or(c));
                2
            }
            '*' | '_' if next == Some(c) => text.toggle(|t| &mut t.bold, 2),
            '~' if next == Some('~') => text.toggle(|t| &mut t.dim, 2),
            // `_` inside a word (snake_case) is not emphasis.
            '*' | '_' if c == '*' || i == 0 || !chars[i - 1].is_alphanumeric() || text.italic => {
                text.toggle(|t| &mut t.italic, 1)
            }
            _ => {
                text.current.push(c);
                1
            }
        };
    }
    text.flush();
    text.spans
}

/// The spans read so far, the text not yet in one, and the emphasis it is in.
struct Inline {
    base: Look,
    spans: Vec<Span>,
    current: String,
    bold: bool,
    italic: bool,
    dim: bool,
}

impl Inline {
    /// What the current text looks like: struck-through wins over bold, bold over italic.
    fn look(&self) -> Look {
        match (self.dim, self.bold, self.italic) {
            (true, _, _) => Look::Dim,
            (_, true, _) => Look::Bold,
            (_, _, true) => Look::Italic,
            _ => self.base,
        }
    }

    /// Ends the text in the current emphasis as a span.
    fn flush(&mut self) {
        if !self.current.is_empty() {
            let look: Look = self.look();
            self.spans.push(span(std::mem::take(&mut self.current), look));
        }
    }

    /// A `**`, `~~` or `*` marker: the text so far keeps its emphasis, the emphasis flips. Returns `len`, the
    /// marker's length.
    fn toggle(&mut self, emphasis: fn(&mut Self) -> &mut bool, len: usize) -> usize {
        self.flush();
        let flag: &mut bool = emphasis(self);
        *flag = !*flag;
        len
    }
}

/// `` `code` `` at `i`: a code span, and the chars it took; None when the backtick is not closed.
fn code_span(chars: &[char], i: usize, text: &mut Inline) -> Option<usize> {
    let len: usize = chars[i + 1..].iter().position(|&x| x == '`')?;
    text.flush();
    text.spans.push(span(chars[i + 1..i + 1 + len].iter().collect::<String>(), Look::Code));
    Some(len + 2)
}

/// `[text](url)` (the text, styled as a link) or `![alt](src)` (a dimmed placeholder) at `i`, and the chars it
/// took; None when the brackets do not make one.
fn link(chars: &[char], i: usize, text: &mut Inline) -> Option<usize> {
    let image: bool = chars[i] == '!';
    let open: usize = if image { i + 1 } else { i };
    if chars.get(open) != Some(&'[') {
        return None;
    }
    let close: usize = open + chars[open..].iter().position(|&x| x == ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end: usize = close + chars[close..].iter().position(|&x| x == ')')?;
    text.flush();
    let label: String = chars[open + 1..close].iter().collect();
    if image {
        text.spans.push(span(format!("▣ {label}"), Look::Dim));
    } else {
        text.spans.extend(inline(&label, Look::Link));
    }
    Some(end + 1 - i)
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

/// `- item`, `* item`, `+ item`, `1. item`, `2) item`: (indent, marker as written, text after it).
fn list_item(line: &str) -> Option<(usize, &str, &str)> {
    let indent: usize = line.len() - line.trim_start().len();
    let t: &str = line.trim_start();
    let marker_len: usize = if t.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits: usize = t.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !matches!(t.as_bytes().get(digits), Some(b'.' | b')')) {
            return None;
        }
        digits + 1
    };
    let rest: &str = t[marker_len..].strip_prefix(' ')?;
    Some((indent, &t[..marker_len], rest))
}

fn is_rule(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3 && ['-', '*', '_'].iter().any(|&m| compact.chars().all(|c| c == m))
}

fn heading(t: &str) -> Option<(u8, &str)> {
    let level: usize = t.bytes().take_while(|&b| b == b'#').count();
    let rest: &str = t[level..].strip_prefix(' ').or_else(|| (level == t.len()).then_some(""))?;
    (1..=6).contains(&level).then(|| (u8::try_from(level).unwrap_or(6), rest.trim_end_matches(['#', ' '])))
}

fn table_cells(line: &str) -> Vec<String> {
    let t: &str = line.trim();
    let t: &str = t.strip_prefix('|').unwrap_or(t);
    let t: &str = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_table_separator(line: &str) -> bool {
    let cells: Vec<String> = table_cells(line);
    !cells.is_empty() && cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':')))
}

/// Starts a block other than a paragraph (so a paragraph stops before it).
fn starts_block(line: &str) -> bool {
    let t: &str = line.trim_start();
    t.is_empty()
        || heading(t).is_some()
        || t.starts_with("```")
        || t.starts_with("~~~")
        || t.starts_with('>')
        || t.starts_with('|')
        || is_rule(t)
        || list_item(line).is_some()
}

/// The whole document as preview lines `width` cells wide.
#[expect(clippy::too_many_lines, reason = "one arm per Markdown block")]
pub fn render(lines: &[String], width: usize) -> Vec<Line> {
    let width: usize = width.max(10);
    let mut out: Vec<Line> = Vec::new();
    let blank = |out: &mut Vec<Line>, source: usize| {
        if out.last().is_some_and(|l: &Line| !l.spans.is_empty()) {
            out.push(Line { spans: Vec::new(), source, block: false });
        }
    };
    let mut i: usize = 0;
    while i < lines.len() {
        let line: &str = &lines[i];
        let t: &str = line.trim();
        if t.is_empty() {
            blank(&mut out, i);
            i += 1;
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            let fence: &str = &t[..3];
            let lang: &str = t[3..].trim();
            blank(&mut out, i);
            if !lang.is_empty() {
                out.push(Line { spans: vec![span(format!(" {lang}"), Look::Dim)], source: i, block: true });
            }
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                let code: String = lines[i].replace('\t', "    ");
                out.push(Line { spans: vec![span(format!(" {code}"), Look::CodeBlock)], source: i, block: true });
                i += 1;
            }
            i += 1;
            blank(&mut out, i.min(lines.len()) - 1);
            continue;
        }
        if let Some((level, title)) = heading(t) {
            blank(&mut out, i);
            out.extend(wrap(inline(title, Look::Heading(level)), width, Vec::new(), &[], i));
            match level {
                1 => out.push(rule(width, "━", i)),
                2 => out.push(rule(width, "─", i)),
                _ => {}
            }
            i += 1;
            continue;
        }
        if is_rule(t) {
            out.push(rule(width, "─", i));
            i += 1;
            continue;
        }
        if t.starts_with('|') && lines.get(i + 1).is_some_and(|l| is_table_separator(l)) {
            let start: usize = i;
            let mut rows: Vec<Vec<String>> = vec![table_cells(t)];
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                rows.push(table_cells(&lines[i]));
                i += 1;
            }
            let columns: usize = rows.iter().map(Vec::len).max().unwrap_or(0);
            let cell_text = |c: &str| inline(c, Look::Text).into_iter().map(|s| s.text).collect::<String>();
            let widths: Vec<usize> = (0..columns)
                .map(|c| rows.iter().map(|r| r.get(c).map_or(0, |s| text::str_width(&cell_text(s)))).max().unwrap_or(0))
                .collect();
            let border = |left: &str, mid: &str, right: &str| -> Line {
                let body: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
                Line {
                    spans: vec![span(format!("{left}{}{right}", body.join(mid)), Look::Rule)],
                    source: start,
                    block: false,
                }
            };
            blank(&mut out, start);
            out.push(border("┌", "┬", "┐"));
            for (r, row) in rows.iter().enumerate() {
                let mut spans: Vec<Span> = vec![span("│", Look::Rule)];
                for (c, w) in widths.iter().enumerate() {
                    let cell: &str = row.get(c).map_or("", String::as_str);
                    let look: Look = if r == 0 { Look::Bold } else { Look::Text };
                    spans.push(span(" ", look));
                    let inner: Vec<Span> = inline(cell, look);
                    let used: usize = inner.iter().map(|s| text::str_width(&s.text)).sum();
                    spans.extend(inner);
                    spans.push(span(" ".repeat(w - used + 1), look));
                    spans.push(span("│", Look::Rule));
                }
                out.push(Line { spans, source: start + r + usize::from(r > 0), block: false });
                if r == 0 {
                    out.push(border("├", "┼", "┤"));
                }
            }
            out.push(border("└", "┴", "┘"));
            continue;
        }
        if t.starts_with('>') {
            let start: usize = i;
            let mut quote: Vec<String> = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let body: &str = lines[i].trim_start()[1..].trim_start();
                quote.push(body.to_owned());
                i += 1;
            }
            let bar: Vec<Span> = vec![span("▌ ", Look::Quote)];
            out.extend(wrap(inline(&quote.join(" "), Look::Quote), width, bar.clone(), &bar, start));
            continue;
        }
        if let Some((indent, marker, rest)) = list_item(line) {
            let depth: usize = indent / 2;
            let pad: String = "  ".repeat(depth);
            let (bullet, rest): (String, &str) = if let Some(after) = rest.strip_prefix("[ ] ") {
                ("☐".into(), after)
            } else if let Some(after) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
                ("☑".into(), after)
            } else if marker.ends_with(['.', ')']) {
                (marker.to_owned(), rest)
            } else {
                (["•", "◦", "▪"][depth % 3].to_owned(), rest)
            };
            let start: usize = i;
            let mut body: String = rest.to_owned();
            i += 1;
            // Lazy continuation: following lines that start no block belong to the item.
            while i < lines.len() && !starts_block(&lines[i]) {
                body.push(' ');
                body.push_str(lines[i].trim());
                i += 1;
            }
            let hang: usize = text::str_width(&bullet) + 1;
            let first: Vec<Span> = vec![span(pad.clone(), Look::Text), span(format!("{bullet} "), Look::Marker)];
            let rest_prefix: Vec<Span> = vec![span(format!("{pad}{}", " ".repeat(hang)), Look::Text)];
            out.extend(wrap(inline(&body, Look::Text), width, first, &rest_prefix, start));
            continue;
        }
        if t.starts_with('<') {
            out.push(Line { spans: vec![span(t, Look::Dim)], source: i, block: false });
            i += 1;
            continue;
        }
        let start: usize = i;
        let mut paragraph: Vec<&str> = vec![t];
        i += 1;
        while i < lines.len() && !starts_block(&lines[i]) {
            paragraph.push(lines[i].trim());
            i += 1;
        }
        out.extend(wrap(inline(&paragraph.join(" "), Look::Text), width, Vec::new(), &[], start));
    }
    while out.last().is_some_and(|l| l.spans.is_empty()) {
        out.pop();
    }
    out
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
        let looks = |s: &str| inline(s, Look::Text).into_iter().map(|s| (s.text, s.look)).collect::<Vec<_>>();
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
