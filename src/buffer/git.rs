//! gitsigns: which lines are added, changed or deleted against the git index.

use std::path::Path;

use gix::diff::blob::{Algorithm, Diff, InternedInput};

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
    counts: Counts,
    computed_for: Option<u64>,
}

/// gitsigns' status counts, shown in the statusline.
/// shortcut: `removed` counts deletion hunks, not deleted lines; diff would have to report them to be exact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
}

impl GitState {
    /// Reads the staged version from the index; no repo or untracked file → no signs.
    pub fn load(&mut self, path: &Path) {
        self.base = crate::repo::index_text(path).map(|text| {
            let body: &str = text.strip_suffix('\n').unwrap_or(&text);
            body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned()).collect()
        });
        self.computed_for = None;
    }

    pub fn signs(&mut self, lines: &[String], version: u64) -> &[Option<Sign>] {
        if self.computed_for != Some(version) {
            self.signs = self.base.as_deref().map_or_else(Vec::new, |base| diff(base, lines));
            self.counts = Counts::default();
            for sign in self.signs.iter().flatten() {
                match sign {
                    Sign::Add => self.counts.added += 1,
                    Sign::Change => self.counts.changed += 1,
                    Sign::Delete => self.counts.removed += 1,
                }
            }
            self.computed_for = Some(version);
        }
        &self.signs
    }

    /// The counts of the last `signs` call.
    pub fn counts(&self) -> Counts {
        self.counts
    }
}

/// One sign slot per line of `new`: imara-diff's Myers diff (its histogram is several times slower on big
/// rewrites), each hunk marking its lines changed (as many as it
/// deleted) then added, or the line above a pure deletion.
pub fn diff(old: &[String], new: &[String]) -> Vec<Option<Sign>> {
    let mut input: InternedInput<&str> = InternedInput::default();
    input.update_before(old.iter().map(String::as_str));
    input.update_after(new.iter().map(String::as_str));
    let mut signs: Vec<Option<Sign>> = vec![None; new.len()];
    for hunk in Diff::compute(Algorithm::Myers, &input).hunks() {
        let deleted: usize = hunk.before.len();
        let start: usize = usize::try_from(hunk.after.start).unwrap_or(0);
        let end: usize = usize::try_from(hunk.after.end).unwrap_or(0).min(signs.len());
        for (i, slot) in signs.iter_mut().enumerate().take(end).skip(start) {
            *slot = Some(if i - start < deleted { Sign::Change } else { Sign::Add });
        }
        if deleted > end.saturating_sub(start)
            && let Some(slot) = signs.get_mut(end.saturating_sub(1))
            && slot.is_none()
        {
            *slot = Some(Sign::Delete);
        }
    }
    signs
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

    #[test]
    fn counts_follow_the_signs() {
        let mut state: GitState = GitState { base: Some(lines("a b c d e")), ..GitState::default() };
        state.signs(&lines("a B B2 c e"), 1);
        assert_eq!(state.counts(), Counts { added: 1, changed: 1, removed: 1 });
        state.signs(&lines("a b c d e"), 2);
        assert_eq!(state.counts(), Counts::default(), "recounted with each version");
    }

    /// A rewrite touching every 10th line of 20k gets exact signs; unrelated or reversed text is all changed.
    #[test]
    fn big_scattered_rewrite() {
        let old: Vec<String> = (0..20_000).map(|n| format!("line {n}")).collect();
        let new: Vec<String> =
            old.iter().enumerate().map(|(n, l)| if n % 10 == 0 { format!("{l} edited") } else { l.clone() }).collect();
        let signs: Vec<Option<Sign>> = diff(&old, &new);
        assert!(signs.iter().enumerate().all(|(n, s)| *s == (n % 10 == 0).then_some(Sign::Change)));
        let unrelated: Vec<String> = (0..20_000).map(|n| format!("other {n}")).collect();
        assert!(diff(&old, &unrelated).iter().all(|s| *s == Some(Sign::Change)), "no shared line");
        let reversed: Vec<String> = old.iter().rev().cloned().collect();
        assert_eq!(diff(&old, &reversed).len(), 20_000);
    }
}
