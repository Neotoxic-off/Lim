//! Buffer list and window management: AstroNvim's `<Leader>b…`, `]b`/`[b`, `>b`/`<b`,
//! `|`/`\` splits and `<C-h/j/k/l>` navigation.

use std::path::Path;

use super::window::{self, Direction, Layout, View, Window};
use super::{Editor, Mode, PickAction};
use crate::buffer::Buffer;
use crate::error::Error;
use crate::term::Key;

/// Which buffers a bulk close targets, relative to the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseSet {
    Others,
    All,
    Left,
    Right,
}

impl Editor {
    pub(super) fn open(&mut self, path: &Path) {
        if let Some(i) = self.buffers.iter().position(|b| b.path.as_deref() == Some(path)) {
            return self.switch_to(i);
        }
        match Buffer::open(path) {
            Ok(buf) => {
                self.buffers.push(buf);
                self.switch_to(self.buffers.len() - 1);
            }
            Err(e) => self.error(e),
        }
    }

    pub(super) fn new_buffer(&mut self) {
        self.buffers.push(Buffer::scratch());
        self.switch_to(self.buffers.len() - 1);
    }

    /// Shows buffer `i` in the active window, remembering the one left for `<Leader>bp`.
    pub(super) fn switch_to(&mut self, i: usize) {
        if i < self.buffers.len() && i != self.current {
            self.previous_buffer = Some(self.current);
            self.current = i;
        }
    }

    pub(super) fn cycle(&mut self, delta: isize) {
        let n = self.buffers.len() as isize;
        self.switch_to((self.current as isize + delta).rem_euclid(n) as usize);
    }

    pub(super) fn go_previous_buffer(&mut self) {
        if let Some(i) = self.previous_buffer {
            self.switch_to(i);
        }
    }

    pub(super) fn close_buffer(&mut self, force: bool) -> Result<(), Error> {
        self.remove_buffer(self.current, force)
    }

    /// Removes buffer `i`; windows showing it fall back to the previous buffer.
    fn remove_buffer(&mut self, i: usize, force: bool) -> Result<(), Error> {
        let Some(buf) = self.buffers.get(i) else { return Ok(()) };
        if !force && buf.is_dirty() {
            return Err(Error::Unsaved(buf.name()));
        }
        let fallback = self.previous_buffer.filter(|&p| p != i && p < self.buffers.len());
        self.buffers.remove(i);
        if self.buffers.is_empty() {
            self.buffers.push(Buffer::scratch());
        }
        let last = self.buffers.len() - 1;
        let shift = |b: usize| if b > i { b - 1 } else { b.min(last) };
        let replacement = fallback.map_or(i.min(last), shift);
        let fix = |b: usize| if b == i { replacement } else { shift(b) };
        self.current = fix(self.current);
        for window in &mut self.windows {
            window.buffer = fix(window.buffer);
        }
        self.previous_buffer = self.previous_buffer.filter(|&p| p != i).map(shift);
        Ok(())
    }

    /// `<Leader>bc`, `bC`, `bl`, `br`: closes the matching unmodified buffers.
    pub(super) fn close_buffers(&mut self, set: CloseSet) {
        let current = self.current;
        let targets: Vec<usize> = (0..self.buffers.len())
            .filter(|&i| match set {
                CloseSet::Others => i != current,
                CloseSet::All => true,
                CloseSet::Left => i < current,
                CloseSet::Right => i > current,
            })
            .collect();
        let mut kept = 0;
        for &i in targets.iter().rev() {
            if self.remove_buffer(i, false).is_err() {
                kept += 1;
            }
        }
        if kept > 0 {
            self.info(format!("{kept} modified buffer(s) kept open"));
        }
    }

    /// `>b` / `<b`: moves the current buffer along the tabline.
    pub(super) fn move_buffer(&mut self, delta: isize) {
        let from = self.current;
        let Some(to) = from.checked_add_signed(delta).filter(|&t| t < self.buffers.len()) else { return };
        self.buffers.swap(from, to);
        let swap = |b: usize| if b == from { to } else if b == to { from } else { b };
        for window in &mut self.windows {
            window.buffer = swap(window.buffer);
        }
        self.previous_buffer = self.previous_buffer.map(swap);
        self.current = to;
    }

    /// Tabline labels for the buffer picker: the file's first letter when free, else the next free one.
    pub fn pick_labels(&self) -> Vec<char> {
        let mut used: Vec<char> = Vec::with_capacity(self.buffers.len());
        for buf in &self.buffers {
            let first = buf.name().chars().next().map(|c| c.to_ascii_lowercase());
            let label = first
                .filter(|c| c.is_ascii_lowercase() && !used.contains(c))
                .or_else(|| ('a'..='z').chain('0'..='9').find(|c| !used.contains(c)))
                .unwrap_or(' ');
            used.push(label);
        }
        used
    }

    pub(super) fn pick(&mut self, key: Key, action: PickAction) {
        self.mode = Mode::Normal;
        let Key::Char(c) = key else { return };
        let Some(i) = self.pick_labels().iter().position(|&l| l == c && l != ' ') else { return };
        match action {
            PickAction::Go => self.switch_to(i),
            PickAction::Close => {
                if let Err(e) = self.remove_buffer(i, false) {
                    self.error(e);
                }
            }
            PickAction::SplitBelow | PickAction::SplitRight => {
                self.split(action == PickAction::SplitRight);
                self.switch_to(i);
            }
        }
    }

    /// `|` / `\` / `:vsplit` / `:split`: opens a window on the same buffer and focuses it.
    pub(super) fn split(&mut self, vertical: bool) {
        let Some(active_id) = self.windows.get(self.active).map(|w| w.id) else { return };
        let view = View::of(self.buf());
        let window = Window::new(self.next_window_id, self.current, view);
        self.next_window_id += 1;
        if self.layout.insert_beside(active_id, window.id, vertical) {
            self.windows.push(window);
            self.focus(self.windows.len() - 1);
        }
    }

    /// Closes the focused window; false when it is the last one.
    pub(super) fn close_window(&mut self) -> bool {
        if self.windows.len() < 2 {
            return false;
        }
        let Some(id) = self.windows.get(self.active).map(|w| w.id) else { return false };
        self.layout.remove(id);
        self.windows.remove(self.active);
        self.active = self.active.saturating_sub(1).min(self.windows.len() - 1);
        self.load_active_view();
        true
    }

    /// `<C-w>o` / `:only`.
    pub(super) fn only_window(&mut self) {
        let Some(window) = self.windows.get(self.active).cloned() else { return };
        self.layout = Layout::Leaf(window.id);
        self.windows = vec![window];
        self.active = 0;
    }

    pub(super) fn focus(&mut self, index: usize) {
        if index == self.active || index >= self.windows.len() {
            return;
        }
        let view = View::of(self.buf());
        if let Some(window) = self.windows.get_mut(self.active) {
            window.view = view;
            window.buffer = self.current;
        }
        self.active = index;
        self.load_active_view();
    }

    fn load_active_view(&mut self) {
        let Some(window) = self.windows.get(self.active).cloned() else { return };
        self.current = window.buffer.min(self.buffers.len() - 1);
        let buf = self.buf_mut();
        window.view.apply(buf);
        buf.clamp(false);
    }

    /// `<C-h/j/k/l>`: focuses the window in `dir`, measured from the cursor's screen row.
    pub(super) fn focus_direction(&mut self, dir: Direction) {
        let mut rects = Vec::new();
        self.layout.rects(self.area, &mut rects);
        let Some(id) = self.windows.get(self.active).map(|w| w.id) else { return };
        let Some(&(_, from)) = rects.iter().find(|(i, _)| *i == id) else { return };
        let buf = self.buf();
        let row = from.y + 1 + buf.cursor.line.saturating_sub(buf.top);
        let row = row.min(from.y + from.h.saturating_sub(1));
        if let Some(target) = window::neighbor(&rects, from, row, from.x, dir)
            && let Some(index) = self.windows.iter().position(|w| w.id == target)
        {
            self.focus(index);
        }
    }

    /// `<C-w>w`: next window in layout order.
    pub(super) fn focus_next_window(&mut self) {
        let mut rects = Vec::new();
        self.layout.rects(self.area, &mut rects);
        let Some(id) = self.windows.get(self.active).map(|w| w.id) else { return };
        let Some(pos) = rects.iter().position(|(i, _)| *i == id) else { return };
        let (next, _) = rects[(pos + 1) % rects.len()];
        if let Some(index) = self.windows.iter().position(|w| w.id == next) {
            self.focus(index);
        }
    }

    /// Runs `f` on window `index`'s buffer with that window's view loaded, then restores the active view.
    pub fn with_view<R>(&mut self, index: usize, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
        if index == self.active {
            return Some(f(self.buf_mut()));
        }
        let window = self.windows.get(index)?.clone();
        let buf = self.buffers.get_mut(window.buffer)?;
        let saved = View::of(buf);
        window.view.apply(buf);
        buf.clamp(false);
        let result = f(buf);
        let after = View::of(buf);
        saved.apply(buf);
        if let Some(w) = self.windows.get_mut(index) {
            w.view = after;
        }
        Some(result)
    }

    /// After an internal error: windows and layout agree and point at real buffers.
    pub(super) fn repair_windows(&mut self) {
        let mut ids = Vec::new();
        self.layout.rects(self.area, &mut ids);
        let last = self.buffers.len() - 1;
        self.windows.retain(|w| ids.iter().any(|(id, _)| *id == w.id));
        if self.windows.is_empty() {
            self.windows.push(Window::new(self.next_window_id, self.current, View::default()));
            self.next_window_id += 1;
        }
        for window in &mut self.windows {
            window.buffer = window.buffer.min(last);
        }
        if self.windows.len() == 1 || ids.len() != self.windows.len() {
            self.windows.truncate(1);
            self.layout = Layout::Leaf(self.windows[0].id);
        }
        self.active = self.active.min(self.windows.len() - 1);
        self.previous_buffer = self.previous_buffer.filter(|&p| p <= last);
    }
}
