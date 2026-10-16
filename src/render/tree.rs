//! neo-tree's file tree in a left-hand pane.

use super::row::{Grid, Row};
use super::theme::{Group, Rgb, Style, theme};
use crate::editor::{Editor, GitStatus, Rect};
use crate::syntax;

/// Draws the tree into `area`; returns the cursor cell when the tree has focus.
pub fn explorer(ed: &mut Editor, grid: &mut Grid, area: Rect) -> Option<(usize, usize)> {
    let t = theme();
    let set = super::icons::icons(&ed.options);
    let open: Vec<std::path::PathBuf> =
        ed.buffers.iter().filter_map(|b| b.path.as_ref().map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))).collect();
    let explorer = ed.explorer.as_mut()?;
    let base = t.style(Group::ExplorerNormal);
    grid.fill(area, base);
    let mut header = Row::new(area.w);
    let root = explorer.root.file_name().map_or_else(|| explorer.root.display().to_string(), |n| n.to_string_lossy().into_owned());
    header.text(t.on(Group::ExplorerRoot, base.bg), &format!(" {}{root}", super::icons::Icons::spaced(set.folder_open)));
    grid.put(area.x, area.y, &header.finish(base));

    let rows = area.h.saturating_sub(1);
    if explorer.cursor < explorer.scroll {
        explorer.scroll = explorer.cursor;
    } else if rows > 0 && explorer.cursor >= explorer.scroll + rows {
        explorer.scroll = explorer.cursor + 1 - rows;
    }
    let mut cursor = None;
    for (r, (i, entry)) in explorer.entries.iter().enumerate().skip(explorer.scroll).take(rows).enumerate() {
        let selected = i == explorer.cursor;
        let bg = if selected && explorer.focused { t.bg(Group::PmenuSel) } else { base.bg };
        let line = Style { bg, bold: selected && explorer.focused, ..base };
        let mut row = Row::new(area.w);
        row.text(line, " ");
        for depth in 0..entry.depth {
            let marker = if depth + 1 == entry.depth && entry.last { "\u{2514} " } else { "\u{2502} " };
            row.text(t.on(Group::ExplorerIndent, bg), marker);
        }
        let (icon, color) = if entry.dir {
            ((if entry.expanded { set.folder_open } else { set.folder_closed }).to_owned(), t.fg(Group::ExplorerDirectory))
        } else {
            let (icon, color) = set.for_file(syntax::lang_for(&entry.path));
            (icon, color.map_or(line.fg, Rgb))
        };
        if !icon.is_empty() {
            row.text(Style { fg: color, ..line }, &format!("{icon} "));
        }
        if selected {
            cursor = Some((area.y + 1 + r, area.x + row.used()));
        }
        let name_group = match entry.git {
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
            let symbol = match status {
                GitStatus::Modified => set.git_modified,
                GitStatus::Added => set.git_added,
                GitStatus::Untracked => set.git_untracked,
                GitStatus::Deleted => set.git_deleted,
            };
            row.pad_to(line, area.w.saturating_sub(2));
            row.text(Style { fg: t.fg(name_group), ..line }, symbol);
        }
        grid.put(area.x, area.y + 1 + r, &row.finish(line));
    }
    explorer.focused.then_some(cursor).flatten()
}
