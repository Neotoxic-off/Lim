//! `NvChad`'s nvim-tree in a left-hand pane.

use std::path::PathBuf;
use std::sync::Arc;

use super::row::{Cell, Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::editor::{Editor, Entry, Explorer, GitStatus, Rect};
use crate::render::Theme;
use crate::render::icons::Icons;
use crate::syntax;

/// Draws the tree into `area`; returns the cursor cell when the tree has focus.
pub fn explorer(ed: &mut Editor, grid: &mut Grid, area: Rect) -> Option<(usize, usize)> {
    let t: Arc<Theme> = theme();
    let set: &Icons = super::icons::icons(&ed.options);
    let open: Vec<PathBuf> =
        ed.buffers.iter().filter_map(|b| b.path.as_deref().map(crate::buffer::canonical)).collect();
    let explorer: &mut Explorer = ed.explorer.as_mut()?;
    let base: Style = t.style(Group::ExplorerNormal);
    grid.fill(area, base);
    // No root line, as NvChad's `root_folder_label = false`: the statusline shows the folder.
    scroll_into_view(explorer, area.h);
    let mut cursor: Option<(usize, usize)> = None;
    for (r, (i, entry)) in explorer.entries.iter().enumerate().skip(explorer.scroll).take(area.h).enumerate() {
        let selected: bool = i == explorer.cursor;
        // NvimTreeCursorLine: the editor's background, no bold.
        let bg: Rgb = if selected && explorer.focused { t.bg(Group::Normal) } else { base.bg };
        let (row, name_col): (Vec<Cell>, usize) = entry_row(&t, set, entry, &open, Style { bg, ..base }, area.w);
        if selected {
            cursor = Some((area.y + r, area.x + name_col));
        }
        grid.put(area.x, area.y + r, &row);
    }
    explorer.focused.then_some(cursor).flatten()
}

/// Scrolls the tree just enough to show its cursor in `rows` rows.
fn scroll_into_view(explorer: &mut Explorer, rows: usize) {
    if explorer.cursor < explorer.scroll {
        explorer.scroll = explorer.cursor;
    } else if rows > 0 && explorer.cursor >= explorer.scroll + rows {
        explorer.scroll = explorer.cursor + 1 - rows;
    }
}

/// One entry: indent markers, icon, name in its git or open color, and its git symbol at the right edge.
/// Returns the cells and the column where the name starts (the cursor's place).
fn entry_row(t: &Theme, set: &Icons, entry: &Entry, open: &[PathBuf], line: Style, width: usize) -> (Vec<Cell>, usize) {
    let mut row: Row = Row::new(width);
    row.text(line, " ");
    for depth in 0..entry.depth {
        let marker: &str = if depth + 1 == entry.depth && entry.last { "\u{2514} " } else { "\u{2502} " };
        row.text(t.on(Group::ExplorerIndent, line.bg), marker);
    }
    let (icon, color): (String, Rgb) = entry_icon(t, set, entry, line.fg);
    if !icon.is_empty() {
        row.text(Style { fg: color, ..line }, &format!("{icon} "));
    }
    let name_col: usize = row.used();
    let name: Style = Style { fg: t.fg(name_group(entry, open)), ..line };
    row.text(name, &entry.name);
    if let Some(status) = entry.git.filter(|_| !entry.dir) {
        row.pad_to(line, width.saturating_sub(2));
        row.text(name, git_symbol(set, status));
    }
    (row.finish(line), name_col)
}

/// A folder's icon (open or closed) in the directory color, or a file's devicon in its language's color.
fn entry_icon(t: &Theme, set: &Icons, entry: &Entry, text_fg: Rgb) -> (String, Rgb) {
    if entry.dir {
        let icon: &str = if entry.expanded { set.folder_open } else { set.folder_closed };
        return (icon.to_owned(), t.fg(Group::ExplorerDirectory));
    }
    let (icon, color): (String, Option<u32>) = set.for_file(syntax::lang_for(&entry.path));
    (icon, color.map_or(text_fg, Rgb))
}

/// The name's color: its git status first, then folder, open file or plain file.
fn name_group(entry: &Entry, open: &[PathBuf]) -> Group {
    match entry.git {
        Some(GitStatus::Modified) => Group::ExplorerGitModified,
        Some(GitStatus::Added) => Group::ExplorerGitAdded,
        Some(GitStatus::Untracked) => Group::ExplorerGitUntracked,
        Some(GitStatus::Deleted) => Group::ExplorerGitDeleted,
        None if entry.dir => Group::Directory,
        None if open.contains(&entry.path) => Group::ExplorerOpened,
        None => Group::ExplorerFile,
    }
}

fn git_symbol(set: &Icons, status: GitStatus) -> &'static str {
    match status {
        GitStatus::Modified => set.git_modified,
        GitStatus::Added => set.git_added,
        GitStatus::Untracked => set.git_untracked,
        GitStatus::Deleted => set.git_deleted,
    }
}
