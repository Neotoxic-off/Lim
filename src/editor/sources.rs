//! Picker sources beyond files: git (graph, log, status, stash), config files, recent in the
//! working directory, notifications, registers, marks, colorschemes, icon sets and sessions.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::picker::{Item, PickerKind, Preview, Target, file_items};
use super::{Editor, Message, Mode, Picker, session};
use crate::buffer::{Buffer, Pos};

/// Git log entries listed at most.
const MAX_COMMITS: &str = "500";

fn git(args: &[&str]) -> Option<String> {
    let out: Output = Command::new("git").args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines_of(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter(|l| !l.trim().is_empty())
}

impl Editor {
    /// Items for the sources this module owns; None (with a message) when there is nothing to show.
    #[expect(clippy::too_many_lines, reason = "flat dispatch: one arm per picker source")]
    pub(super) fn extra_items(&mut self, kind: PickerKind) -> Option<Vec<Item>> {
        let items: Vec<Item> = match kind {
            PickerKind::GitGraph => {
                let out: String = git(&[
                    "log",
                    "--graph",
                    "--all",
                    "--decorate",
                    "--date-order",
                    "--date=short",
                    "--pretty=format:%h %ad %d %s",
                    "-n",
                    MAX_COMMITS,
                ])?;
                // Lines with a commit open it; the ones that only draw the graph do nothing.
                lines_of(&out)
                    .map(|l| {
                        let hash: &str = l.trim_start_matches(|c: char| "*|/\\_ .-".contains(c));
                        let hash: &str = hash.split(' ').next().unwrap_or_default();
                        let is_commit: bool = hash.len() >= 7 && hash.bytes().all(|b| b.is_ascii_hexdigit());
                        let target: Target = if is_commit { Target::Commit(hash.to_owned()) } else { Target::Nothing };
                        Item { label: l.to_owned(), target }
                    })
                    .collect()
            }
            PickerKind::GitLog { file } => {
                let mut args: Vec<&str> = vec!["log", "--date=short", "--pretty=format:%h %ad %s", "-n", MAX_COMMITS];
                let path: Option<String> = self.buf().path.as_ref().map(|p| p.display().to_string());
                if file {
                    args.push("--");
                    args.push(path.as_deref()?);
                }
                let out: String = git(&args)?;
                lines_of(&out)
                    .map(|l| Item {
                        label: l.to_owned(),
                        target: Target::Commit(l.split(' ').next().unwrap_or_default().to_owned()),
                    })
                    .collect()
            }
            PickerKind::GitStatus => {
                let out: String = git(&["status", "--porcelain", "--untracked-files=all"])?;
                lines_of(&out)
                    .filter_map(|l| {
                        let path: String = l.get(3..)?.rsplit(" -> ").next()?.to_owned();
                        Some(Item { label: l.to_owned(), target: Target::File(PathBuf::from(path), None) })
                    })
                    .collect()
            }
            PickerKind::GitStash => {
                let out: String = git(&["stash", "list"])?;
                lines_of(&out)
                    .map(|l| Item {
                        label: l.to_owned(),
                        target: Target::Stash(l.split(':').next().unwrap_or_default().to_owned()),
                    })
                    .collect()
            }
            PickerKind::ConfigFiles => {
                let path: PathBuf = crate::config::path()?;
                let mut items: Vec<Item> = file_items(vec![path.clone()]);
                if let Some(dir) = path.parent()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    items.extend(file_items(
                        entries.flatten().map(|e| e.path()).filter(|p| *p != path && p.is_file()).collect(),
                    ));
                }
                items
            }
            PickerKind::RecentHere => {
                let cwd: PathBuf = std::env::current_dir().unwrap_or_default();
                file_items(
                    self.recent
                        .iter()
                        .filter(|p| p.starts_with(&cwd) && p.is_file())
                        .map(|p| p.strip_prefix(&cwd).map_or_else(|_| p.clone(), Path::to_path_buf))
                        .collect(),
                )
            }
            PickerKind::Notifications => self
                .history
                .iter()
                .rev()
                .map(|m| {
                    let label: String = match m {
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
                let buf: &Buffer = self.buf();
                let mut marks: Vec<_> = buf.marks.iter().filter(|(n, _)| n.is_ascii_lowercase()).collect();
                marks.sort();
                marks
                    .into_iter()
                    .map(|(name, pos)| {
                        let line: &str = buf.lines().get(pos.line).map_or("", |l| l.trim());
                        Item { label: format!("{name}  {:>5}  {line}", pos.line + 1), target: Target::Line(pos.line) }
                    })
                    .collect()
            }
            PickerKind::Themes => crate::render::palettes::THEMES
                .iter()
                .map(|t| Item { label: t.name.to_owned(), target: Target::Theme(t.name.to_owned()) })
                .collect(),
            PickerKind::IconSets => crate::render::icons::SETS
                .iter()
                .map(|(s, _, _)| Item { label: (*s).to_owned(), target: Target::IconSet((*s).to_owned()) })
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
        let Some(picker): &mut Option<Picker> = &mut self.picker else { return };
        let target: Option<Target> = picker.selected_item().map(|i| i.target.clone());
        let preview: Option<(String, String)> = match &target {
            Some(Target::Commit(hash)) => {
                git(&["show", "--stat", "--patch", hash]).map(|o| (format!("commit {hash}"), o))
            }
            Some(Target::Stash(name)) => git(&["stash", "show", "-p", name]).map(|o| (name.clone(), o)),
            _ => None,
        };
        picker.preview =
            preview.map(|(title, text)| Preview { title, lines: text.lines().map(str::to_owned).collect() });
        if let Some(Target::Theme(name)) = target {
            self.set_colorscheme(&name, false);
        }
    }

    pub(super) fn accept_extra(&mut self, target: Target) {
        match target {
            Target::Commit(hash) => self.show_scratch(&format!("commit {hash}"), git(&["show", &hash])),
            Target::Stash(name) => self.show_scratch(&name, git(&["stash", "show", "-p", &name])),
            Target::Session { name, dir, delete: false } => self.load_session(&name, dir),
            Target::Session { name, dir, delete: true } => self.delete_session(&name, dir),
            Target::Theme(name) => self.set_colorscheme(&name, true),
            Target::IconSet(name) => {
                self.options.iconset.clone_from(&name);
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
    pub(super) fn show_scratch(&mut self, title: &str, text: Option<String>) {
        let Some(text): Option<String> = text else { return self.info("git produced no output") };
        let mut buf: Buffer = Buffer::from_text(None, &text);
        buf.title = Some(title.to_owned());
        buf.cursor = Pos::default();
        self.buffers.push(buf);
        self.switch_to(self.buffers.len() - 1);
        self.dashboard = false;
    }

    /// The settings screen, the theme picker and `<Leader>ub`; `persist` remembers it for next start.
    pub(super) fn set_colorscheme(&mut self, name: &str, persist: bool) {
        if crate::render::palettes::THEMES.iter().all(|t| t.name != name) {
            return self.set_message(Message::Error(format!("E185: Cannot find color scheme '{name}'")));
        }
        let warnings: Vec<String> = crate::render::apply_colorscheme(name, &self.config);
        name.clone_into(&mut self.colorscheme);
        if persist {
            crate::render::save_colorscheme(name);
            self.info(format!("colorscheme {name}"));
        }
        if !warnings.is_empty() {
            self.set_message(Message::Error(warnings.join("; ")));
        }
    }
}
