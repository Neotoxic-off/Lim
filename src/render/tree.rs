//! `NvChad`'s nvim-tree in a left-hand pane.

use std::path::PathBuf;
use std::sync::Arc;

use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::editor::{Editor, Explorer, GitStatus, Rect};
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
    let rows: usize = area.h;
    if explorer.cursor < explorer.scroll {
        explorer.scroll = explorer.cursor;
    } else if rows > 0 && explorer.cursor >= explorer.scroll + rows {
        explorer.scroll = explorer.cursor + 1 - rows;
    }
    let mut cursor: Option<(usize, usize)> = None;
    for (r, (i, entry)) in explorer.entries.iter().enumerate().skip(explorer.scroll).take(rows).enumerate() {
        let selected: bool = i == explorer.cursor;
        // NvimTreeCursorLine: the editor's background, no bold.
        let bg: Rgb = if selected && explorer.focused { t.bg(Group::Normal) } else { base.bg };
        let line: Style = Style { bg, ..base };
        let mut row: Row = Row::new(area.w);
        row.text(line, " ");
        for depth in 0..entry.depth {
            let marker: &str = if depth + 1 == entry.depth && entry.last { "\u{2514} " } else { "\u{2502} " };
            row.text(t.on(Group::ExplorerIndent, bg), marker);
        }
        let (icon, color): (String, Rgb) = if entry.dir {
            (
                (if entry.expanded { set.folder_open } else { set.folder_closed }).to_owned(),
                t.fg(Group::ExplorerDirectory),
            )
        } else {
            let (icon, color): (String, Option<u32>) = set.for_file(syntax::lang_for(&entry.path));
            (icon, color.map_or(line.fg, Rgb))
        };
        if !icon.is_empty() {
            row.text(Style { fg: color, ..line }, &format!("{icon} "));
        }
        if selected {
            cursor = Some((area.y + r, area.x + row.used()));
        }
        let name_group: Group = match entry.git {
            Some(GitStatus::Modified) => Group::ExplorerGitModified,
            Some(GitStatus::Added) => Group::ExplorerGitAdded,
            Some(GitStatus::Untracked) => Group::ExplorerGitUntracked,
            Some(GitStatus::Deleted) => Group::ExplorerGitDeleted,
            None if entry.dir => Group::Directory,
            None if open.contains(&entry.path) => Group::ExplorerOpened,
            None => Group::ExplorerFile,
        };
        row.text(Style { fg: t.fg(name_group), ..line }, &entry.name);
        if let Some(status) = entry.git.filter(|_| !entry.dir) {
            let symbol: &str = match status {
                GitStatus::Modified => set.git_modified,
                GitStatus::Added => set.git_added,
                GitStatus::Untracked => set.git_untracked,
                GitStatus::Deleted => set.git_deleted,
            };
            row.pad_to(line, area.w.saturating_sub(2));
            row.text(Style { fg: t.fg(name_group), ..line }, symbol);
        }
        grid.put(area.x, area.y + r, &row.finish(line));
    }
    explorer.focused.then_some(cursor).flatten()
}
