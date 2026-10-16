//! gitsigns: which lines are added, changed or deleted against the git index.

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

/// Above this many line pairs left after trimming, the middle is marked changed wholesale.
// shortcut: LCS on the differing middle only; switch to Myers if huge rewrites need exact signs.
const MAX_CELLS: usize = 1_000_000;

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
    let ops: Vec<Op> = if old_mid.len().saturating_mul(new_mid.len()) <= MAX_CELLS {
        lcs_ops(old_mid, new_mid)
    } else {
        let mut ops: Vec<Op> = vec![Op::Delete; old_mid.len()];
        ops.extend(std::iter::repeat_n(Op::Insert, new_mid.len()));
        ops
    };
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

fn lcs_ops(old: &[String], new: &[String]) -> Vec<Op> {
    let (n, m): (usize, usize) = (old.len(), new.len());
    // table[i][j] = LCS length of old[i..] and new[j..]
    let mut table: Vec<u32> = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if old[i] == new[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut ops: Vec<Op> = Vec::with_capacity(n + m);
    let (mut i, mut j): (usize, usize) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            ops.push(Op::Keep);
            i += 1;
            j += 1;
        } else if j < m && (i == n || table[at(i, j + 1)] >= table[at(i + 1, j)]) {
            ops.push(Op::Insert);
            j += 1;
        } else {
            ops.push(Op::Delete);
            i += 1;
        }
    }
    ops
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
}
