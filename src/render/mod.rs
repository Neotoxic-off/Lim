//! Screen composition: tabline, text, statusline, command line. Only changed rows are rewritten.

mod bars;
mod row;
mod theme;
mod view;

use std::fmt::Write;
use std::path::PathBuf;
use std::{env, fs};

use crate::buffer::text;
use crate::editor::{Editor, Mode, Rect};
use row::Row;
use theme::{BASE, SPLIT, Style};
use crate::term::{ColorDepth, Size};

/// Workspace facts shown in the statusline, read once at startup.
pub struct Context {
    pub branch: Option<String>,
    pub cwd: String,
}

impl Context {
    fn detect() -> Self {
        let dir = env::current_dir().ok();
        let cwd = dir.as_deref().and_then(|d| d.file_name()).map_or_else(|| "/".into(), |n| n.to_string_lossy().into());
        Self { branch: dir.and_then(git_branch), cwd }
    }
}

fn git_branch(mut dir: PathBuf) -> Option<String> {
    loop {
        if let Ok(head) = fs::read_to_string(dir.join(".git/HEAD")) {
            let head = head.trim();
            return Some(head.strip_prefix("ref: refs/heads/").map_or_else(|| head.chars().take(7).collect(), str::to_owned));
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub struct Screen {
    depth: ColorDepth,
    ctx: Context,
    size: Size,
    rows: Vec<String>,
}

impl Screen {
    pub fn new(depth: ColorDepth) -> Self {
        Self { depth, ctx: Context::detect(), size: Size::default(), rows: Vec::new() }
    }

    /// Every window and split border inside `area`, as full screen rows, plus the cursor cell.
    fn windows(&self, ed: &mut Editor, area: Rect) -> (Vec<String>, Option<(usize, usize)>) {
        ed.area = area;
        let mut rects = Vec::new();
        ed.layout.rects(area, &mut rects);
        let mut separators = Vec::new();
        ed.layout.separators(area, &mut separators);
        // Each screen row is assembled from (x, rendered segment) pieces sorted left to right.
        let mut pieces: Vec<Vec<(usize, String)>> = vec![Vec::new(); area.h];
        let mut cursor = None;
        for (id, rect) in rects {
            let Some(index) = ed.windows.iter().position(|w| w.id == id) else { continue };
            if index == ed.active {
                ed.view_height = rect.h.saturating_sub(1).max(1);
            }
            let drawn = view::window(ed, index, rect, self.depth);
            cursor = cursor.or(drawn.cursor);
            for (i, row) in drawn.rows.into_iter().enumerate() {
                if let Some(line) = pieces.get_mut(rect.y + i - area.y) {
                    line.push((rect.x, row));
                }
            }
        }
        let border = Style::new(SPLIT, BASE);
        for sep in separators {
            if sep.vertical {
                for r in sep.y..sep.y + sep.len {
                    let mut cell = Row::new(self.depth, 1);
                    cell.text(border, "\u{2502}");
                    if let Some(line) = pieces.get_mut(r - area.y) {
                        line.push((sep.x, cell.finish(border)));
                    }
                }
            } else if let Some(line) = pieces.get_mut(sep.y - area.y) {
                let mut row = Row::new(self.depth, sep.len);
                row.text(border, &"\u{2500}".repeat(sep.len));
                line.push((sep.x, row.finish(border)));
            }
        }
        let rows = pieces
            .into_iter()
            .map(|mut line| {
                line.sort_by_key(|(x, _)| *x);
                line.into_iter().map(|(_, s)| s).collect()
            })
            .collect();
        (rows, cursor)
    }

    /// Forces the next frame to repaint everything.
    pub fn invalidate(&mut self) {
        self.size = Size::default();
        self.rows.clear();
    }

    /// Escape sequences taking the terminal from the previous frame to this one.
    pub fn draw(&mut self, ed: &mut Editor, size: Size) -> String {
        let mut out = String::from("\x1b[?25l");
        if size != self.size {
            self.size = size;
            self.rows.clear();
            out.push_str("\x1b[0m\x1b[2J");
        }
        let (width, height) = (size.cols.max(1), size.rows.max(1));
        let text_height = height.saturating_sub(3).max(1);

        let mut frame = Vec::with_capacity(text_height + 3);
        frame.push(bars::tabline(ed, width, self.depth));
        let area = Rect { x: 0, y: 1, w: width, h: text_height };
        let (rows, cursor) = self.windows(ed, area);
        frame.extend(rows);
        frame.push(bars::statusline(ed, &self.ctx, width, self.depth));
        frame.push(bars::cmdline(ed, width, self.depth));
        frame.truncate(height);
        let (row, col) = cursor.unwrap_or((1, 0));

        for (i, line) in frame.iter().enumerate() {
            if self.rows.get(i) != Some(line) {
                let _ = write!(out, "\x1b[{};1H{line}", i + 1);
            }
        }
        self.rows = frame;

        let (row, col, shape) = match ed.mode {
            Mode::Command | Mode::Search => (height - 1, 1 + text::str_width(&ed.cmdline), 6),
            Mode::Insert => (row, col, 6),
            _ => (row, col, 2),
        };
        let _ = write!(out, "\x1b[{};{}H\x1b[{shape} q\x1b[?25h", row.min(height - 1) + 1, col.min(width - 1) + 1);
        out
    }
}
