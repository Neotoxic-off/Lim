//! The one table of built-in prefixed keys (`<Leader>…`, `g…`, `]…`, `[…`, `z…`, `<C-w>…`, `Z…`):
//! what each does and how which-key, the cheat sheet and the keymaps picker describe it.

use super::manage::CloseSet;
use super::motion::Motion;
use super::operator::Operator;
use super::picker::PickerKind;
use super::textops::TextOp;
use super::tools::{Generated, Tool};
use super::ui::{CTRL_W, LEADER_B, LEADER_BS, LEADER_F, LEADER_G, LEADER_S, LEADER_T, LEADER_U, LEADER_X};
use super::window::Direction;
use super::{Mode, PickAction};
use crate::config::keys::LEADER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Opens another menu.
    Group(char),
    Motion(Motion),
    Operator(Operator),
    Save,
    Quit,
    QuitAll,
    WriteQuit,
    ForceQuit,
    NewFile,
    CloseBuffer {
        force: bool,
    },
    RenameFile,
    CommentLines,
    ToggleExplorer,
    FocusExplorer,
    Home,
    Cheatsheet,
    Settings,
    Picker(PickerKind),
    ResumePicker,
    GrepWord,
    BufferPick(PickAction),
    CloseBuffers(CloseSet),
    PreviousBuffer,
    SortBuffers(SortBy),
    CycleBuffer {
        forward: bool,
    },
    Shell,
    Tool(&'static [&'static str]),
    GitBrowse,
    Hunk {
        forward: bool,
    },
    HunkEdge {
        last: bool,
    },
    Reference {
        forward: bool,
    },
    Todo {
        forward: bool,
    },
    Toggle(Toggle),
    SessionLoadLast,
    SessionSave,
    SessionSaveDir,
    SessionLoadDir,
    Split {
        vertical: bool,
    },
    CloseWindow,
    OnlyWindow,
    NextWindow,
    Focus(Direction),
    ScrollTo(Align),
    Reselect,
    /// `gn`
    SelectMatch,
    /// `gf`
    OpenFile,
    InsertLast,
    InsertColumnZero,
    JoinRaw,
    OpenUrl,
    /// `<Leader>x…`: a tool's window.
    ToolWindow(Tool),
    /// `<Leader>xk`
    CasePrompt,
    /// `<Leader>x…`: text inserted at the cursor.
    Generate(Generated),
    /// `<Leader>x…`: a text command on the whole buffer.
    BufferText(TextOp),
}

/// `<Leader>u…`: an option to flip or a setting to change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Autopairs,
    Background,
    Autocomplete,
    DismissNotifications,
    SignColumn,
    FoldColumn,
    Indent,
    StatusLine,
    Numbering,
    Notifications,
    Paste,
    References,
    TabLine,
    Urls,
    Syntax,
    Colors,
    Zen,
    IndentGuides,
    MarkdownPreview,
}

/// `<Leader>bs…`: the key buffers are sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Extension,
    RelativePath,
    FullPath,
    Number,
    Modified,
}

/// `zt`, `zz`, `zb`: where the cursor line lands in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Top,
    Middle,
    Bottom,
}

pub struct Binding {
    pub prefix: char,
    /// The key after the prefix; `'\r'` is `<CR>`.
    pub key: char,
    pub action: Action,
    pub desc: &'static str,
}

const fn b(prefix: char, key: char, action: Action, desc: &'static str) -> Binding {
    Binding { prefix, key, action, desc }
}

use Action as A;

/// Menu titles for which-key and the cheat sheet.
pub const MENUS: &[(char, &str)] = &[
    (LEADER, "<Leader>"),
    (LEADER_B, "<Leader>b"),
    (LEADER_BS, "<Leader>bs"),
    (LEADER_F, "<Leader>f"),
    (LEADER_G, "<Leader>g"),
    (LEADER_S, "<Leader>S"),
    (LEADER_T, "<Leader>t"),
    (LEADER_U, "<Leader>u"),
    (LEADER_X, "<Leader>x"),
    ('g', "g"),
    (']', "]"),
    ('[', "["),
    ('z', "z"),
    ('Z', "Z"),
    (CTRL_W, "<C-w>"),
];

/// `AstroNvim`'s defaults (and Vim's `g`, `z`, `Z`, `<C-w>` keys), with `AstroNvim`'s descriptions.
pub const BINDINGS: &[Binding] = &[
    b(LEADER, 'b', A::Group(LEADER_B), "+Buffers"),
    b(LEADER, 'f', A::Group(LEADER_F), "+Find"),
    b(LEADER, 'g', A::Group(LEADER_G), "+Git"),
    b(LEADER, 'S', A::Group(LEADER_S), "+Session"),
    b(LEADER, 't', A::Group(LEADER_T), "+Terminal"),
    b(LEADER, 'u', A::Group(LEADER_U), "+UI/UI Toggles"),
    b(LEADER, 'x', A::Group(LEADER_X), "+Tools"),
    b(LEADER, 'c', A::CloseBuffer { force: false }, "Close buffer"),
    b(LEADER, 'C', A::CloseBuffer { force: true }, "Force close buffer"),
    b(LEADER, 'e', A::ToggleExplorer, "Toggle Explorer"),
    b(LEADER, 'o', A::FocusExplorer, "Toggle Explorer Focus"),
    b(LEADER, 'h', A::Home, "Home Screen"),
    b(LEADER, 'n', A::NewFile, "New File"),
    b(LEADER, 'q', A::Quit, "Quit Window"),
    b(LEADER, 'Q', A::QuitAll, "Exit lim"),
    b(LEADER, 'R', A::RenameFile, "Rename file"),
    b(LEADER, 'w', A::Save, "Save"),
    b(LEADER, '/', A::CommentLines, "Toggle comment line"),
    b(LEADER, '?', A::Cheatsheet, "Cheatsheet"),
    b(LEADER, ',', A::Settings, "Settings"),
    b(LEADER_B, 's', A::Group(LEADER_BS), "+Sort Buffers"),
    b(LEADER_B, 'b', A::BufferPick(PickAction::Go), "Select buffer from tabline"),
    b(LEADER_B, 'c', A::CloseBuffers(CloseSet::Others), "Close all buffers except current"),
    b(LEADER_B, 'C', A::CloseBuffers(CloseSet::All), "Close all buffers"),
    b(LEADER_B, 'd', A::BufferPick(PickAction::Close), "Close buffer from tabline"),
    b(LEADER_B, 'l', A::CloseBuffers(CloseSet::Left), "Close all buffers to the left"),
    b(LEADER_B, 'p', A::PreviousBuffer, "Previous buffer"),
    b(LEADER_B, 'r', A::CloseBuffers(CloseSet::Right), "Close all buffers to the right"),
    b(LEADER_B, '\\', A::BufferPick(PickAction::SplitBelow), "Horizontal split buffer from tabline"),
    b(LEADER_B, '|', A::BufferPick(PickAction::SplitRight), "Vertical split buffer from tabline"),
    b(LEADER_BS, 'e', A::SortBuffers(SortBy::Extension), "By extension"),
    b(LEADER_BS, 'r', A::SortBuffers(SortBy::RelativePath), "By relative path"),
    b(LEADER_BS, 'p', A::SortBuffers(SortBy::FullPath), "By full path"),
    b(LEADER_BS, 'i', A::SortBuffers(SortBy::Number), "By buffer number"),
    b(LEADER_BS, 'm', A::SortBuffers(SortBy::Modified), "By modification"),
    b(LEADER_F, '\r', A::ResumePicker, "Resume previous search"),
    b(LEADER_F, '\'', A::Picker(PickerKind::Marks), "Find marks"),
    b(LEADER_F, 'a', A::Picker(PickerKind::ConfigFiles), "Find lim config files"),
    b(LEADER_F, 'b', A::Picker(PickerKind::Buffers), "Find buffers"),
    b(LEADER_F, 'c', A::GrepWord, "Find word under cursor"),
    b(LEADER_F, 'C', A::Picker(PickerKind::Commands), "Find commands"),
    b(LEADER_F, 'f', A::Picker(PickerKind::Files { all: false }), "Find files"),
    b(LEADER_F, 'F', A::Picker(PickerKind::Files { all: true }), "Find all files"),
    b(LEADER_F, 'g', A::Picker(PickerKind::GitFiles), "Find git files"),
    b(LEADER_F, 'h', A::Cheatsheet, "Find help (cheatsheet)"),
    b(LEADER_F, 'I', A::Picker(PickerKind::IconSets), "Find icon sets"),
    b(LEADER_F, 'k', A::Picker(PickerKind::Keymaps), "Find keymaps"),
    b(LEADER_F, 'l', A::Picker(PickerKind::Lines), "Find lines"),
    b(LEADER_F, 'n', A::Picker(PickerKind::Notifications), "Find notifications"),
    b(LEADER_F, 'o', A::Picker(PickerKind::Recent), "Find old files"),
    b(LEADER_F, 'O', A::Picker(PickerKind::RecentHere), "Find old files (cwd)"),
    b(LEADER_F, 'r', A::Picker(PickerKind::Registers), "Find registers"),
    b(LEADER_F, 's', A::Picker(PickerKind::Smart), "Find buffers/recent/files"),
    b(LEADER_F, 't', A::Picker(PickerKind::Themes), "Find themes"),
    b(LEADER_F, 'T', A::Picker(PickerKind::Todos), "Find TODOs"),
    b(LEADER_F, 'w', A::Picker(PickerKind::Grep { all: false }), "Find words"),
    b(LEADER_F, 'W', A::Picker(PickerKind::Grep { all: true }), "Find words in all files"),
    b(LEADER_G, 'c', A::Picker(PickerKind::GitLog { file: false }), "Git commits (repository)"),
    b(LEADER_G, 'C', A::Picker(PickerKind::GitLog { file: true }), "Git commits (current file)"),
    b(LEADER_G, 'g', A::Picker(PickerKind::GitGraph), "Git graph (all branches)"),
    b(LEADER_G, 'o', A::GitBrowse, "Git browse (open)"),
    b(LEADER_G, 't', A::Picker(PickerKind::GitStatus), "Git status"),
    b(LEADER_G, 'T', A::Picker(PickerKind::GitStash), "Git stash"),
    b(LEADER_S, 'l', A::SessionLoadLast, "Load last session"),
    b(LEADER_S, 's', A::SessionSave, "Save this session"),
    b(LEADER_S, 'S', A::SessionSaveDir, "Save this dirsession"),
    b(LEADER_S, 't', A::SessionSaveDir, "Save this tab's session"),
    b(LEADER_S, 'd', A::Picker(PickerKind::Sessions { dir: false, delete: true }), "Delete a session"),
    b(LEADER_S, 'D', A::Picker(PickerKind::Sessions { dir: true, delete: true }), "Delete a dirsession"),
    b(LEADER_S, 'f', A::Picker(PickerKind::Sessions { dir: false, delete: false }), "Load a session"),
    b(LEADER_S, 'F', A::Picker(PickerKind::Sessions { dir: true, delete: false }), "Load a dirsession"),
    b(LEADER_S, '.', A::SessionLoadDir, "Load current dirsession"),
    b(LEADER_T, 'f', A::Shell, "ToggleTerm float"),
    b(LEADER_T, 'h', A::Shell, "ToggleTerm horizontal split"),
    b(LEADER_T, 'v', A::Shell, "ToggleTerm vertical split"),
    b(LEADER_T, 'n', A::Tool(&["node"]), "ToggleTerm node"),
    b(LEADER_T, 'p', A::Tool(&["python3"]), "ToggleTerm python"),
    b(LEADER_T, 't', A::Tool(&["btm"]), "ToggleTerm btm"),
    b(LEADER_T, 'u', A::Tool(&["gdu"]), "ToggleTerm gdu"),
    b(LEADER_U, 'a', A::Toggle(Toggle::Autopairs), "Toggle autopairs"),
    b(LEADER_U, 'b', A::Toggle(Toggle::Background), "Toggle background"),
    b(LEADER_U, 'c', A::Toggle(Toggle::Autocomplete), "Toggle autocompletion (buffer)"),
    b(LEADER_U, 'C', A::Toggle(Toggle::Autocomplete), "Toggle autocompletion (global)"),
    b(LEADER_U, 'D', A::Toggle(Toggle::DismissNotifications), "Dismiss notifications"),
    b(LEADER_U, 'g', A::Toggle(Toggle::SignColumn), "Toggle signcolumn"),
    b(LEADER_U, '>', A::Toggle(Toggle::FoldColumn), "Toggle foldcolumn"),
    b(LEADER_U, 'i', A::Toggle(Toggle::Indent), "Change indent setting"),
    b(LEADER_U, 'l', A::Toggle(Toggle::StatusLine), "Toggle statusline"),
    b(LEADER_U, 'n', A::Toggle(Toggle::Numbering), "Change line numbering"),
    b(LEADER_U, 'N', A::Toggle(Toggle::Notifications), "Toggle Notifications"),
    b(LEADER_U, 'p', A::Toggle(Toggle::Paste), "Toggle paste mode"),
    b(LEADER_U, 'r', A::Toggle(Toggle::References), "Toggle reference highlighting"),
    b(LEADER_U, 't', A::Toggle(Toggle::TabLine), "Toggle tabline"),
    b(LEADER_U, 'u', A::Toggle(Toggle::Urls), "Toggle URL highlight"),
    b(LEADER_U, 'y', A::Toggle(Toggle::Syntax), "Toggle syntax highlight"),
    b(LEADER_U, 'z', A::Toggle(Toggle::Colors), "Toggle color highlight"),
    b(LEADER_U, 'Z', A::Toggle(Toggle::Zen), "Toggle zen mode"),
    b(LEADER_U, 'm', A::Toggle(Toggle::MarkdownPreview), "Toggle Markdown preview"),
    b(LEADER_U, '|', A::Toggle(Toggle::IndentGuides), "Toggle indent guides"),
    b(LEADER_X, 'c', A::ToolWindow(Tool::Calc), "Calculator window"),
    b(LEADER_X, 'u', A::ToolWindow(Tool::Convert), "Unit converter window"),
    b(LEADER_X, 'b', A::ToolWindow(Tool::Base), "Number bases window"),
    b(LEADER_X, 't', A::ToolWindow(Tool::Time), "Time and timestamps window"),
    b(LEADER_X, 'h', A::ToolWindow(Tool::Color), "Color window: hex, rgb, hsl"),
    b(LEADER_X, 'k', A::CasePrompt, "Case of the word"),
    b(LEADER_X, 'd', A::Generate(Generated::Date), "Insert date"),
    b(LEADER_X, 'D', A::Generate(Generated::DateTime), "Insert date and time"),
    b(LEADER_X, 'i', A::Generate(Generated::Uuid), "Insert UUID"),
    b(LEADER_X, 'p', A::Generate(Generated::Password), "Insert password"),
    b(LEADER_X, 'l', A::Generate(Generated::Lorem), "Insert lorem ipsum"),
    b(LEADER_X, 'j', A::BufferText(TextOp::Json { minify: false }), "Format JSON"),
    b(LEADER_X, 'J', A::BufferText(TextOp::Json { minify: true }), "Minify JSON"),
    b(LEADER_X, 'w', A::BufferText(TextOp::Wc), "Count lines, words, chars"),
    b(LEADER_X, 's', A::BufferText(TextOp::Sha256), "SHA-256 of the buffer"),
    b('g', 'g', A::Motion(Motion::FileStart), "First line"),
    b('g', 'c', A::Operator(Operator::Comment), "Toggle comment (gcc line, gco/gcO add)"),
    b('g', 'e', A::Motion(Motion::WordEndBack { big: false }), "Previous word end"),
    b('g', 'E', A::Motion(Motion::WordEndBack { big: true }), "Previous WORD end"),
    b('g', 'i', A::InsertLast, "Insert where insert mode stopped"),
    b('g', 'I', A::InsertColumnZero, "Insert at column 0"),
    b('g', 'J', A::JoinRaw, "Join lines without spaces"),
    b('g', 'u', A::Operator(Operator::Lower), "Lowercase"),
    b('g', 'U', A::Operator(Operator::Upper), "Uppercase"),
    b('g', '~', A::Operator(Operator::ToggleCase), "Toggle case"),
    b('g', 'v', A::Reselect, "Reselect last selection"),
    b('g', 'n', A::SelectMatch, "Select the next search match (cgn, then . repeats)"),
    b('g', 'f', A::OpenFile, "Open the file under the cursor (path:line too)"),
    b('g', 'x', A::OpenUrl, "Open URL under cursor"),
    b('g', '_', A::Motion(Motion::LastNonBlank), "Last non-blank character"),
    b(']', 'b', A::CycleBuffer { forward: true }, "Next buffer"),
    b(']', 'g', A::Hunk { forward: true }, "Next Git hunk"),
    b(']', 'G', A::HunkEdge { last: true }, "Last Git hunk"),
    b(']', 'r', A::Reference { forward: true }, "Next reference"),
    b(']', 'T', A::Todo { forward: true }, "Next TODO comment"),
    b('[', 'b', A::CycleBuffer { forward: false }, "Previous buffer"),
    b('[', 'g', A::Hunk { forward: false }, "Previous Git hunk"),
    b('[', 'G', A::HunkEdge { last: false }, "First Git hunk"),
    b('[', 'r', A::Reference { forward: false }, "Previous reference"),
    b('[', 'T', A::Todo { forward: false }, "Previous TODO comment"),
    b('z', 'z', A::ScrollTo(Align::Middle), "Cursor line to the middle"),
    b('z', 't', A::ScrollTo(Align::Top), "Cursor line to the top"),
    b('z', 'b', A::ScrollTo(Align::Bottom), "Cursor line to the bottom"),
    b('Z', 'Z', A::WriteQuit, "Write if changed and quit"),
    b('Z', 'Q', A::ForceQuit, "Quit without writing"),
    b(CTRL_W, 'v', A::Split { vertical: true }, "Split window vertically"),
    b(CTRL_W, 's', A::Split { vertical: false }, "Split window"),
    b(CTRL_W, 'c', A::CloseWindow, "Close window"),
    b(CTRL_W, 'q', A::CloseWindow, "Quit window"),
    b(CTRL_W, 'o', A::OnlyWindow, "Close all other windows"),
    b(CTRL_W, 'w', A::NextWindow, "Switch windows"),
    b(CTRL_W, 'h', A::Focus(Direction::Left), "Go to the left window"),
    b(CTRL_W, 'j', A::Focus(Direction::Down), "Go to the down window"),
    b(CTRL_W, 'k', A::Focus(Direction::Up), "Go to the up window"),
    b(CTRL_W, 'l', A::Focus(Direction::Right), "Go to the right window"),
];

pub fn lookup(prefix: char, key: char) -> Option<Action> {
    BINDINGS.iter().find(|b| b.prefix == prefix && b.key == key).map(|b| b.action)
}

pub fn menu_title(prefix: char) -> Option<&'static str> {
    MENUS.iter().find(|(p, _)| *p == prefix).map(|(_, t)| *t)
}

/// How a binding key is written: `<CR>` for Enter, the char otherwise.
pub fn key_name(key: char) -> String {
    if key == '\r' { "<CR>".into() } else { key.to_string() }
}

/// Visual mode reaches `g`, the leader and their sub-menus; the rest are normal-mode only.
pub fn allowed_in(mode: Mode, action: Action) -> bool {
    !matches!(mode, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
        || matches!(action, A::Group(_) | A::Motion(_) | A::Operator(_) | A::CommentLines | A::OpenUrl | A::SelectMatch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_keys_and_every_menu_titled() {
        for (i, a) in BINDINGS.iter().enumerate() {
            assert!(BINDINGS[i + 1..].iter().all(|b| (b.prefix, b.key) != (a.prefix, a.key)), "duplicate {:?}", a.key);
            assert!(menu_title(a.prefix).is_some());
            if let A::Group(sub) = a.action {
                assert!(menu_title(sub).is_some());
            }
        }
    }
}
