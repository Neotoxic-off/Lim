//! Undo/redo as groups of line-range changes, so an edit costs the lines it touches, not the file.

use super::Pos;

/// Undo replaces the `new_len` lines at `at` with `old`.
#[derive(Debug)]
pub struct Change {
    pub at: usize,
    pub old: Vec<String>,
    pub new_len: usize,
    pub cursor: Pos,
}

#[derive(Debug)]
struct Group {
    id: u64,
    changes: Vec<Change>,
}

/// Vim's default `undolevels`: older steps are dropped so memory stays bounded.
const UNDO_LEVELS: usize = 1000;

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Group>,
    redo: Vec<Group>,
    next_id: u64,
    sealed: bool,
}

impl History {
    /// Ends the current group: the next change starts a new undo step.
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Identifies the current text state; equal ids mean equal text.
    pub fn state(&self) -> u64 {
        self.undo.last().map_or(0, |g| g.id)
    }

    pub fn record(&mut self, change: Change) {
        self.redo.clear();
        if self.sealed || self.undo.is_empty() {
            self.next_id += 1;
            self.undo.push(Group { id: self.next_id, changes: Vec::new() });
            if self.undo.len() > UNDO_LEVELS {
                self.undo.remove(0);
            }
            self.sealed = false;
        }
        let Some(group) = self.undo.last_mut() else { return };
        // Typing on one line: the first snapshot of that line already reverts the whole run.
        if let Some(last) = group.changes.last()
            && last.at == change.at
            && last.new_len == 1
            && change.new_len == 1
            && change.old.len() == 1
        {
            return;
        }
        group.changes.push(change);
    }

    /// Reverts the latest group; returns where the cursor was before it.
    pub fn undo(&mut self, lines: &mut Vec<String>) -> Option<Pos> {
        self.sealed = true;
        swap(&mut self.undo, &mut self.redo, lines)
    }

    pub fn redo(&mut self, lines: &mut Vec<String>) -> Option<Pos> {
        self.sealed = true;
        swap(&mut self.redo, &mut self.undo, lines)
    }
}

fn swap(from: &mut Vec<Group>, to: &mut Vec<Group>, lines: &mut Vec<String>) -> Option<Pos> {
    let group = from.pop()?;
    let mut inverse = Vec::with_capacity(group.changes.len());
    let mut cursor = None;
    for change in group.changes.into_iter().rev() {
        let new_len = change.old.len();
        let old = lines.splice(change.at..change.at + change.new_len, change.old).collect();
        inverse.push(Change { at: change.at, old, new_len, cursor: change.cursor });
        cursor = Some(change.cursor);
    }
    to.push(Group { id: group.id, changes: inverse });
    cursor
}
