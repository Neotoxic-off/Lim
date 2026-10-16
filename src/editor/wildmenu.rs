//! Command-line completion (blink.cmp's cmdline source / Vim's wildmenu): command names, then the
//! argument each command takes: colorschemes, icon sets, or file paths.

use std::fs;
use std::io;
use std::path::Path;

use super::Editor;
use super::command::COMMANDS;
use crate::buffer::text;

/// Paths listed at most for one completion.
const MAX_PATHS: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CmdCompletion {
    /// Char index in the command line where the completed word starts.
    pub start: usize,
    /// (completion, description)
    pub items: Vec<(String, String)>,
    pub selected: Option<usize>,
}

/// Where the word to complete starts in `line`, and its candidates.
pub fn candidates(line: &str) -> (usize, Vec<(String, String)>) {
    let Some((name, _)): Option<(&str, &str)> = line.split_once(' ') else {
        let items: Vec<(String, String)> = COMMANDS
            .iter()
            .filter(|(c, _)| c.starts_with(line) && *c != line)
            .map(|(c, d)| ((*c).to_owned(), (*d).to_owned()))
            .collect();
        return (0, items);
    };
    let arg_start: usize = text::char_len(name) + 1 + text::char_len(&line[name.len() + 1..])
        - text::char_len(line[name.len() + 1..].trim_start());
    let arg: &str = line[name.len() + 1..].trim_start();
    let items: Vec<(String, String)> = match name {
        "colo" | "colorscheme" => crate::render::palettes::THEMES
            .iter()
            .filter(|t| t.name.starts_with(arg))
            .map(|t| (t.name.to_owned(), if t.light { "light".into() } else { "dark".into() }))
            .collect(),
        "iconset" => [("nerd", "Nerd Font glyphs"), ("unicode", "any font"), ("ascii", "plain ASCII")]
            .iter()
            .filter(|(s, _)| s.starts_with(arg))
            .map(|(s, d)| ((*s).to_owned(), (*d).to_owned()))
            .collect(),
        "e" | "edit" | "w" | "write" | "sp" | "split" | "vs" | "vsplit" | "new" | "vnew" | "wq" | "x" | "xit" => {
            paths(arg)
        }
        _ => Vec::new(),
    };
    (arg_start, items)
}

/// Entries of the directory `arg` points into whose names start with what follows its last `/`.
fn paths(arg: &str) -> Vec<(String, String)> {
    let (dir, prefix): (&str, &str) = match arg.rfind('/') {
        Some(i) => (&arg[..=i], &arg[i + 1..]),
        None => ("", arg),
    };
    let Ok(entries): io::Result<fs::ReadDir> =
        fs::read_dir(if dir.is_empty() { Path::new(".") } else { Path::new(dir) })
    else {
        return Vec::new();
    };
    let mut found: Vec<(bool, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name: String = e.file_name().to_string_lossy().into_owned();
            let hidden_ok: bool = prefix.starts_with('.') || !name.starts_with('.');
            (name.starts_with(prefix) && hidden_ok).then(|| (e.path().is_dir(), name))
        })
        .take(MAX_PATHS)
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found
        .into_iter()
        .map(|(is_dir, name)| {
            (
                format!("{dir}{name}{}", if is_dir { "/" } else { "" }),
                if is_dir { "directory".into() } else { "file".into() },
            )
        })
        .collect()
}

impl Editor {
    /// Recomputes the menu after the command line changed.
    pub(super) fn update_cmd_completion(&mut self) {
        let (start, items): (usize, Vec<(String, String)>) = candidates(&self.cmdline);
        self.cmd_completion =
            (!self.cmdline.is_empty() && !items.is_empty()).then_some(CmdCompletion { start, items, selected: None });
    }

    /// `<Tab>` / `<S-Tab>`: selects the next or previous item and writes it into the command line.
    pub(super) fn cycle_cmd_completion(&mut self, forward: bool) {
        let Some(menu): &mut Option<CmdCompletion> = &mut self.cmd_completion else { return };
        let count: usize = menu.items.len();
        let next: usize = match menu.selected {
            None if forward => 0,
            None => count - 1,
            Some(i) if forward => (i + 1) % count,
            Some(i) => (i + count - 1) % count,
        };
        menu.selected = Some(next);
        let keep: String = self.cmdline.chars().take(menu.start).collect();
        if let Some((item, _)) = menu.items.get(next) {
            self.cmdline = format!("{keep}{item}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_names_and_arguments() {
        let (start, items): (usize, Vec<(String, String)>) = candidates("col");
        assert_eq!(start, 0);
        assert!(items.iter().any(|(c, _)| c == "colorscheme"));
        let (start, items): (usize, Vec<(String, String)>) = candidates("colorscheme cat");
        assert_eq!(start, 12);
        assert_eq!(items.len(), 4);
        let (start, items): (usize, Vec<(String, String)>) = candidates("iconset u");
        assert_eq!((start, items[0].0.as_str()), (8, "unicode"));
        assert!(candidates("e src/edi").1.iter().any(|(p, _)| p == "src/editor/"));
        assert!(candidates("frob x").1.is_empty());
    }
}
