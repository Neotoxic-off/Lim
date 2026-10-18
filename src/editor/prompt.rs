//! snacks.input: a one-line prompt in a float, for renames, new files, indent width, sessions.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Editor, Message, Mode};
use crate::buffer::{Buffer, text};
use crate::term::Key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptAction {
    /// `<Leader>R`
    RenameFile,
    /// `<Leader>ui`
    IndentWidth,
    /// File tree `a` (a trailing `/` makes a directory) and `A`.
    TreeAdd {
        dir: PathBuf,
        directory: bool,
    },
    TreeRename {
        from: PathBuf,
    },
    TreeCopy {
        from: PathBuf,
    },
    TreeMove {
        from: PathBuf,
    },
    /// Asks for `y` before deleting.
    TreeDelete {
        path: PathBuf,
    },
    /// `<Leader>Ss`
    SessionSave,
    /// `<Leader>xk`: the case to put the word under the cursor in.
    Case,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub title: String,
    pub text: String,
    /// Cursor position in chars.
    pub cursor: usize,
    pub action: PromptAction,
    back_to: Mode,
}

impl Editor {
    pub(super) fn open_prompt(&mut self, title: &str, initial: &str, action: PromptAction) {
        let back_to: Mode = if self.mode == Mode::Explorer { Mode::Explorer } else { Mode::Normal };
        self.prompt = Some(Prompt {
            title: title.to_owned(),
            text: initial.to_owned(),
            cursor: text::char_len(initial),
            action,
            back_to,
        });
        self.mode = Mode::Prompt;
    }

    pub(super) fn prompt_key(&mut self, key: Key) {
        let Some(prompt): &mut Option<Prompt> = &mut self.prompt else {
            self.mode = Mode::Normal;
            return;
        };
        let at: usize = text::byte_index(&prompt.text, prompt.cursor);
        match key {
            Key::Esc | Key::Ctrl('c') => {
                self.mode = prompt.back_to;
                self.prompt = None;
            }
            Key::Enter => {
                if let Some(prompt) = self.prompt.take() {
                    self.mode = prompt.back_to;
                    self.submit(prompt.action, prompt.text.trim());
                }
            }
            Key::Char(c) => {
                prompt.text.insert(at, c);
                prompt.cursor += 1;
            }
            Key::Backspace | Key::Ctrl('h') if prompt.cursor > 0 => {
                prompt.cursor -= 1;
                let start: usize = text::byte_index(&prompt.text, prompt.cursor);
                prompt.text.replace_range(start..at, "");
            }
            Key::Delete if at < prompt.text.len() => {
                let end: usize = text::byte_index(&prompt.text, prompt.cursor + 1);
                prompt.text.replace_range(at..end, "");
            }
            Key::Ctrl('u') => {
                prompt.text.replace_range(..at, "");
                prompt.cursor = 0;
            }
            Key::Left => prompt.cursor = prompt.cursor.saturating_sub(1),
            Key::Right => prompt.cursor = (prompt.cursor + 1).min(text::char_len(&prompt.text)),
            Key::Home | Key::Ctrl('a') => prompt.cursor = 0,
            Key::End | Key::Ctrl('e') => prompt.cursor = text::char_len(&prompt.text),
            _ => {}
        }
    }

    fn submit(&mut self, action: PromptAction, input: &str) {
        let touched: Option<PathBuf> = match &action {
            PromptAction::TreeAdd { dir, .. } => Some(dir.join(input)),
            PromptAction::TreeRename { from } | PromptAction::TreeMove { from } | PromptAction::TreeCopy { from } => {
                Some(from.clone())
            }
            PromptAction::TreeDelete { path } => Some(path.clone()),
            PromptAction::RenameFile => self.buf().path.clone(),
            _ => None,
        };
        if touched.is_some_and(|p| !super::may_touch(&p)) {
            return;
        }
        let result: Result<String, String> = match action {
            _ if input.is_empty() && !matches!(action, PromptAction::TreeDelete { .. }) => return,
            PromptAction::Case => self.case_word(input),
            PromptAction::RenameFile => self.rename_current(input),
            PromptAction::IndentWidth => self.set_indent(input),
            PromptAction::TreeAdd { dir, directory } => {
                let path: PathBuf = dir.join(input);
                let made: io::Result<()> = if directory || input.ends_with('/') {
                    fs::create_dir_all(&path)
                } else {
                    path.parent()
                        .map_or(Ok(()), fs::create_dir_all)
                        .and_then(|()| fs::File::create_new(&path).map(drop))
                };
                made.map(|()| format!("Created {}", path.display())).map_err(|e| format!("{}: {e}", path.display()))
            }
            PromptAction::TreeRename { from } => {
                let to: PathBuf = sibling(&from, input);
                self.move_path(&from, &to).map(|()| format!("Renamed to {}", to.display()))
            }
            PromptAction::TreeMove { from } => {
                let to: PathBuf = self.tree_target(input);
                self.move_path(&from, &to).map(|()| format!("Moved to {}", to.display()))
            }
            PromptAction::TreeCopy { from } => {
                let to: PathBuf = self.tree_target(input);
                if !super::may_touch(&to) {
                    return;
                }
                copy_recursive(&from, &to)
                    .map(|()| format!("Copied to {}", to.display()))
                    .map_err(|e| format!("{}: {e}", to.display()))
            }
            PromptAction::TreeDelete { path } => {
                if !input.eq_ignore_ascii_case("y") {
                    return;
                }
                let removed: io::Result<()> =
                    if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
                removed.map(|()| format!("Deleted {}", path.display())).map_err(|e| format!("{}: {e}", path.display()))
            }
            PromptAction::SessionSave => self.save_session(input).map(|()| format!("Session \"{input}\" saved")),
        };
        if let Some(tree) = &mut self.explorer {
            tree.refresh();
        }
        match result {
            Ok(done) => self.set_message(Message::Info(done)),
            Err(e) => self.set_message(Message::Error(e)),
        }
    }

    /// `<Leader>R` and `:rename`: renames the file on disk (if it exists) and the buffer.
    pub(super) fn rename_current(&mut self, input: &str) -> Result<String, String> {
        let to: PathBuf = PathBuf::from(input);
        let from: Option<PathBuf> = self.buf().path.clone();
        if to.exists() && from.as_deref() != Some(to.as_path()) {
            return Err(format!("E13: File exists: {}", to.display()));
        }
        if let Some(from) = from.filter(|f| f.exists()) {
            self.move_path(&from, &to)?;
        } else {
            let buf: &mut Buffer = self.buf_mut();
            buf.lang = crate::syntax::lang_for(&to);
            buf.path = Some(to.clone());
        }
        Ok(format!("Renamed to {}", to.display()))
    }

    /// Renames on disk, then points every buffer that showed the old path at the new one.
    fn move_path(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        if !super::may_touch(to) {
            return Err("refused outside the temp dir".into());
        }
        if to.exists() {
            return Err(format!("E13: File exists: {}", to.display()));
        }
        if let Some(parent) = to.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::rename(from, to).map_err(|e| format!("{}: {e}", from.display()))?;
        for buf in &mut self.buffers {
            if let Some(path) = buf.path.clone()
                && let Ok(rest) = path.strip_prefix(from)
            {
                let moved: PathBuf = if rest.as_os_str().is_empty() { to.to_path_buf() } else { to.join(rest) };
                buf.lang = crate::syntax::lang_for(&moved);
                buf.path = Some(moved);
            }
        }
        Ok(())
    }

    /// `<Leader>ui`: like `AstroNvim`, a positive width means spaces, zero or negative means tabs.
    fn set_indent(&mut self, input: &str) -> Result<String, String> {
        let n: i64 = input.parse().map_err(|_| format!("Invalid indent: {input}"))?;
        let width: usize = n.unsigned_abs().clamp(1, 16) as usize;
        self.options.expandtab = n > 0;
        self.options.tabstop = width;
        self.options.shiftwidth = width;
        self.apply_options();
        Ok(format!("indent={width} expandtab={}", self.options.expandtab))
    }

    fn tree_target(&self, input: &str) -> PathBuf {
        let root: PathBuf = self.explorer.as_ref().map_or_else(|| PathBuf::from("."), |t| t.root.clone());
        let path: PathBuf = PathBuf::from(input);
        if path.is_absolute() { path } else { root.join(path) }
    }
}

fn sibling(from: &Path, name: &str) -> PathBuf {
    if name.contains('/') { PathBuf::from(name) } else { from.with_file_name(name) }
}

fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if to.starts_with(from) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "cannot copy a directory into itself"));
    }
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "already exists"));
    }
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)?.flatten() {
            // Symlinks are copied as the files they point to; directories recurse.
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to).map(drop)
    }
}
