//! User keymaps from the config, matched like Vim: a key that starts a mapping waits for the
//! rest, up to `timeoutlen`; the right-hand side runs without remapping (`noremap`).

use std::time::Instant;

use super::{Editor, Mode};
use crate::config::keys::LEADER;
use crate::config::MapMode;
use crate::term::Key;

impl Editor {
    /// The configured `mapleader` becomes the internal leader key where commands start.
    pub(super) fn translate_leader(&self, key: Key) -> Key {
        let at_command_start = matches!(self.mode, Mode::Normal | Mode::Visual | Mode::VisualLine | Mode::Explorer)
            && self.pending.prefix.is_none_or(|p| p == LEADER);
        if at_command_start && key == self.options.mapleader { Key::Char(LEADER) } else { key }
    }

    fn map_mode(&self) -> Option<MapMode> {
        match self.mode {
            Mode::Normal if self.pending.is_empty() => Some(MapMode::Normal),
            Mode::Insert => Some(MapMode::Insert),
            Mode::Visual | Mode::VisualLine if self.pending.is_empty() => Some(MapMode::Visual),
            _ => None,
        }
    }

    /// True when `key` was consumed by the mapping machinery (waiting, or mapped).
    pub(super) fn map_key(&mut self, key: Key) -> bool {
        let Some(mode) = self.map_mode() else { return self.flush_then(key) };
        self.typeahead.push(key);
        let typed = &self.typeahead;
        let mut exact = None;
        let mut longer = false;
        for map in self.keymaps.iter().filter(|m| m.mode == mode && m.lhs.starts_with(typed)) {
            if map.lhs.len() == typed.len() {
                exact = Some(map.rhs.clone());
            } else {
                longer = true;
            }
        }
        match (exact, longer) {
            (_, true) => {
                self.typeahead_at = Instant::now();
                true
            }
            (Some(rhs), false) => {
                self.typeahead.clear();
                for k in rhs.unwrap_or_default() {
                    self.dispatch(k);
                }
                true
            }
            (None, false) => {
                self.flush_typeahead(false);
                true
            }
        }
    }

    /// Outside mappable states the key goes straight through, after anything still waiting.
    fn flush_then(&mut self, key: Key) -> bool {
        if !self.typeahead.is_empty() {
            self.flush_typeahead(false);
        }
        self.typeahead.push(key);
        self.flush_typeahead(false);
        true
    }

    /// Runs the waiting keys: as their mapping when `timed_out` and one matches exactly, else as typed.
    pub(super) fn flush_typeahead(&mut self, timed_out: bool) {
        let keys = std::mem::take(&mut self.typeahead);
        if timed_out
            && let Some(mode) = self.map_mode()
            && let Some(map) = self.keymaps.iter().find(|m| m.mode == mode && m.lhs == keys)
        {
            for k in map.rhs.clone().unwrap_or_default() {
                self.dispatch(k);
            }
            return;
        }
        for k in keys {
            self.dispatch(k);
        }
    }
}
