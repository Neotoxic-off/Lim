//! Visual (`v`) and visual-line (`V`) modes.

use super::motion::{Motion, Span};
use super::normal::VISUAL_PREFIXES;
use super::operator::Operator;
use super::{Editor, Mode, Pending};
use crate::term::Key;

impl Editor {
    pub(super) fn visual(&mut self, key: Key) {
        if let Some(prefix) = self.pending.prefix.take() {
            return self.prefixed(prefix, key);
        }
        if self.push_count(key) || self.push_prefix(key, VISUAL_PREFIXES) {
            return;
        }
        if let Some(motion) = Motion::from_key(key) {
            return self.run_motion(motion);
        }
        self.pending = Pending::default();
        let cursor = self.buf().cursor;
        match key {
            Key::Esc | Key::Ctrl('c') => self.mode = Mode::Normal,
            Key::Char('v') => self.toggle_visual(Mode::Visual),
            Key::Char('V') => self.toggle_visual(Mode::VisualLine),
            Key::Char('o') => {
                let anchor = self.anchor;
                self.anchor = cursor;
                self.buf_mut().cursor = anchor;
            }
            Key::Char(c) => {
                let op = match c {
                    'x' => Some(Operator::Delete),
                    's' => Some(Operator::Change),
                    c => Operator::from_char(c),
                };
                if let Some(op) = op {
                    let span = if self.mode == Mode::VisualLine { Span::Linewise } else { Span::Inclusive };
                    self.mode = Mode::Normal;
                    self.buf_mut().seal();
                    self.apply(op, self.anchor, cursor, span);
                }
            }
            _ => {}
        }
    }

    fn toggle_visual(&mut self, mode: Mode) {
        self.mode = if self.mode == mode { Mode::Normal } else { mode };
    }
}
