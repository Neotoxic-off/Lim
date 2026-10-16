//! Windows and their split layout. AstroNvim sets `splitright` and `splitbelow`, so new windows
//! open right of / below the current one and take the focus.

use crate::buffer::{Buffer, Pos};

/// Per-window view of a buffer. The active window's view lives in its buffer's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    cursor: Pos,
    want_col: usize,
    top: usize,
    left: usize,
}

impl View {
    pub fn of(buf: &Buffer) -> Self {
        Self { cursor: buf.cursor, want_col: buf.want_col, top: buf.top, left: buf.left }
    }

    /// Loads this view into `buf` as is; callers clamp when the text may have changed under it.
    pub fn apply(self, buf: &mut Buffer) {
        buf.cursor = self.cursor;
        buf.want_col = self.want_col;
        buf.top = self.top;
        buf.left = self.left;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowId(u32);

impl WindowId {
    pub const fn first() -> Self {
        Self(0)
    }
}

#[derive(Clone, Debug)]
pub struct Window {
    pub id: WindowId,
    pub buffer: usize,
    pub view: View,
}

impl Window {
    pub fn new(id: u32, buffer: usize, view: View) -> Self {
        Self { id: WindowId(id), buffer, view }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Leaf(WindowId),
    /// `vertical`: children side by side (`:vsplit`), else stacked (`:split`).
    Split { vertical: bool, children: Vec<Layout> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// A one-cell-thick border between split children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Separator {
    pub x: usize,
    pub y: usize,
    pub len: usize,
    pub vertical: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

/// Sizes of `n` children sharing `total` cells with one-cell separators between them.
fn split_sizes(total: usize, n: usize) -> Vec<usize> {
    let avail = total.saturating_sub(n.saturating_sub(1));
    let (base, extra) = (avail / n.max(1), avail % n.max(1));
    (0..n).map(|i| base + usize::from(i < extra)).collect()
}

impl Layout {
    /// Puts `new` right after `target`, nesting a split when the orientation differs.
    pub fn insert_beside(&mut self, target: WindowId, new: WindowId, vertical: bool) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                *self = Self::Split { vertical, children: vec![Self::Leaf(target), Self::Leaf(new)] };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { vertical: v, children } => {
                if *v == vertical
                    && let Some(i) = children.iter().position(|c| *c == Self::Leaf(target))
                {
                    children.insert(i + 1, Self::Leaf(new));
                    return true;
                }
                children.iter_mut().any(|c| c.insert_beside(target, new, vertical))
            }
        }
    }

    /// Removes leaf `id`, collapsing splits left with a single child.
    pub fn remove(&mut self, id: WindowId) -> bool {
        let Self::Split { children, .. } = self else { return false };
        if let Some(i) = children.iter().position(|c| *c == Self::Leaf(id)) {
            children.remove(i);
        } else if !children.iter_mut().any(|c| c.remove(id)) {
            return false;
        }
        if children.len() == 1
            && let Some(only) = children.pop()
        {
            *self = only;
        }
        true
    }

    pub fn rects(&self, area: Rect, out: &mut Vec<(WindowId, Rect)>) {
        self.walk(area, &mut |id, rect| out.push((id, rect)), &mut |_| {});
    }

    pub fn separators(&self, area: Rect, out: &mut Vec<Separator>) {
        self.walk(area, &mut |_, _| {}, &mut |sep| out.push(sep));
    }

    fn walk(&self, area: Rect, leaf: &mut dyn FnMut(WindowId, Rect), sep: &mut dyn FnMut(Separator)) {
        match self {
            Self::Leaf(id) => leaf(*id, area),
            Self::Split { vertical, children } => {
                let total = if *vertical { area.w } else { area.h };
                let mut offset = 0;
                for (i, (child, size)) in children.iter().zip(split_sizes(total, children.len())).enumerate() {
                    let rect = if *vertical {
                        Rect { x: area.x + offset, w: size, ..area }
                    } else {
                        Rect { y: area.y + offset, h: size, ..area }
                    };
                    child.walk(rect, leaf, sep);
                    offset += size;
                    if i + 1 < children.len() && offset < total {
                        sep(if *vertical {
                            Separator { x: area.x + offset, y: area.y, len: area.h, vertical: true }
                        } else {
                            Separator { x: area.x, y: area.y + offset, len: area.w, vertical: false }
                        });
                        offset += 1;
                    }
                }
            }
        }
    }
}

/// The window next to `from` in `dir`, overlapping screen position (`row`, `col`).
pub fn neighbor(rects: &[(WindowId, Rect)], from: Rect, row: usize, col: usize, dir: Direction) -> Option<WindowId> {
    let spans_row = |r: &Rect| r.y <= row && row < r.y + r.h;
    let spans_col = |r: &Rect| r.x <= col && col < r.x + r.w;
    let candidates = rects.iter().filter(|(_, r)| match dir {
        Direction::Left => r.x + r.w <= from.x && spans_row(r),
        Direction::Right => r.x >= from.x + from.w && spans_row(r),
        Direction::Up => r.y + r.h <= from.y && spans_col(r),
        Direction::Down => r.y >= from.y + from.h && spans_col(r),
    });
    let distance = |r: &Rect| match dir {
        Direction::Left => from.x - (r.x + r.w),
        Direction::Right => r.x - (from.x + from.w),
        Direction::Up => from.y - (r.y + r.h),
        Direction::Down => r.y - (from.y + from.h),
    };
    candidates.min_by_key(|(_, r)| distance(r)).map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_rects_and_neighbors() {
        let (a, b, c) = (WindowId(0), WindowId(1), WindowId(2));
        let mut layout = Layout::Leaf(a);
        assert!(layout.insert_beside(a, b, true));
        assert!(layout.insert_beside(b, c, false));
        let area = Rect { x: 0, y: 1, w: 21, h: 11 };
        let mut rects = Vec::new();
        layout.rects(area, &mut rects);
        assert_eq!(rects, vec![
            (a, Rect { x: 0, y: 1, w: 10, h: 11 }),
            (b, Rect { x: 11, y: 1, w: 10, h: 5 }),
            (c, Rect { x: 11, y: 7, w: 10, h: 5 }),
        ]);
        let mut seps = Vec::new();
        layout.separators(area, &mut seps);
        assert_eq!(seps, vec![
            Separator { x: 10, y: 1, len: 11, vertical: true },
            Separator { x: 11, y: 6, len: 10, vertical: false },
        ]);
        assert_eq!(neighbor(&rects, rects[0].1, 8, 0, Direction::Right), Some(c));
        assert_eq!(neighbor(&rects, rects[2].1, 7, 11, Direction::Up), Some(b));
        assert_eq!(neighbor(&rects, rects[1].1, 1, 11, Direction::Right), None);
        assert!(layout.remove(b));
        assert_eq!(layout, Layout::Split { vertical: true, children: vec![Layout::Leaf(a), Layout::Leaf(c)] });
        assert!(layout.remove(c));
        assert_eq!(layout, Layout::Leaf(a));
    }
}
