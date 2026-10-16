//! `AstroNvim` actions that are not plain Vim: `gco`/`gcO`, `]r`/`[r`, `]T`/`[T`, `]G`/`[G`,
//! `<Leader>bs…` sorting, `<Leader>t…` tools, `<Leader>go` git browse, `<C-arrows>` resizing, `gx`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::SystemTime;

use super::bindings::SortBy;
use super::{Editor, Mode, may_spawn};
use crate::buffer::{Buffer, CharClass, Pos, text};
use crate::syntax;

/// Whether `program` is an executable on `$PATH`.
pub fn on_path(program: &str) -> bool {
    env::var_os("PATH").is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// xdg-open / open / explorer, detached from the editor.
pub fn open_external(target: &str) -> bool {
    if !may_spawn() {
        return false;
    }
    let opener: &str = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener).arg(target).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

fn git(args: &[&str], dir: &Path) -> Option<String> {
    let out: Output =
        Command::new("git").arg("-C").arg(dir).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

impl Editor {
    /// `gco` / `gcO`
    pub(super) fn add_comment_line(&mut self, below: bool) {
        self.open_line(below);
        let leader: &str = self.buf().lang.map_or("#", |l| l.comment);
        let buf: &mut Buffer = self.buf_mut();
        let at: Pos = buf.cursor;
        let end: Pos = buf.insert(at, &format!("{leader} "));
        buf.cursor = end;
        self.mode = Mode::Insert;
    }

    /// `gv`
    pub(super) fn reselect(&mut self) {
        if let Some((anchor, cursor, mode)) = self.last_visual {
            self.anchor = anchor;
            let buf: &mut Buffer = self.buf_mut();
            buf.cursor = cursor;
            buf.clamp(false);
            self.mode = mode;
        }
    }

    /// `gx`: opens the URL under the cursor in the browser.
    pub(super) fn open_url_under_cursor(&mut self) {
        let buf: &Buffer = self.buf();
        let line: &str = buf.line(buf.cursor.line);
        let col: usize = buf.cursor.col;
        let url: Option<String> = text::find_urls(line)
            .into_iter()
            .find(|r| r.contains(&col))
            .map(|r| line.chars().skip(r.start).take(r.len()).collect::<String>());
        match url {
            Some(url) if open_external(&url) => self.info(format!("Opening {url}")),
            Some(_) => self.info("No opener available"),
            None => self.info("No URL under the cursor"),
        }
    }

    /// `]G` / `[G`: last or first git hunk.
    pub(super) fn jump_hunk_edge(&mut self, last: bool) {
        let buf: &mut Buffer = self.buf_mut();
        let target: Option<usize> = if last { buf.next_hunk(0, false) } else { buf.next_hunk(buf.last_line(), true) };
        match target {
            Some(line) => {
                self.push_jump();
                let buf: &mut Buffer = self.buf_mut();
                buf.cursor = Pos::new(line, buf.first_non_blank(line));
            }
            None => self.info("No hunks"),
        }
    }

    /// `]r` / `[r` (snacks.words): the next occurrence of the word under the cursor, wrapping.
    pub(super) fn jump_reference(&mut self, forward: bool, count: usize) {
        let buf: &Buffer = self.buf();
        let Some((start, end)): Option<(Pos, Pos)> = buf.word_object(buf.cursor, false) else { return };
        if buf.class_at(start) != CharClass::Word {
            return;
        }
        let word: String = buf.text(start, end);
        let mut hits: Vec<Pos> = Vec::new();
        for (y, line) in buf.lines().iter().enumerate() {
            hits.extend(text::find_word(line, &word).into_iter().map(|r| Pos::new(y, r.start)));
        }
        let Some(mut i): Option<usize> = hits.iter().position(|&p| p == start) else { return };
        for _ in 0..count {
            i = if forward { (i + 1) % hits.len() } else { (i + hits.len() - 1) % hits.len() };
        }
        if let Some(&target) = hits.get(i) {
            self.buf_mut().cursor = target;
        }
    }

    /// `]T` / `[T` (todo-comments): the next comment keyword like `TODO:` or `FIX:`.
    pub(super) fn jump_todo(&mut self, forward: bool) {
        let buf: &Buffer = self.buf();
        let n: usize = buf.line_count();
        let from: usize = buf.cursor.line;
        let found: Option<Pos> = (1..=n)
            .map(|i| if forward { (from + i) % n } else { (from + n * 2 - i) % n })
            .find_map(|y| syntax::todo(buf.line(y)).map(|(col, _, _)| Pos::new(y, col)));
        match found {
            Some(at) => {
                self.push_jump();
                self.buf_mut().cursor = at;
            }
            None => self.info("No TODO comments"),
        }
    }

    /// `<Leader>bse`, `bsr`, `bsp`, `bsi`, `bsm`: by extension, relative path, full path,
    /// opening order or modification time.
    pub(super) fn sort_buffers(&mut self, by: SortBy) {
        let cwd: PathBuf = env::current_dir().unwrap_or_default();
        let key = |i: usize| -> (String, u128) {
            let path: PathBuf = self.buffers[i].path.clone().unwrap_or_default();
            let full: PathBuf = std::path::absolute(&path).unwrap_or_else(|_| path.clone());
            match by {
                SortBy::Extension => (
                    path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
                        + &path.display().to_string(),
                    0,
                ),
                SortBy::RelativePath => (full.strip_prefix(&cwd).unwrap_or(&full).display().to_string(), 0),
                SortBy::FullPath => (full.display().to_string(), 0),
                SortBy::Modified => {
                    let modified: Option<SystemTime> = std::fs::metadata(&full).and_then(|m| m.modified()).ok();
                    let age: u128 = modified.and_then(|t| t.elapsed().ok()).map_or(u128::MAX, |d| d.as_nanos());
                    (String::new(), age)
                }
                SortBy::Number => (String::new(), i as u128),
            }
        };
        let mut order: Vec<usize> = (0..self.buffers.len()).collect();
        order.sort_by_key(|&i| key(i));
        let mut old: Vec<Option<Buffer>> = std::mem::take(&mut self.buffers).into_iter().map(Some).collect();
        self.buffers = order.iter().filter_map(|&i| old.get_mut(i).and_then(Option::take)).collect();
        let new_index = |old: usize| order.iter().position(|&i| i == old).unwrap_or(0);
        self.current = new_index(self.current);
        self.previous_buffer = self.previous_buffer.map(new_index);
        for window in &mut self.windows {
            window.buffer = new_index(window.buffer);
        }
    }

    /// `<Leader>tl`, `tn`, `tp`, `tt`, `tu`, `<Leader>gg`: run a TUI tool in the terminal.
    pub(super) fn run_tool(&mut self, argv: &[&str]) {
        let Some(program): Option<&&str> = argv.first() else { return };
        if on_path(program) {
            self.shell_requested = Some(argv.iter().map(|s| (*s).to_owned()).collect());
        } else {
            self.info(format!("{program} is not installed"));
        }
    }

    /// `<Leader>go`: the current file and line on the git host.
    pub(super) fn git_browse(&mut self) {
        let Some(path): Option<PathBuf> = self.buf().path.clone() else { return self.info("Buffer has no file") };
        let dir: PathBuf =
            path.parent().filter(|d| !d.as_os_str().is_empty()).map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let url: Option<String> = (|| {
            let remote: String = git(&["remote", "get-url", "origin"], &dir)?;
            let branch: String = git(&["rev-parse", "--abbrev-ref", "HEAD"], &dir)?;
            let name: String = path.file_name()?.to_string_lossy().into_owned();
            let file: String = git(&["ls-files", "--full-name", &name], &dir)?;
            Some(format!("{}/blob/{branch}/{file}#L{}", web_url(&remote), self.buf().cursor.line + 1))
        })();
        match url {
            Some(url) if open_external(&url) => self.info(format!("Opening {url}")),
            Some(url) => self.info(url),
            None => self.info("No git remote for this file"),
        }
    }

    /// `<C-Up/Down/Left/Right>`
    pub(super) fn resize_window(&mut self, horizontal: bool, delta: i32) {
        if let Some(id) = self.windows.get(self.active).map(|w| w.id) {
            self.layout.resize(id, horizontal, delta);
        }
    }
}

/// `git@host:user/repo.git` or `https://host/user/repo.git` → `https://host/user/repo`.
fn web_url(remote: &str) -> String {
    let url: &str = remote.trim_end_matches(".git");
    match url.strip_prefix("git@") {
        Some(rest) => format!("https://{}", rest.replacen(':', "/", 1)),
        None => url.replacen("ssh://git@", "https://", 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_to_web() {
        assert_eq!(web_url("git@github.com:a/b.git"), "https://github.com/a/b");
        assert_eq!(web_url("https://gitlab.com/a/b.git"), "https://gitlab.com/a/b");
        assert_eq!(web_url("ssh://git@host/a/b"), "https://host/a/b");
    }
}
