//! git through gix: gitsigns' base, statuses, the file list, the git pickers and git browse. No git process runs.
//! An opened folder may come from anyone (an archive, a copied checkout), so nothing its own `.git/config` names
//! is run: gix only takes filter and diff drivers from the user's and the system's config.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::bstr::{BString, ByteSlice};
use gix::config::Source;
use gix::config::file::Metadata;
use gix::diff::blob::unified_diff::{ConsumeHunk, ContextSize, DiffLineKind, HunkHeader};
use gix::diff::blob::{Algorithm, Diff, InternedInput, UnifiedDiff};
use gix::object::tree::diff::ChangeDetached;
use gix::revision::walk::Sorting;
use gix::sec::Trust;
use gix::traverse::commit::simple::CommitTimeOrder;
use gix::{Commit as GixCommit, ObjectId, Repository};

use crate::buffer::canonical;

/// The repository holding `dir`.
pub fn open(dir: &Path) -> Option<Repository> {
    let options: gix::open::Options = gix::open::Options::default().filter_config_section(trusted);
    let map: gix::sec::trust::Mapping<gix::open::Options> =
        gix::sec::trust::Mapping { full: options.clone(), reduced: options };
    let repo: gix::ThreadSafeRepository =
        gix::ThreadSafeRepository::discover_opts(dir, gix::discover::upwards::Options::default(), map).ok()?;
    Some(repo.to_thread_local())
}

/// Config sections whose programs gix may run: the user's and the system's, never the repository's own.
fn trusted(meta: &Metadata) -> bool {
    meta.trust == Trust::Full && !matches!(meta.source, Source::Local | Source::Worktree)
}

/// `path` relative to the work tree, `/`-separated, as the index and trees name it.
fn rela_path(repo: &Repository, path: &Path) -> Option<String> {
    let root: PathBuf = canonical(repo.workdir()?);
    let full: PathBuf = canonical(path);
    let rel: &Path = full.strip_prefix(&root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    Some(parts.join("/"))
}

/// The folder of a file path, `.` for a bare name.
fn parent(path: &Path) -> &Path {
    path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

/// The staged version of `path` (`git show :./name`); None outside a repo or for an untracked file.
pub fn index_text(path: &Path) -> Option<String> {
    let repo: Repository = open(parent(path))?;
    let rel: String = rela_path(&repo, path)?;
    let index: gix::worktree::Index = repo.index().ok()?;
    let entry: &gix::index::Entry = index.entry_by_path(rel.as_bytes().as_bstr())?;
    String::from_utf8(repo.find_object(entry.id).ok()?.detach().data).ok()
}

/// Changed files under the repository of `dir`, as `git status --porcelain` codes (`" M"`, `"A "`, `"??"`...), by
/// absolute path.
pub fn status(dir: &Path) -> Vec<(PathBuf, [char; 2])> {
    use gix::status::Item;
    use gix::status::index_worktree::iter::Summary;
    let Some(repo): Option<Repository> = open(dir) else { return Vec::new() };
    let Some(root): Option<PathBuf> = repo.workdir().map(canonical) else { return Vec::new() };
    let Ok(platform) = repo.status(gix::progress::Discard) else { return Vec::new() };
    let no_patterns: Vec<BString> = Vec::new();
    let Ok(items) = platform.untracked_files(gix::status::UntrackedFiles::Files).into_iter(no_patterns) else {
        return Vec::new();
    };
    let mut codes: BTreeMap<String, [char; 2]> = BTreeMap::new();
    for item in items.flatten() {
        let slot: &mut [char; 2] = codes.entry(item.location().to_str_lossy().into_owned()).or_insert([' ', ' ']);
        match &item {
            Item::TreeIndex(change) => {
                slot[0] = match change {
                    gix::diff::index::Change::Addition { .. } => 'A',
                    gix::diff::index::Change::Deletion { .. } => 'D',
                    gix::diff::index::Change::Modification { .. } => 'M',
                    gix::diff::index::Change::Rewrite { .. } => 'R',
                };
            }
            Item::IndexWorktree(change) => match change.summary() {
                Some(Summary::Added) => *slot = ['?', '?'],
                Some(Summary::Removed) => slot[1] = 'D',
                Some(Summary::Renamed | Summary::Copied) => slot[1] = 'R',
                Some(Summary::Conflict) => *slot = ['U', 'U'],
                Some(Summary::IntentToAdd) => slot[1] = 'A',
                Some(Summary::Modified | Summary::TypeChange) => slot[1] = 'M',
                None => {}
            },
        }
    }
    codes.into_iter().filter(|(_, code)| *code != [' ', ' ']).map(|(path, code)| (root.join(path), code)).collect()
}

/// Tracked and untracked, not ignored, files under `dir`, relative to it (`git ls-files --cached --others
/// --exclude-standard`); None outside a repo.
pub fn files(dir: &Path) -> Option<Vec<PathBuf>> {
    use gix::dir::entry::Status;
    static NEVER: AtomicBool = AtomicBool::new(false);
    let repo: Repository = open(dir)?;
    let root: PathBuf = canonical(repo.workdir()?);
    let here: PathBuf = canonical(dir);
    let options: gix::dirwalk::Options =
        repo.dirwalk_options().ok()?.emit_tracked(true).emit_untracked(gix::dir::walk::EmissionMode::Matching);
    let index: gix::worktree::IndexPersistedOrInMemory = repo.index_or_empty().ok()?.into();
    let no_patterns: Vec<BString> = Vec::new();
    let walk: gix::dirwalk::Iter = repo.dirwalk_iter(index, no_patterns, (&NEVER).into(), options).ok()?;
    let mut files: Vec<PathBuf> = Vec::new();
    for item in walk.flatten() {
        if !matches!(item.entry.status, Status::Tracked | Status::Untracked) {
            continue;
        }
        let full: PathBuf = root.join(item.entry.rela_path.to_str_lossy().as_ref());
        if let Ok(rel) = full.strip_prefix(&here) {
            files.push(rel.to_path_buf());
        }
    }
    files.sort();
    Some(files)
}

/// One commit as the git pickers show it.
pub struct Commit {
    pub id: ObjectId,
    pub parents: Vec<ObjectId>,
    /// `%h %ad %d %s`: short hash, author date, ` (refs)` when `all`, subject.
    pub line: String,
}

impl Commit {
    pub fn hash(&self) -> String {
        self.id.to_hex_with_len(7).to_string()
    }
}

/// The newest `max` commits of the repository holding `dir`, by date: from every ref (`--all`, with their names) or
/// from HEAD only, and when `file` is given, only those changing it.
pub fn log(dir: &Path, all: bool, file: Option<&Path>, max: usize) -> Option<Vec<Commit>> {
    let repo: Repository = open(dir)?;
    let file: Option<String> = match file {
        Some(path) => Some(rela_path(&repo, path)?),
        None => None,
    };
    let names: HashMap<ObjectId, Vec<String>> = if all { ref_names(&repo) } else { HashMap::new() };
    let tips: Vec<ObjectId> = if all { names.keys().copied().collect() } else { vec![repo.head_id().ok()?.detach()] };
    let walk = repo.rev_walk(tips).sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst)).all().ok()?;
    let mut commits: Vec<Commit> = Vec::new();
    // The file's blob by commit: a commit's parent is mostly the next one walked, so each tree is read once.
    let mut blobs: HashMap<ObjectId, Option<ObjectId>> = HashMap::new();
    for info in walk.flatten() {
        let parents: Vec<ObjectId> = info.parent_ids.iter().copied().collect();
        if let Some(path) = &file {
            let mut blob = |id: ObjectId| *blobs.entry(id).or_insert_with(|| path_id(&repo, id, path));
            if blob(info.id) == parents.first().and_then(|&p| blob(p)) {
                continue;
            }
        }
        let commit: GixCommit<'_> = info.object().ok()?;
        let date: String = commit
            .author()
            .ok()
            .and_then(|a| a.time().ok())
            .map(|t| t.format_or_unix(gix::date::time::format::SHORT))
            .unwrap_or_default();
        let refs: String = names.get(&info.id).map(|n| format!(" ({})", n.join(", "))).unwrap_or_default();
        let subject: String = commit.message().map(|m| m.summary().to_str_lossy().into_owned()).unwrap_or_default();
        let hash: String = info.id.to_hex_with_len(7).to_string();
        commits.push(Commit { id: info.id, parents, line: format!("{hash} {date} {refs} {subject}") });
        if commits.len() >= max {
            break;
        }
    }
    Some(commits)
}

/// The blob `path` has in commit `id`.
fn path_id(repo: &Repository, id: ObjectId, path: &str) -> Option<ObjectId> {
    let tree: gix::Tree<'_> = repo.find_commit(id).ok()?.tree().ok()?;
    Some(tree.lookup_entry_by_path(path).ok()??.object_id())
}

/// The names `--decorate` shows, by the commit they point at: `HEAD -> main`, `origin/main`, `tag: v1`.
fn ref_names(repo: &Repository) -> HashMap<ObjectId, Vec<String>> {
    use gix::reference::Category;
    let mut names: HashMap<ObjectId, Vec<String>> = HashMap::new();
    let head: Option<String> = repo.head_name().ok().flatten().map(|n| n.shorten().to_string());
    if let Ok(id) = repo.head_id() {
        names.entry(id.detach()).or_default().push(if head.is_some() { String::new() } else { "HEAD".into() });
    }
    let Ok(platform) = repo.references() else { return names };
    let Ok(all) = platform.all() else { return names };
    for mut reference in all.flatten() {
        let short: String = reference.name().shorten().to_string();
        let name: String = match reference.name().category() {
            Some(Category::LocalBranch) if head.as_deref() == Some(short.as_str()) => format!("HEAD -> {short}"),
            Some(Category::LocalBranch | Category::RemoteBranch) => short,
            Some(Category::Tag) => format!("tag: {short}"),
            _ => continue,
        };
        let Ok(id) = reference.peel_to_id() else { continue };
        if repo.find_commit(id).is_ok() {
            names.entry(id.detach()).or_default().push(name);
        }
    }
    for list in names.values_mut() {
        list.retain(|n| !n.is_empty());
    }
    names
}

/// `git log --graph` lines for `commits`, newest first: a lane per open line of history, `*` on each commit, and
/// connector rows (`/` where a lane ends in another, `\` where a merge opens one) that hold no commit.
pub fn graph(commits: &[Commit]) -> Vec<String> {
    let mut lanes: Vec<Option<ObjectId>> = Vec::new();
    let mut rows: Vec<String> = Vec::new();
    for commit in commits {
        let waiting: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i] == Some(commit.id)).collect();
        // Lanes that all lead to this commit first join the leftmost one.
        if let [_, joining @ ..] = waiting.as_slice()
            && !joining.is_empty()
        {
            let mut joint: Vec<char> = vec![' '; lanes.len() * 2];
            for (i, lane) in lanes.iter().enumerate() {
                if joining.contains(&i) {
                    joint[i * 2 - 1] = '/';
                } else if lane.is_some() {
                    joint[i * 2] = '|';
                }
            }
            for &i in joining {
                lanes[i] = None;
            }
            rows.push(joint.into_iter().collect::<String>().trim_end().to_owned());
        }
        let col: usize = waiting.first().copied().unwrap_or_else(|| {
            lanes.iter().position(Option::is_none).unwrap_or_else(|| {
                lanes.push(None);
                lanes.len() - 1
            })
        });
        let row: String = (0..lanes.len())
            .map(|i| {
                if i == col {
                    "*"
                } else if lanes[i].is_some() {
                    "|"
                } else {
                    " "
                }
            })
            .collect::<Vec<&str>>()
            .join(" ");
        rows.push(format!("{} {}", row.trim_end(), commit.line));
        // Its first parent takes its lane; a merge's other parents open new ones.
        lanes[col] = commit.parents.first().copied();
        let mut opened: Vec<usize> = Vec::new();
        for parent in commit.parents.iter().skip(1) {
            if lanes.contains(&Some(*parent)) {
                continue;
            }
            let free: usize = (col + 1..lanes.len()).find(|&i| lanes[i].is_none()).unwrap_or_else(|| {
                lanes.push(None);
                lanes.len() - 1
            });
            lanes[free] = Some(*parent);
            opened.push(free);
        }
        if !opened.is_empty() {
            let mut fork: Vec<char> = vec![' '; lanes.len() * 2];
            for (i, lane) in lanes.iter().enumerate() {
                if opened.contains(&i) {
                    fork[i * 2 - 1] = '\\';
                } else if lane.is_some() {
                    fork[i * 2] = '|';
                }
            }
            rows.push(fork.into_iter().collect::<String>().trim_end().to_owned());
        }
        while lanes.last() == Some(&None) {
            lanes.pop();
        }
    }
    rows
}

/// `git stash list`: `(stash@{n}, "stash@{n}: message")`, newest first.
pub fn stashes(dir: &Path) -> Vec<(String, String)> {
    let Some(repo): Option<Repository> = open(dir) else { return Vec::new() };
    let Ok(stash) = repo.find_reference("refs/stash") else { return Vec::new() };
    let mut platform = stash.log_iter();
    let Ok(Some(entries)) = platform.rev() else { return Vec::new() };
    entries
        .flatten()
        .enumerate()
        .map(|(n, entry)| {
            let name: String = format!("stash@{{{n}}}");
            let label: String = format!("{name}: {}", entry.message.to_str_lossy());
            (name, label)
        })
        .collect()
}

/// What `rev` (a hash, `stash@{n}`) changed against its first parent, as `git show` prints it: a header with the
/// author, date and message (`header`), a `--stat` summary (`stat`), then the patch.
pub fn show(dir: &Path, rev: &str, header: bool, stat: bool) -> Option<String> {
    let repo: Repository = open(dir)?;
    let commit: GixCommit<'_> = repo.rev_parse_single(rev).ok()?.object().ok()?.peel_to_commit().ok()?;
    let new: gix::Tree<'_> = commit.tree().ok()?;
    let old: gix::Tree<'_> = match commit.parent_ids().next() {
        Some(parent) => parent.object().ok()?.peel_to_commit().ok()?.tree().ok()?,
        None => repo.empty_tree(),
    };
    let changes: Vec<ChangeDetached> = repo.diff_tree_to_tree(&old, &new, None).ok()?;
    let mut out: String = String::new();
    if header {
        let author: Option<gix::actor::SignatureRef<'_>> = commit.author().ok();
        let _: std::fmt::Result = writeln!(out, "commit {}", commit.id);
        if let Some(author) = author {
            let date: String =
                author.time().map(|t| t.format_or_unix(gix::date::time::format::DEFAULT)).unwrap_or_default();
            let _: std::fmt::Result = write!(out, "Author: {} <{}>\nDate:   {date}\n", author.name, author.email);
        }
        out.push('\n');
        let message: String = commit.message_raw_sloppy().to_str_lossy().into_owned();
        for line in message.trim_end().lines() {
            let _: std::fmt::Result = writeln!(out, "    {line}");
        }
        out.push('\n');
    }
    let patches: Vec<(String, Patch)> =
        changes.iter().filter_map(|change| Some((location(change), patch(&repo, change)?))).collect();
    if stat {
        out.push_str(&stats(&patches));
        out.push('\n');
    }
    for (_, p) in &patches {
        out.push_str(&p.text);
    }
    Some(out)
}

/// One file's part of a diff.
struct Patch {
    text: String,
    added: usize,
    removed: usize,
}

fn location(change: &ChangeDetached) -> String {
    change.location().to_str_lossy().into_owned()
}

/// A file change as a unified diff (no textconv or external driver: the blobs as stored); None for trees.
fn patch(repo: &Repository, change: &ChangeDetached) -> Option<Patch> {
    let blob = |id: ObjectId| repo.find_object(id).ok().map(|o| o.detach().data);
    let path: String = location(change);
    let (old, new, from, to): (Option<Vec<u8>>, Option<Vec<u8>>, String, String) = match change {
        ChangeDetached::Addition { entry_mode, id, .. } if !entry_mode.is_tree() => {
            (None, blob(*id), "/dev/null".into(), format!("b/{path}"))
        }
        ChangeDetached::Deletion { entry_mode, id, .. } if !entry_mode.is_tree() => {
            (blob(*id), None, format!("a/{path}"), "/dev/null".into())
        }
        ChangeDetached::Modification { entry_mode, previous_id, id, .. } if !entry_mode.is_tree() => {
            (blob(*previous_id), blob(*id), format!("a/{path}"), format!("b/{path}"))
        }
        ChangeDetached::Rewrite { source_location, source_id, id, entry_mode, .. } if !entry_mode.is_tree() => {
            (blob(*source_id), blob(*id), format!("a/{}", source_location.to_str_lossy()), format!("b/{path}"))
        }
        _ => return None,
    };
    let (old, new): (Vec<u8>, Vec<u8>) = (old.unwrap_or_default(), new.unwrap_or_default());
    let mut text: String = format!("diff --git a/{path} b/{path}\n");
    if old.contains(&0) || new.contains(&0) {
        text.push_str("Binary files differ\n");
        return Some(Patch { text, added: 0, removed: 0 });
    }
    let (old, new): (String, String) =
        (String::from_utf8_lossy(&old).into_owned(), String::from_utf8_lossy(&new).into_owned());
    let input: InternedInput<&str> = InternedInput::new(old.as_str(), new.as_str());
    let diff: Diff = gix::diff::blob::diff_with_slider_heuristics(Algorithm::Histogram, &input);
    let (mut added, mut removed): (usize, usize) = (0, 0);
    for hunk in diff.hunks() {
        removed += hunk.before.len();
        added += hunk.after.len();
    }
    let hunks: String =
        UnifiedDiff::new(&diff, &input, Hunks(String::new()), ContextSize::symmetrical(3)).consume().ok()?;
    if !hunks.is_empty() {
        let _: std::fmt::Result = write!(text, "--- {from}\n+++ {to}\n{hunks}");
    }
    Some(Patch { text, added, removed })
}

/// `--stat`: a line per file with its count of changed lines and a +/- bar, then the totals.
fn stats(patches: &[(String, Patch)]) -> String {
    const BAR: usize = 50;
    let width: usize = patches.iter().map(|(path, _)| path.chars().count()).max().unwrap_or(0);
    let most: usize = patches.iter().map(|(_, p)| p.added + p.removed).max().unwrap_or(0).max(1);
    let mut out: String = String::new();
    let (mut added, mut removed): (usize, usize) = (0, 0);
    for (path, p) in patches {
        let total: usize = p.added + p.removed;
        let scale = |n: usize| if most > BAR { (n * BAR).div_ceil(most) } else { n };
        let bar: String = format!("{}{}", "+".repeat(scale(p.added)), "-".repeat(scale(p.removed)));
        let _: std::fmt::Result = writeln!(out, " {path:<width$} | {total} {bar}");
        (added, removed) = (added + p.added, removed + p.removed);
    }
    let files: usize = patches.len();
    let _: std::fmt::Result = writeln!(
        out,
        " {files} file{} changed, {added} insertion{}(+), {removed} deletion{}(-)",
        if files == 1 { "" } else { "s" },
        if added == 1 { "" } else { "s" },
        if removed == 1 { "" } else { "s" },
    );
    out
}

/// Collects hunks as unified diff text.
struct Hunks(String);

impl ConsumeHunk for Hunks {
    type Out = String;

    fn consume_hunk(&mut self, header: HunkHeader, lines: &[(DiffLineKind, &[u8])]) -> io::Result<()> {
        let _: std::fmt::Result = writeln!(
            self.0,
            "@@ -{},{} +{},{} @@",
            header.before_hunk_start, header.before_hunk_len, header.after_hunk_start, header.after_hunk_len
        );
        for (kind, line) in lines {
            self.0.push(match kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Add => '+',
                DiffLineKind::Remove => '-',
            });
            self.0.push_str(String::from_utf8_lossy(line).trim_end_matches(['\n', '\r']));
            self.0.push('\n');
        }
        Ok(())
    }

    fn finish(self) -> String {
        self.0
    }
}

/// For `<Leader>go`: the `origin` remote's URL, the branch (`HEAD` when detached) and `path` in the repository.
pub fn browse(path: &Path) -> Option<(String, String, String)> {
    let repo: Repository = open(parent(path))?;
    let remote: String = repo.config_snapshot().string("remote.origin.url")?.to_string();
    let branch: String = repo.head_name().ok().flatten().map_or_else(|| "HEAD".to_owned(), |n| n.shorten().to_string());
    Some((remote, branch, rela_path(&repo, path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// A fresh repository under the temp dir, built with the git CLI (only tests use it).
    fn fixture(name: &str) -> (PathBuf, impl Fn(&[&str]) -> String) {
        let dir: PathBuf = std::env::temp_dir().join(format!("lim-repo-{name}-{}", std::process::id()));
        let _: io::Result<()> = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let root: PathBuf = dir.clone();
        let git = move |args: &[&str]| {
            let out: std::process::Output = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "-q", "-b", "main"]);
        (dir, git)
    }

    /// Statuses, the file list, history, the graph, a commit's patch, stashes and browse read a real repository.
    #[test]
    fn reads_a_repository() {
        let (dir, git) = fixture("read");
        fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.join("a.txt"), "one\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "first"]);
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        fs::write(dir.join("b.txt"), "b\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "second"]);
        git(&["tag", "v1"]);
        git(&["remote", "add", "origin", "git@github.com:me/proj.git"]);
        fs::write(dir.join("a.txt"), "three\n").unwrap();
        fs::write(dir.join("d.txt"), "d\n").unwrap();
        git(&["add", "d.txt"]);
        fs::write(dir.join("c.txt"), "c\n").unwrap();
        fs::write(dir.join("e.log"), "ignored\n").unwrap();

        assert_eq!(index_text(&dir.join("a.txt")).as_deref(), Some("two\n"));
        assert_eq!(index_text(&dir.join("c.txt")), None, "untracked");
        let root: PathBuf = canonical(&dir);
        let codes: Vec<(String, String)> = status(&dir)
            .into_iter()
            .map(|(p, [x, y])| (p.strip_prefix(&root).unwrap().display().to_string(), format!("{x}{y}")))
            .collect();
        let expected: [(&str, &str); 3] = [("a.txt", " M"), ("c.txt", "??"), ("d.txt", "A ")];
        assert_eq!(codes, expected.map(|(p, c)| (p.to_owned(), c.to_owned())));
        let listed: Vec<PathBuf> = files(&dir).unwrap();
        let names: [&str; 5] = [".gitignore", "a.txt", "b.txt", "c.txt", "d.txt"];
        assert_eq!(listed, names.map(PathBuf::from), "tracked and untracked, not ignored");

        let head: Vec<Commit> = log(&dir, false, None, 10).unwrap();
        assert_eq!(head.len(), 2);
        assert!(head[0].line.ends_with("  second") && head[1].line.ends_with("  first"), "{}", head[0].line);
        let of_b: Vec<Commit> = log(&dir, false, Some(&dir.join("b.txt")), 10).unwrap();
        assert_eq!(of_b.iter().map(|c| c.id).collect::<Vec<ObjectId>>(), [head[0].id]);
        let all: Vec<Commit> = log(&dir, true, None, 10).unwrap();
        assert!(all[0].line.contains(" (HEAD -> main, tag: v1) second"), "{}", all[0].line);
        let rows: Vec<String> = graph(&all);
        assert!(rows.len() == 2 && rows.iter().all(|r| r.starts_with("* ")), "{rows:?}");

        let shown: String = show(&dir, &head[0].hash(), true, true).unwrap();
        for part in [
            &format!("commit {}\n", head[0].id),
            "Author: t <t@t>\n",
            "\n    second\n",
            " a.txt | 2 +-\n",
            " 2 files changed, 2 insertions(+), 1 deletion(-)\n",
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,1 @@\n-one\n+two\n",
            "--- /dev/null\n+++ b/b.txt\n",
        ] {
            assert!(shown.contains(part), "{part:?} missing from:\n{shown}");
        }

        git(&["stash", "-q"]);
        let stashed: Vec<(String, String)> = stashes(&dir);
        assert_eq!(stashed.len(), 1);
        assert!(stashed[0].1.starts_with("stash@{0}: WIP on main"), "{:?}", stashed[0]);
        assert!(show(&dir, &stashed[0].0, false, false).unwrap().contains("-two\n+three\n"));

        let (remote, branch, file): (String, String, String) = browse(&dir.join("a.txt")).unwrap();
        assert_eq!((remote.as_str(), branch.as_str(), file.as_str()), ("git@github.com:me/proj.git", "main", "a.txt"));
        let _: io::Result<()> = fs::remove_dir_all(&dir);
    }

    /// A merge opens a lane for its second parent, which joins back where the branches met.
    #[test]
    fn graph_draws_merges() {
        let id = |n: u8| ObjectId::from_hex(format!("{n:040x}").as_bytes()).unwrap();
        let commit = |n: u8, parents: &[u8], line: &str| Commit {
            id: id(n),
            parents: parents.iter().map(|&p| id(p)).collect(),
            line: line.to_owned(),
        };
        let commits: [Commit; 3] = [commit(3, &[1, 2], "merge"), commit(2, &[1], "topic"), commit(1, &[], "root")];
        assert_eq!(graph(&commits), ["* merge", "|\\", "| * topic", "|/", "* root"]);
    }

    /// Filter and diff drivers a repository's own config names never run, though the attributes select them.
    #[cfg(unix)]
    #[test]
    fn repository_programs_never_run() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, git) = fixture("drivers");
        let marker: PathBuf = dir.join(".git/marker");
        let hook: PathBuf = dir.join(".git/hook.sh");
        fs::write(&hook, format!("#!/bin/sh\necho ran >> '{}'\ncat\n", marker.display())).unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.join(".gitattributes"), "*.txt filter=evil diff=evil\n").unwrap();
        for text in ["one\n", "two\n"] {
            fs::write(dir.join("a.txt"), text).unwrap();
            git(&["add", "."]);
            git(&["commit", "-qm", text]);
        }
        let hook: String = hook.display().to_string();
        for key in ["core.fsmonitor", "filter.evil.clean", "filter.evil.smudge", "diff.evil.textconv"] {
            git(&["config", key, &hook]);
        }
        // Same size, newer mtime: status has to compare the content, through the clean filter.
        fs::write(dir.join("a.txt"), "TWO\n").unwrap();

        assert_eq!(status(&dir).len(), 1);
        assert!(files(&dir).is_some());
        assert_eq!(index_text(&dir.join("a.txt")).as_deref(), Some("two\n"));
        let head: Vec<Commit> = log(&dir, true, Some(&dir.join("a.txt")), 10).unwrap();
        assert!(show(&dir, &head[0].hash(), true, true).unwrap().contains("-one\n+two\n"));
        assert!(!marker.exists(), "a repository program ran");
        git(&["status", "--porcelain"]);
        assert!(marker.exists(), "control: git itself runs it");
        let _: io::Result<()> = fs::remove_dir_all(&dir);
    }
}
