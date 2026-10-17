//! `:g` / `:v`: run an Ex command on every line that matches (or not), and `:norm`: type Normal-mode keys on
//! lines. Lines are marked first, so commands that delete or add lines still visit the right ones, and the whole
//! run is one undo step.

use super::command::Command;
use super::vim::LineRange;
use super::{Editor, Mode};
use crate::buffer::{DROP_ON_DELETE, Pos, text};
use crate::error::Error;
use crate::term::Key;

impl Editor {
    /// `:[range]g/pattern/command` (`invert` for `:v` / `:g!`). Without a command, the matches are highlighted
    /// and counted.
    pub(super) fn global(&mut self, range: LineRange, pattern: &str, invert: bool, command: &str) -> Result<(), Error> {
        if self.in_global {
            return Err(Error::Vim("E147: Cannot do :global recursive".into()));
        }
        text::compile(pattern).map_err(Error::Vim)?;
        let buf = self.buf();
        let lines: Vec<usize> = (range.first..=range.last.min(buf.last_line()))
            .filter(|&y| text::find_all(buf.line(y), pattern).is_empty() == invert)
            .collect();
        if lines.is_empty() {
            return Err(Error::PatternNotFound(pattern.into()));
        }
        if command.trim().is_empty() {
            pattern.clone_into(&mut self.search);
            self.highlight_search = true;
            self.info(format!("{} line{} match", lines.len(), if lines.len() == 1 { "" } else { "s" }));
            return Ok(());
        }
        self.in_global = true;
        let result: Result<(), Error> = self.on_marked_lines(&lines, |ed| match Command::parse(command)? {
            Some(command) => match ed.try_run(command) {
                // A line where `:s` finds nothing is not an error inside `:g`, as in Vim.
                Err(Error::PatternNotFound(_)) => Ok(()),
                other => other,
            },
            None => Ok(()),
        });
        self.in_global = false;
        result
    }

    /// `:[range]norm[!] keys`: the keys as typed in Normal mode on each line (the cursor's line without a range);
    /// `!` skips user mappings. An unfinished command is cancelled, as with `<Esc>`.
    pub(super) fn normal_on_lines(&mut self, range: Option<LineRange>, keys: &str, remap: bool) -> Result<(), Error> {
        let lines: Vec<usize> = match range {
            Some(range) => (range.first..=range.last.min(self.buf().last_line())).collect(),
            None => vec![self.buf().cursor.line],
        };
        let keys: Vec<Key> = keys.chars().map(|c| if c == '\t' { Key::Tab } else { Key::Char(c) }).collect();
        self.on_marked_lines(&lines, |ed| {
            ed.mode = Mode::Normal;
            for &key in &keys {
                if remap { ed.handle(key) } else { ed.dispatch(key) }
            }
            // Insert or visual left open, or a half-typed operator: leave them like `<Esc>` would.
            for _ in 0..2 {
                if ed.mode != Mode::Normal || !ed.pending.is_empty() {
                    ed.dispatch(Key::Esc);
                }
            }
            Ok(())
        })
    }

    /// Runs `each` with the cursor at the start of every line in `lines`, following them through edits; a line
    /// deleted on the way is skipped. Stops at the first error. One undo step for the lot.
    fn on_marked_lines(
        &mut self,
        lines: &[usize],
        mut each: impl FnMut(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let current: usize = self.current;
        let ids: Vec<u64> =
            (0..lines.len()).filter_map(|i| u64::try_from(i).ok()).map(|i| DROP_ON_DELETE | i).collect();
        let buf = self.buf_mut();
        for (&id, &y) in ids.iter().zip(lines) {
            buf.add_anchor(id, Pos::new(y, 0));
        }
        buf.seal();
        buf.hold_undo(true);
        let mut result: Result<(), Error> = Ok(());
        for &id in &ids {
            // The command may have switched buffers (`:bn`): stop rather than act on another file.
            if self.current != current {
                break;
            }
            let Some(at): Option<Pos> = self.buf().anchor(id) else { continue };
            self.buf_mut().cursor = Pos::new(at.line, 0);
            result = each(self);
            if result.is_err() {
                break;
            }
        }
        let buf = &mut self.buffers[current];
        for id in ids {
            buf.remove_anchor(id);
        }
        buf.hold_undo(false);
        buf.seal();
        result
    }
}
