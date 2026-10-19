//! Buffer list and window management: `AstroNvim`'s `<Leader>b…`, `]b`/`[b`, `>b`/`<b`,
//! `|`/`\` splits and `<C-h/j/k/l>` navigation.

use std::path::Path;

use super::window::{self, Direction, Layout, View, Window};
use super::{Editor, Mode, PickAction, Rect};
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
            Ok(mut buf) => {
                if self.options.gitsigns {
                    buf.load_git_base();
                }
                self.remember(path);
                // The first file opened replaces the untouched start buffer, as in Vim.
                if self.buffers.len() == 1 && self.buffers[0].path.is_none() && !self.buffers[0].is_dirty() {
                    self.buffers.clear();
                    self.current = 0;
                    self.previous_buffer = None;
                    for window in &mut self.windows {
                        window.buffer = 0;
                    }
                    self.buffers.push(buf);
                    return;
                }
                self.buffers.push(buf);
                self.switch_to(self.buffers.len() - 1);
            }
            Err(e) => self.error(&e),
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

    /// `]b` / `[b`, `<Tab>` / `<S-Tab>`: `count` buffers forward or back, wrapping around.
    pub(super) fn cycle(&mut self, forward: bool, count: usize) {
        let len: usize = self.buffers.len();
        let step: usize = count % len;
        self.switch_to(if forward { self.current + step } else { self.current + len - step } % len);
    }

    /// `:b N` (as numbered in the tabline, from 1) or `:b part-of-name`.
    pub(super) fn switch_to_named(&mut self, name: &str) -> Result<(), Error> {
        if name.is_empty() {
            return Ok(());
        }
        if let Ok(n) = name.parse::<usize>() {
            if n == 0 || n > self.buffers.len() {
                return Err(Error::Vim(format!("E86: Buffer {n} does not exist")));
            }
            self.switch_to(n - 1);
            return Ok(());
        }
        let matching: Vec<usize> = (0..self.buffers.len())
            .filter(|&i| {
                let buf: &Buffer = &self.buffers[i];
                buf.name().contains(name) || buf.path.as_ref().is_some_and(|p| p.to_string_lossy().contains(name))
            })
            .collect();
        match matching[..] {
            [i] => {
                self.switch_to(i);
                Ok(())
            }
            [] => Err(Error::Vim(format!("E94: No matching buffer for {name}"))),
            _ => Err(Error::Vim(format!("E93: More than one match for {name}"))),
        }
    }

    pub(super) fn go_previous_buffer(&mut self) {
        if let Some(i) = self.previous_buffer {
            self.switch_to(i);
        }
    }

    pub(super) fn close_buffer(&mut self, force: bool) -> Result<(), Error> {
        self.remove_buffer(self.current, force)
    }

    /// Removes buffer `i`; windows showing it fall back to the previous buffer, except a tool's, which close.
    pub(super) fn remove_buffer(&mut self, i: usize, force: bool) -> Result<(), Error> {
        let Some(buf): Option<&Buffer> = self.buffers.get(i) else { return Ok(()) };
        if !force && buf.is_dirty() {
            return Err(Error::Unsaved(buf.name()));
        }
        // A tool's window was split for it alone: it goes with the buffer, unless it is the last window.
        if buf.throwaway {
            while self.windows.len() > 1
                && let Some(window) = self.windows.iter().position(|w| w.buffer == i)
            {
                self.focus(window);
                self.close_window();
            }
        }
        let fallback: Option<usize> = self.previous_buffer.filter(|&p| p != i && p < self.buffers.len());
        self.buffers.remove(i);
        if self.buffers.is_empty() {
            self.buffers.push(Buffer::scratch());
        }
        let last: usize = self.buffers.len() - 1;
        let shift = |b: usize| if b > i { b - 1 } else { b.min(last) };
        let replacement: usize = fallback.map_or(i.min(last), shift);
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
        let current: usize = self.current;
        let targets: Vec<usize> = (0..self.buffers.len())
            .filter(|&i| match set {
                CloseSet::Others => i != current,
                CloseSet::All => true,
                CloseSet::Left => i < current,
                CloseSet::Right => i > current,
            })
            .collect();
        let mut kept: i32 = 0;
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
        let from: usize = self.current;
        let Some(to): Option<usize> = from.checked_add_signed(delta).filter(|&t| t < self.buffers.len()) else {
            return;
        };
        self.buffers.swap(from, to);
        let swap = |b: usize| {
            if b == from {
                to
            } else if b == to {
                from
            } else {
                b
            }
        };
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
            let first: Option<char> = buf.name().chars().next().map(|c| c.to_ascii_lowercase());
            let label: char = first
                .filter(|c| c.is_ascii_lowercase() && !used.contains(c))
                .or_else(|| ('a'..='z').chain('0'..='9').find(|c| !used.contains(c)))
                .unwrap_or(' ');
            used.push(label);
        }
        used
    }

    pub(super) fn pick(&mut self, key: Key, action: PickAction) {
        self.mode = Mode::Normal;
        let Key::Char(c): Key = key else { return };
        let Some(i): Option<usize> = self.pick_labels().iter().position(|&l| l == c && l != ' ') else { return };
        match action {
            PickAction::Go => self.switch_to(i),
            PickAction::Close => {
                if let Err(e) = self.remove_buffer(i, false) {
                    self.error(&e);
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
        let Some(active_id): Option<window::WindowId> = self.windows.get(self.active).map(|w| w.id) else { return };
        let view: View = View::of(self.buf());
        let window: Window = Window::new(self.next_window_id, self.current, view);
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
        let Some(id): Option<window::WindowId> = self.windows.get(self.active).map(|w| w.id) else { return false };
        self.layout.remove(id);
        self.windows.remove(self.active);
        self.active = self.active.saturating_sub(1).min(self.windows.len() - 1);
        self.load_active_view();
        true
    }

    /// `<C-w>o` / `:only`.
    pub(super) fn only_window(&mut self) {
        let Some(window): Option<Window> = self.windows.get(self.active).cloned() else { return };
        self.layout = Layout::Leaf(window.id);
        self.windows = vec![window];
        self.active = 0;
    }

    pub(super) fn focus(&mut self, index: usize) {
        if index == self.active || index >= self.windows.len() {
            return;
        }
        let view: View = View::of(self.buf());
        if let Some(window) = self.windows.get_mut(self.active) {
            window.view = view;
            window.buffer = self.current;
        }
        self.active = index;
        self.load_active_view();
    }

    fn load_active_view(&mut self) {
        let Some(window): Option<Window> = self.windows.get(self.active).cloned() else { return };
        self.current = window.buffer.min(self.buffers.len() - 1);
        let buf: &mut Buffer = self.buf_mut();
        window.view.apply(buf);
        buf.clamp(false);
    }

    /// `<C-h/j/k/l>`: focuses the window in `dir`, measured from the cursor's screen row.
    pub(super) fn focus_direction(&mut self, dir: Direction) {
        let mut rects: Vec<(window::WindowId, Rect)> = Vec::new();
        self.layout.rects(self.area, &mut rects);
        let Some(id): Option<window::WindowId> = self.windows.get(self.active).map(|w| w.id) else { return };
        let Some(&(_, from)): Option<&(window::WindowId, Rect)> = rects.iter().find(|(i, _)| *i == id) else {
            return;
        };
        // `<C-h>` from the leftmost window goes into the open file tree.
        if dir == Direction::Left
            && let Some(explorer) = &mut self.explorer
            && !rects.iter().any(|(_, r)| r.x + r.w <= from.x)
        {
            explorer.focused = true;
            self.mode = Mode::Explorer;
            return;
        }
        let buf: &Buffer = self.buf();
        let row: usize = from.y + 1 + buf.cursor.line.saturating_sub(buf.top);
        let row: usize = row.min(from.y + from.h.saturating_sub(1));
        if let Some(target) = window::neighbor(&rects, from, row, from.x, dir)
            && let Some(index) = self.windows.iter().position(|w| w.id == target)
        {
            self.focus(index);
        }
    }

    /// `<C-w>w` from the file tree: back to the first window.
    pub(super) fn leave_tree_to_first_window(&mut self) {
        if let Some(explorer) = &mut self.explorer {
            explorer.focused = false;
        }
        self.mode = Mode::Normal;
        let mut rects: Vec<(window::WindowId, Rect)> = Vec::new();
        self.layout.rects(self.area, &mut rects);
        if let Some(index) = rects.first().and_then(|(id, _)| self.windows.iter().position(|w| w.id == *id)) {
            self.focus(index);
        }
    }

    /// `<C-w>w`: next window in layout order.
    pub(super) fn focus_next_window(&mut self) {
        let mut rects: Vec<(window::WindowId, Rect)> = Vec::new();
        self.layout.rects(self.area, &mut rects);
        let Some(id): Option<window::WindowId> = self.windows.get(self.active).map(|w| w.id) else { return };
        let Some(pos): Option<usize> = rects.iter().position(|(i, _)| *i == id) else { return };
        // As Vim with neo-tree: after the last window comes the file tree, when it is open.
        if pos + 1 == rects.len()
            && let Some(explorer) = &mut self.explorer
        {
            explorer.focused = true;
            self.mode = Mode::Explorer;
            return;
        }
        let (next, _): (window::WindowId, Rect) = rects[(pos + 1) % rects.len()];
        if let Some(index) = self.windows.iter().position(|w| w.id == next) {
            self.focus(index);
        }
    }

    /// Runs `f` on window `index`'s buffer with that window's view loaded, then restores the active view.
    pub fn with_view<R>(&mut self, index: usize, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
        if index == self.active {
            return Some(f(self.buf_mut()));
        }
        let window: Window = self.windows.get(index)?.clone();
        let buf: &mut Buffer = self.buffers.get_mut(window.buffer)?;
        let saved: View = View::of(buf);
        window.view.apply(buf);
        buf.clamp(false);
        let result: R = f(buf);
        let after: View = View::of(buf);
        saved.apply(buf);
        if let Some(w) = self.windows.get_mut(index) {
            w.view = after;
        }
        Some(result)
    }

    /// After an internal error: windows and layout agree and point at real buffers.
    pub(super) fn repair_windows(&mut self) {
        let mut ids: Vec<(window::WindowId, Rect)> = Vec::new();
        self.layout.rects(self.area, &mut ids);
        let last: usize = self.buffers.len() - 1;
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
