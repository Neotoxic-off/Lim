//! `.` (repeat last change) and macros (`q{reg}` … `q`, `@{reg}`, `@@`).

use super::{Editor, Mode};
use crate::term::Key;

/// Nested `@` beyond this depth is ignored, so a macro calling itself cannot loop forever.
const MAX_MACRO_DEPTH: usize = 32;

#[derive(Debug, Default)]
pub(super) struct Repeat {
    /// Keys of the last command that changed text.
    last_change: Vec<Key>,
    /// Keys of the command being typed, with the buffer and version it started on.
    current: Vec<Key>,
    origin: (usize, u64),
    replaying: bool,
    /// Set by `u`, `Ctrl-r` and `.`: their effect is not a change `.` should repeat.
    pub(super) skip: bool,
    macros: std::collections::HashMap<char, Vec<Key>>,
    recording: Option<(char, Vec<Key>)>,
    last_macro: Option<char>,
    depth: usize,
}

impl Editor {
    pub fn recording(&self) -> Option<char> {
        self.repeat.recording.as_ref().map(|(reg, _)| *reg)
    }

    /// Called around every key: tracks the keys of each command so `.` can replay text changes.
    pub(super) fn track_before(&mut self, key: Key) {
        if self.repeat.depth == 0
            && let Some((_, keys)) = &mut self.repeat.recording
        {
            keys.push(key);
        }
        if self.repeat.replaying {
            return;
        }
        if self.mode == Mode::Normal && self.pending.is_empty() {
            self.repeat.current.clear();
            self.repeat.origin = (self.current, self.buf().version());
        }
        self.repeat.current.push(key);
    }

    pub(super) fn track_after(&mut self) {
        let (buffer, version) = self.repeat.origin;
        let changed = buffer == self.current && version != self.buf().version();
        // `.` itself never becomes the last change, or the next `.` would replay itself forever.
        let skip = std::mem::take(&mut self.repeat.skip);
        if !self.repeat.replaying && self.mode == Mode::Normal && self.pending.is_empty() && changed && !skip {
            self.repeat.last_change = std::mem::take(&mut self.repeat.current);
            self.repeat.origin.1 = self.buf().version();
        }
    }

    pub(super) fn reset_repeat(&mut self) {
        self.repeat.replaying = false;
        self.repeat.depth = 0;
        self.repeat.skip = false;
    }

    pub(super) fn repeat_change(&mut self, count: usize) {
        if self.repeat.replaying {
            return;
        }
        let keys = self.repeat.last_change.clone();
        self.repeat.replaying = true;
        for _ in 0..count {
            for &key in &keys {
                self.dispatch(key);
            }
        }
        self.repeat.replaying = false;
        self.repeat.skip = true;
    }

    pub(super) fn start_recording(&mut self, reg: char) {
        if reg.is_ascii_alphanumeric() {
            self.repeat.recording = Some((reg, Vec::new()));
        }
    }

    pub(super) fn stop_recording(&mut self) {
        if let Some((reg, mut keys)) = self.repeat.recording.take() {
            keys.pop(); // the `q` that stopped it
            self.repeat.macros.insert(reg, keys);
        }
    }

    pub(super) fn play_macro(&mut self, reg: char, count: usize) {
        let reg = if reg == '@' { self.repeat.last_macro } else { Some(reg) };
        let Some(keys) = reg.and_then(|r| self.repeat.macros.get(&r)).cloned() else { return };
        if self.repeat.depth >= MAX_MACRO_DEPTH {
            return;
        }
        self.repeat.last_macro = reg;
        self.repeat.depth += 1;
        for _ in 0..count {
            for &key in &keys {
                self.dispatch(key);
            }
        }
        self.repeat.depth -= 1;
    }
}
