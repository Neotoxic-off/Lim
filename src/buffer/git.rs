//! gitsigns: which lines are added, changed or deleted against the git index.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Add,
    Change,
    /// Lines were deleted below this one.
    Delete,
}

/// The index version of the file and the signs computed for one buffer version.
#[derive(Debug, Default)]
pub struct GitState {
    base: Option<Vec<String>>,
    signs: Vec<Option<Sign>>,
    computed_for: Option<u64>,
}

/// Diagonal steps one diff may take (a few ms); a rewrite needing more marks what is left changed wholesale.
const MAX_STEPS: usize = 2_000_000;

impl GitState {
    /// Reads the staged version via `git show :./name`; no repo, untracked file or no git → no signs.
    pub fn load(&mut self, path: &Path) {
        self.base = index_text(path).map(|text| {
            let body: &str = text.strip_suffix('\n').unwrap_or(&text);
            body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned()).collect()
        });
        self.computed_for = None;
    }

    pub fn signs(&mut self, lines: &[String], version: u64) -> &[Option<Sign>] {
        if self.computed_for != Some(version) {
            self.signs = self.base.as_deref().map_or_else(Vec::new, |base| diff(base, lines));
            self.computed_for = Some(version);
        }
        &self.signs
    }
}

fn index_text(path: &Path) -> Option<String> {
    let dir: &Path = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name: String = path.file_name()?.to_string_lossy().into_owned();
    let output: Output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("show")
        .arg(format!(":./{name}"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// One sign slot per line of `new`.
pub fn diff(old: &[String], new: &[String]) -> Vec<Option<Sign>> {
    let mut signs: Vec<Option<Sign>> = vec![None; new.len()];
    let prefix: usize = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix: usize = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    let (old_mid, new_mid): (&[String], &[String]) =
        (&old[prefix..old.len() - suffix], &new[prefix..new.len() - suffix]);
    if old_mid.is_empty() && new_mid.is_empty() {
        return signs;
    }
    let ops: Vec<Op> = Myers::ops(old_mid, new_mid);
    // Each run of deletions/insertions between kept lines becomes one hunk.
    let (mut line, mut deleted, mut inserted): (usize, usize, usize) = (prefix, 0, 0);
    let flush = |line: usize, deleted: usize, inserted: usize, signs: &mut Vec<Option<Sign>>| {
        let start: usize = line - inserted;
        for (i, slot) in signs.iter_mut().enumerate().skip(start).take(inserted) {
            *slot = Some(if i - start < deleted { Sign::Change } else { Sign::Add });
        }
        if deleted > inserted {
            let at: usize = if line > 0 { line - 1 } else { 0 };
            if let Some(slot) = signs.get_mut(at)
                && slot.is_none()
            {
                *slot = Some(Sign::Delete);
            }
        }
    };
    for op in ops {
        match op {
            Op::Keep => {
                flush(line, deleted, inserted, &mut signs);
                (deleted, inserted) = (0, 0);
                line += 1;
            }
            Op::Delete => deleted += 1,
            Op::Insert => {
                inserted += 1;
                line += 1;
            }
        }
    }
    flush(line, deleted, inserted, &mut signs);
    signs
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Keep,
    Delete,
    Insert,
}

/// Myers' O(ND) diff in linear space (the middle snake, split and recursed), over line ids.
struct Myers {
    a: Vec<usize>,
    b: Vec<usize>,
    ops: Vec<Op>,
    steps: usize,
    /// Furthest x reached forward, and furthest y backward, per diagonal (box-relative); reused between snakes.
    forward: Vec<isize>,
    backward: Vec<isize>,
}

fn signed(v: usize) -> isize {
    isize::try_from(v).unwrap_or(isize::MAX)
}

fn unsigned(v: isize) -> usize {
    usize::try_from(v).unwrap_or(0)
}

/// A middle snake: from the point before its one edit to the end of its diagonal run.
type Snake = ((usize, usize), (usize, usize));

impl Myers {
    fn ops<'a>(old: &'a [String], new: &'a [String]) -> Vec<Op> {
        let mut ids: HashMap<&'a str, usize> = HashMap::new();
        let mut id = |line: &'a String| -> usize {
            let next: usize = ids.len();
            *ids.entry(line.as_str()).or_insert(next)
        };
        let a: Vec<usize> = old.iter().map(&mut id).collect();
        let b: Vec<usize> = new.iter().map(&mut id).collect();
        // Lines missing from the other side can never be kept: diff without them (as xdiff does), so a rewrite
        // costs nothing to search, then put them back as deletions and insertions.
        let mut seen: [Vec<bool>; 2] = [vec![false; ids.len()], vec![false; ids.len()]];
        for (side, lines) in seen.iter_mut().zip([&a, &b]) {
            for &line in lines {
                side[line] = true;
            }
        }
        let a_kept: Vec<usize> = (0..a.len()).filter(|&i| seen[1][a[i]]).collect();
        let b_kept: Vec<usize> = (0..b.len()).filter(|&j| seen[0][b[j]]).collect();
        let mut myers: Self = Self {
            a: a_kept.iter().map(|&i| a[i]).collect(),
            b: b_kept.iter().map(|&j| b[j]).collect(),
            ops: Vec::with_capacity(a_kept.len() + b_kept.len()),
            steps: 0,
            forward: Vec::new(),
            backward: Vec::new(),
        };
        myers.compare(0, a_kept.len(), 0, b_kept.len());
        let mut ops: Vec<Op> = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j, mut fi, mut fj): (usize, usize, usize, usize) = (0, 0, 0, 0);
        for op in myers.ops {
            let (to_i, to_j): (usize, usize) = match op {
                Op::Keep => (a_kept[fi], b_kept[fj]),
                Op::Delete => (a_kept[fi] + 1, j),
                Op::Insert => (i, b_kept[fj] + 1),
            };
            ops.extend(std::iter::repeat_n(Op::Delete, to_i - i));
            ops.extend(std::iter::repeat_n(Op::Insert, to_j - j));
            (i, j) = (to_i, to_j);
            if op == Op::Keep {
                ops.push(Op::Keep);
                (i, j) = (i + 1, j + 1);
            }
            fi += usize::from(op != Op::Insert);
            fj += usize::from(op != Op::Delete);
        }
        ops.extend(std::iter::repeat_n(Op::Delete, a.len() - i));
        ops.extend(std::iter::repeat_n(Op::Insert, b.len() - j));
        ops
    }

    /// Ops turning `a[left..right]` into `b[top..bottom]`.
    fn compare(&mut self, mut left: usize, mut right: usize, mut top: usize, mut bottom: usize) {
        while left < right && top < bottom && self.a[left] == self.b[top] {
            self.ops.push(Op::Keep);
            (left, top) = (left + 1, top + 1);
        }
        let mut kept: usize = 0;
        while left < right && top < bottom && self.a[right - 1] == self.b[bottom - 1] {
            (right, bottom, kept) = (right - 1, bottom - 1, kept + 1);
        }
        // Both sides non-empty with different ends: at least 2 edits, so each part below costs less.
        let snake: Option<Snake> =
            if left == right || top == bottom { None } else { self.middle_snake(left, right, top, bottom) };
        if let Some(((px, py), (x, y))) = snake {
            self.compare(left, px, top, py);
            self.compare(px, x, py, y);
            self.compare(x, right, y, bottom);
        } else {
            self.ops.extend(std::iter::repeat_n(Op::Delete, right - left));
            self.ops.extend(std::iter::repeat_n(Op::Insert, bottom - top));
        }
        self.ops.extend(std::iter::repeat_n(Op::Keep, kept));
    }

    /// The middle snake of the box, searched from both ends at once; `None` once over `MAX_STEPS`.
    fn middle_snake(&mut self, left: usize, right: usize, top: usize, bottom: usize) -> Option<Snake> {
        let (w, h): (isize, isize) = (signed(right - left), signed(bottom - top));
        let max: isize = (w + h + 1) / 2;
        let ix = |k: isize| unsigned(k + max + 1);
        // Only the seeds need setting: every other slot is written before it is read.
        let mut forward: Vec<isize> = std::mem::take(&mut self.forward);
        let mut backward: Vec<isize> = std::mem::take(&mut self.backward);
        if forward.len() <= ix(max + 1) {
            forward.resize(ix(max + 1) + 1, 0);
            backward.resize(ix(max + 1) + 1, 0);
        }
        (forward[ix(1)], backward[ix(1)]) = (0, h);
        let snake: Option<Snake> = self.search(&mut forward, &mut backward, (left, top), (w, h));
        (self.forward, self.backward) = (forward, backward);
        snake
    }

    /// The two-ended search of `middle_snake` in a `w` x `h` box at `origin`.
    fn search(
        &mut self,
        forward: &mut [isize],
        backward: &mut [isize],
        (left, top): (usize, usize),
        (w, h): (isize, isize),
    ) -> Option<Snake> {
        let delta: isize = w - h;
        let odd: bool = delta.rem_euclid(2) == 1;
        let max: isize = (w + h + 1) / 2;
        let ix = |k: isize| unsigned(k + max + 1);
        let same = |me: &Self, x: isize, y: isize| me.a[left + unsigned(x)] == me.b[top + unsigned(y)];
        let at = |x: isize, y: isize| (left + unsigned(x), top + unsigned(y));
        for d in 0..=max {
            self.steps += unsigned(2 * d + 1);
            if self.steps > MAX_STEPS {
                return None;
            }
            for k in (-d..=d).rev().step_by(2) {
                let (px, mut x): (isize, isize) = if k == -d || (k != d && forward[ix(k - 1)] < forward[ix(k + 1)]) {
                    (forward[ix(k + 1)], forward[ix(k + 1)])
                } else {
                    (forward[ix(k - 1)], forward[ix(k - 1)] + 1)
                };
                let mut y: isize = x - k;
                let py: isize = if d == 0 || x != px { y } else { y - 1 };
                while x < w && y < h && same(self, x, y) {
                    (x, y) = (x + 1, y + 1);
                }
                forward[ix(k)] = x;
                let c: isize = k - delta;
                if odd && (-(d - 1)..=d - 1).contains(&c) && y >= backward[ix(c)] {
                    return Some((at(px, py), at(x, y)));
                }
            }
            for c in (-d..=d).rev().step_by(2) {
                let (py, mut y): (isize, isize) = if c == -d || (c != d && backward[ix(c - 1)] > backward[ix(c + 1)]) {
                    (backward[ix(c + 1)], backward[ix(c + 1)])
                } else {
                    (backward[ix(c - 1)], backward[ix(c - 1)] - 1)
                };
                let k: isize = c + delta;
                let mut x: isize = y + k;
                let px: isize = if d == 0 || y != py { x } else { x + 1 };
                while x > 0 && y > 0 && same(self, x - 1, y - 1) {
                    (x, y) = (x - 1, y - 1);
                }
                backward[ix(c)] = y;
                if !odd && (-d..=d).contains(&k) && x <= forward[ix(k)] {
                    return Some((at(x, y), at(px, py)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn hunks() {
        use Sign::*;
        assert_eq!(diff(&lines("a b c"), &lines("a b c")), vec![None; 3]);
        assert_eq!(diff(&lines("a b c"), &lines("a X b c")), vec![None, Some(Add), None, None]);
        assert_eq!(diff(&lines("a b c"), &lines("a B c")), vec![None, Some(Change), None]);
        assert_eq!(diff(&lines("a b c d"), &lines("a d")), vec![Some(Delete), None]);
        assert_eq!(diff(&lines("a b c"), &lines("b c")), vec![Some(Delete), None]);
        assert_eq!(diff(&lines("a b c"), &lines("a B B2 c")), vec![None, Some(Change), Some(Add), None]);
    }

    /// Myers' ops rebuild `new` from `old` and keep as many lines as a plain LCS table finds.
    #[test]
    fn myers_is_a_shortest_edit() {
        let mut seed: u32 = 7;
        let mut next = |n: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 16) % n
        };
        for _ in 0..300 {
            let mut sides: [Vec<String>; 2] = [Vec::new(), Vec::new()];
            for side in &mut sides {
                for _ in 0..next(12) {
                    side.push(next(4).to_string());
                }
            }
            let [old, new]: [Vec<String>; 2] = sides;
            let ops: Vec<Op> = Myers::ops(&old, &new);
            let (mut i, mut j, mut kept): (usize, usize, usize) = (0, 0, 0);
            for op in &ops {
                match op {
                    Op::Keep => {
                        assert_eq!(old[i], new[j], "{old:?} -> {new:?}");
                        (i, j, kept) = (i + 1, j + 1, kept + 1);
                    }
                    Op::Delete => i += 1,
                    Op::Insert => j += 1,
                }
            }
            assert_eq!((i, j), (old.len(), new.len()));
            let mut table: Vec<Vec<usize>> = vec![vec![0; new.len() + 1]; old.len() + 1];
            for i in (0..old.len()).rev() {
                for j in (0..new.len()).rev() {
                    table[i][j] =
                        if old[i] == new[j] { table[i + 1][j + 1] + 1 } else { table[i + 1][j].max(table[i][j + 1]) };
                }
            }
            assert_eq!(kept, table[0][0], "{old:?} -> {new:?}");
        }
    }

    /// A rewrite touching every 10th line of 20k gets exact signs, fast; a reversal stays within the step budget.
    #[test]
    fn big_scattered_rewrite() {
        let old: Vec<String> = (0..20_000).map(|n| format!("line {n}")).collect();
        let new: Vec<String> =
            old.iter().enumerate().map(|(n, l)| if n % 10 == 0 { format!("{l} edited") } else { l.clone() }).collect();
        let signs: Vec<Option<Sign>> = diff(&old, &new);
        assert!(signs.iter().enumerate().all(|(n, s)| *s == (n % 10 == 0).then_some(Sign::Change)));
        let unrelated: Vec<String> = (0..20_000).map(|n| format!("other {n}")).collect();
        assert!(diff(&old, &unrelated).iter().all(|s| *s == Some(Sign::Change)), "no shared line: nothing to search");
        let reversed: Vec<String> = old.iter().rev().cloned().collect();
        assert_eq!(diff(&old, &reversed).len(), 20_000, "over the step budget: the rest is changed wholesale");
    }
}
