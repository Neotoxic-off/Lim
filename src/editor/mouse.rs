//! The mouse (`mouse` option): a click places the cursor and focuses its window, a drag selects, the wheel
//! scrolls the window under the pointer, or moves through a picker or the settings.

use super::manage::CloseSet;
use super::{Editor, Mode, Rect};
use crate::buffer::{Buffer, Pos, text};
use crate::term::{Key, Mouse, MouseKind};

/// Lines the wheel scrolls per notch, as Neovim's `mousescroll=ver:3`.
const WHEEL_LINES: usize = 3;

/// Where the renderer drew a window's text, so a screen cell maps back to a position in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    pub window: usize,
    /// The text rows, gutter included.
    pub area: Rect,
    pub gutter: usize,
    /// First buffer line and first screen column shown.
    pub top: usize,
    pub left: usize,
}

impl Pane {
    fn contains(&self, col: usize, row: usize) -> bool {
        (self.area.x..self.area.x + self.area.w).contains(&col)
            && (self.area.y..self.area.y + self.area.h).contains(&row)
    }
}

/// What a click on the tabline does, as `NvChad`'s tabufline buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabClick {
    Buffer(usize),
    Close(usize),
    ToggleTheme,
    CloseAll,
}

/// Columns of the tabline (the top row) a click acts on, from the last frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabHit {
    pub from: usize,
    pub to: usize,
    pub click: TabClick,
}

impl Editor {
    pub(super) fn mouse(&mut self, event: Mouse) {
        if !self.options.mouse {
            return;
        }
        let wheel: Option<Key> = match event.kind {
            MouseKind::ScrollUp => Some(Key::Up),
            MouseKind::ScrollDown => Some(Key::Down),
            _ => None,
        };
        if self.mode == Mode::Normal
            && event.kind == MouseKind::Press
            && event.row == 0
            && self.click_tabline(event.col)
        {
            return;
        }
        match self.mode {
            Mode::Normal | Mode::Insert | Mode::Replace | Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                self.mouse_in_text(event);
            }
            // Lists move with the wheel; their own keys do the rest.
            Mode::Picker | Mode::Setup | Mode::Cheatsheet | Mode::Explorer => {
                if let Some(key) = wheel {
                    self.dispatch(key);
                }
            }
            _ => {}
        }
    }

    /// A click on a tab, its close button or a button on the right; false when `col` holds none.
    fn click_tabline(&mut self, col: usize) -> bool {
        let Some(hit): Option<TabHit> = self.tab_hits.iter().copied().find(|h| (h.from..h.to).contains(&col)) else {
            return false;
        };
        match hit.click {
            TabClick::Buffer(i) => self.switch_to(i),
            TabClick::Close(i) => {
                if let Err(e) = self.remove_buffer(i, false) {
                    self.error(&e);
                }
            }
            TabClick::ToggleTheme => self.toggle_background(),
            TabClick::CloseAll => self.close_buffers(CloseSet::All),
        }
        true
    }

    fn mouse_in_text(&mut self, event: Mouse) {
        let Some(pane): Option<Pane> = self.panes.iter().copied().find(|p| p.contains(event.col, event.row)) else {
            return;
        };
        match event.kind {
            MouseKind::ScrollUp | MouseKind::ScrollDown => {
                self.focus_pointed(pane.window);
                self.scroll_view(event.kind == MouseKind::ScrollDown, WHEEL_LINES);
            }
            MouseKind::Press => {
                if matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
                    self.mode = Mode::Normal;
                }
                self.focus_pointed(pane.window);
                let at: Pos = self.position_at(&pane, event.col, event.row);
                self.mouse_anchor = Some((self.active, at));
                self.place_cursor(at);
            }
            MouseKind::Drag => {
                // A selection stays in the window (so the buffer) it was pressed in.
                let Some((window, anchor)): Option<(usize, Pos)> = self.mouse_anchor else { return };
                if window != pane.window || window != self.active {
                    return;
                }
                if !matches!(self.mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock) {
                    self.mode = Mode::Visual;
                    self.anchor = anchor;
                }
                let at: Pos = self.position_at(&pane, event.col, event.row);
                self.place_cursor(at);
            }
            MouseKind::Release => self.mouse_anchor = None,
        }
    }

    /// Focuses the window under the pointer; insert or visual mode ends first, its state belongs to the old buffer.
    fn focus_pointed(&mut self, window: usize) {
        if window != self.active && self.mode != Mode::Normal {
            self.dispatch(Key::Esc);
        }
        self.focus(window);
    }

    /// The text position under a screen cell of `pane`: past the end of a line is its end, the gutter is col 0.
    fn position_at(&self, pane: &Pane, col: usize, row: usize) -> Pos {
        let buf: &Buffer = self.buf();
        let line: usize = (pane.top + row - pane.area.y).min(buf.last_line());
        let x: usize = col.saturating_sub(pane.area.x + pane.gutter) + pane.left;
        Pos::new(line, text::col_at_display(buf.line(line), x))
    }

    fn place_cursor(&mut self, at: Pos) {
        let past_end: bool = matches!(self.mode, Mode::Insert | Mode::Replace);
        let buf: &mut Buffer = self.buf_mut();
        buf.cursor = at;
        buf.clamp(past_end);
        buf.want_col = buf.cursor.col;
    }
}
