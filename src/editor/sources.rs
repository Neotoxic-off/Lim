//! Picker sources beyond files: git (branches, log, status, stash), config files, recent in the
//! working directory, notifications, registers, marks, colorschemes, icon sets and sessions.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::picker::{Item, PickerKind, Preview, Target, file_items};
use super::{Editor, Message, Mode, may_spawn, session};
use crate::buffer::{Buffer, Pos};

/// Git log entries listed at most.
const MAX_COMMITS: &str = "500";

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_or_error(args: &[&str]) -> Result<String, String> {
    if !may_spawn() {
        return Err("disabled in tests".into());
    }
    let out = Command::new("git").args(args).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn lines_of(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter(|l| !l.trim().is_empty())
}

impl Editor {
    /// Items for the sources this module owns; None (with a message) when there is nothing to show.
    pub(super) fn extra_items(&mut self, kind: PickerKind) -> Option<Vec<Item>> {
        let items: Vec<Item> = match kind {
            PickerKind::GitBranches => {
                let out = git(&["branch", "--all", "--format=%(HEAD) %(refname:short)"])?;
                lines_of(&out)
                    .map(|l| {
                        let name = l.get(1..).unwrap_or_default().trim().to_owned();
                        Item { label: l.to_owned(), target: Target::Branch(name) }
                    })
                    .collect()
            }
            PickerKind::GitLog { file } => {
                let mut args = vec!["log", "--date=short", "--pretty=format:%h %ad %s", "-n", MAX_COMMITS];
                let path = self.buf().path.as_ref().map(|p| p.display().to_string());
                if file {
                    args.push("--");
                    args.push(path.as_deref()?);
                }
                let out = git(&args)?;
                lines_of(&out)
                    .map(|l| Item { label: l.to_owned(), target: Target::Commit(l.split(' ').next().unwrap_or_default().to_owned()) })
                    .collect()
            }
            PickerKind::GitStatus => {
                let out = git(&["status", "--porcelain", "--untracked-files=all"])?;
                lines_of(&out)
                    .filter_map(|l| {
                        let path = l.get(3..)?.rsplit(" -> ").next()?.to_owned();
                        Some(Item { label: l.to_owned(), target: Target::File(PathBuf::from(path), None) })
                    })
                    .collect()
            }
            PickerKind::GitStash => {
                let out = git(&["stash", "list"])?;
                lines_of(&out)
                    .map(|l| Item { label: l.to_owned(), target: Target::Stash(l.split(':').next().unwrap_or_default().to_owned()) })
                    .collect()
            }
            PickerKind::ConfigFiles => {
                let path = crate::config::path()?;
                let mut items = file_items(vec![path.clone()]);
                if let Some(dir) = path.parent()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    items.extend(file_items(entries.flatten().map(|e| e.path()).filter(|p| *p != path && p.is_file()).collect()));
                }
                items
            }
            PickerKind::RecentHere => {
                let cwd = std::env::current_dir().unwrap_or_default();
                file_items(
                    self.recent
                        .iter()
                        .filter(|p| p.starts_with(&cwd) && p.is_file())
                        .map(|p| p.strip_prefix(&cwd).map(Path::to_path_buf).unwrap_or_else(|_| p.clone()))
                        .collect(),
                )
            }
            PickerKind::Notifications => self
                .history
                .iter()
                .rev()
                .map(|m| {
                    let label = match m {
                        Message::Info(t) => format!("info   {t}"),
                        Message::Error(t) => format!("error  {t}"),
                    };
                    Item { label, target: Target::Nothing }
                })
                .collect(),
            PickerKind::Registers => self
                .registers
                .list()
                .into_iter()
                .map(|(name, reg)| {
                    let shown: String = reg.text.replace('\n', "⏎").chars().take(120).collect();
                    Item { label: format!("\"{name}  {shown}"), target: Target::Register(name) }
                })
                .collect(),
            PickerKind::Marks => {
                let buf = self.buf();
                let mut marks: Vec<_> = buf.marks.iter().filter(|(n, _)| n.is_ascii_lowercase()).collect();
                marks.sort();
                marks
                    .into_iter()
                    .map(|(name, pos)| {
                        let line = buf.lines().get(pos.line).map_or("", |l| l.trim());
                        Item { label: format!("{name}  {:>5}  {line}", pos.line + 1), target: Target::Line(pos.line) }
                    })
                    .collect()
            }
            PickerKind::Themes => crate::render::palettes::THEMES
                .iter()
                .map(|t| Item { label: t.name.to_owned(), target: Target::Theme(t.name.to_owned()) })
                .collect(),
            PickerKind::IconSets => ["nerd", "unicode", "ascii"]
                .iter()
                .map(|s| Item { label: (*s).to_owned(), target: Target::IconSet((*s).to_owned()) })
                .collect(),
            PickerKind::Sessions { dir, delete } => session::list(dir)
                .into_iter()
                .map(|name| Item { label: name.clone(), target: Target::Session { name, dir, delete } })
                .collect(),
            _ => Vec::new(),
        };
        if items.is_empty() {
            self.info(format!("{}: nothing to show", kind.title()));
            return None;
        }
        Some(items)
    }

    /// Keeps the preview (and the live theme preview) in step with the selection.
    pub(super) fn selection_changed(&mut self) {
        let Some(picker) = &mut self.picker else { return };
        let target = picker.selected_item().map(|i| i.target.clone());
        let preview = match &target {
            Some(Target::Commit(hash)) => git(&["show", "--stat", "--patch", hash]).map(|o| (format!("commit {hash}"), o)),
            Some(Target::Stash(name)) => git(&["stash", "show", "-p", name]).map(|o| (name.clone(), o)),
            Some(Target::Branch(name)) => git(&["log", "--oneline", "-n", "50", name]).map(|o| (name.clone(), o)),
            _ => None,
        };
        picker.preview = preview.map(|(title, text)| Preview { title, lines: text.lines().map(str::to_owned).collect() });
        if let Some(Target::Theme(name)) = target {
            self.set_colorscheme(&name, false);
        }
    }

    pub(super) fn accept_extra(&mut self, target: Target) {
        match target {
            Target::Branch(name) => {
                let local = name.strip_prefix("origin/").unwrap_or(&name).to_owned();
                match git_or_error(&["switch", &local]) {
                    Ok(_) => {
                        self.reload_changed_buffers();
                        self.info(format!("Switched to {local}"));
                    }
                    Err(e) => self.set_message(Message::Error(e)),
                }
            }
            Target::Commit(hash) => self.show_scratch(&format!("commit {hash}"), git(&["show", &hash])),
            Target::Stash(name) => self.show_scratch(&name, git(&["stash", "show", "-p", &name])),
            Target::Session { name, dir, delete: false } => self.load_session(&name, dir),
            Target::Session { name, dir, delete: true } => self.delete_session(&name, dir),
            Target::Theme(name) => self.set_colorscheme(&name, true),
            Target::IconSet(name) => {
                self.options.iconset = name.clone();
                self.options.icons = name != "ascii";
                self.info(format!("Icons: {name}"));
            }
            Target::Register(name) => {
                self.register_name = Some(name);
                self.mode = Mode::Normal;
                self.handle(crate::term::Key::Char('p'));
            }
            _ => {}
        }
    }

    /// Opens git output (a commit, a stash) in a read-only scratch buffer.
    fn show_scratch(&mut self, title: &str, text: Option<String>) {
        let Some(text) = text else { return self.info("git produced no output") };
        let mut buf = Buffer::from_text(None, &text);
        buf.title = Some(title.to_owned());
        buf.cursor = Pos::default();
        self.buffers.push(buf);
        self.switch_to(self.buffers.len() - 1);
        self.dashboard = false;
    }

    /// `:colorscheme`, the theme picker and `<Leader>ub`; `persist` remembers it for next start.
    pub(super) fn set_colorscheme(&mut self, name: &str, persist: bool) {
        if crate::render::palettes::THEMES.iter().all(|t| t.name != name) {
            return self.set_message(Message::Error(format!("E185: Cannot find color scheme '{name}'")));
        }
        let warnings = crate::render::apply_colorscheme(name, &self.config);
        self.colorscheme = name.to_owned();
        if persist {
            crate::render::save_colorscheme(name);
            self.info(format!("colorscheme {name}"));
        }
        if !warnings.is_empty() {
            self.set_message(Message::Error(warnings.join("; ")));
        }
    }
}
