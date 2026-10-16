//! Command-line history, one for `:` and one for `/` `?`, as in Vim: `<Up>` / `<Down>` recall the entries that
//! start with what was typed before browsing.

const MAX_ENTRIES: usize = 100;

#[derive(Default)]
pub struct CmdHistory {
    /// Oldest first, without duplicates.
    entries: Vec<String>,
    /// The entry shown while browsing; None while editing.
    browsing: Option<usize>,
    /// What was typed when browsing started: the prefix filter, given back past the newest entry.
    typed: String,
}

impl CmdHistory {
    /// Records a submitted line as the newest entry.
    pub fn push(&mut self, entry: &str) {
        self.reset();
        if entry.is_empty() {
            return;
        }
        self.entries.retain(|e: &String| e != entry);
        self.entries.push(entry.to_owned());
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    /// Stops browsing: the next `step` filters on the text typed by then.
    pub fn reset(&mut self) {
        self.browsing = None;
    }

    /// The next older (`older`) or newer entry starting with the typed prefix; after the newest, the typed text
    /// itself. None when there is nowhere to go.
    pub fn step(&mut self, older: bool, current: &str) -> Option<String> {
        if self.browsing.is_none() {
            current.clone_into(&mut self.typed);
        }
        let typed: &str = &self.typed;
        let matches = |e: &String| e.starts_with(typed);
        let found: Option<usize> = if older {
            let end: usize = self.browsing.unwrap_or(self.entries.len());
            self.entries[..end].iter().rposition(matches)
        } else {
            let start: usize = self.browsing? + 1;
            self.entries[start..].iter().position(matches).map(|i: usize| start + i)
        };
        match found {
            Some(i) => {
                self.browsing = Some(i);
                Some(self.entries[i].clone())
            }
            None if older => None,
            None => {
                self.browsing = None;
                Some(self.typed.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browses_with_prefix_and_comes_back() {
        let mut history: CmdHistory = CmdHistory::default();
        for entry in ["w", "set nu", "wq", "", "set nu"] {
            history.push(entry);
        }
        assert_eq!(history.entries, ["w", "wq", "set nu"], "blank skipped, repeat moved to the end");
        assert_eq!(history.step(true, ""), Some("set nu".into()));
        assert_eq!(history.step(true, "set nu"), Some("wq".into()));
        assert_eq!(history.step(true, "wq"), Some("w".into()));
        assert_eq!(history.step(true, "w"), None, "the oldest entry stays");
        assert_eq!(history.step(false, "w"), Some("wq".into()));
        assert_eq!(history.step(false, "wq"), Some("set nu".into()));
        assert_eq!(history.step(false, "set nu"), Some(String::new()), "past the newest: what was typed");
        assert_eq!(history.step(false, ""), None, "not browsing: nothing newer");

        assert_eq!(history.step(true, "w"), Some("wq".into()), "the typed prefix filters");
        assert_eq!(history.step(true, "wq"), Some("w".into()));
        assert_eq!(history.step(false, "w"), Some("wq".into()));
        assert_eq!(history.step(false, "wq"), Some("w".into()), "back to the typed text");
    }

    #[test]
    fn keeps_the_newest_entries() {
        let mut history: CmdHistory = CmdHistory::default();
        for i in 0..=MAX_ENTRIES {
            history.push(&i.to_string());
        }
        assert_eq!(history.entries.len(), MAX_ENTRIES);
        assert_eq!(history.entries.first().map(String::as_str), Some("1"));
    }
}
