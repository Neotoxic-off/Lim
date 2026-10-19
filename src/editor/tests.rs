//! Key-sequence tests: type like a user, check the text.

use std::path::PathBuf;

use super::*;

fn editor(text: &str) -> Editor {
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    ed.buffers[0] = Buffer::from_text(None, text);
    ed
}

fn keys(ed: &mut Editor, typed: &str) {
    for c in typed.chars() {
        ed.handle(match c {
            '\x1b' => Key::Esc,
            '\n' => Key::Enter,
            '\x08' => Key::Backspace,
            c => Key::Char(c),
        });
    }
    ed.refresh_completion();
}

fn text(ed: &Editor) -> String {
    ed.buf().lines().join("\n")
}

#[test]
fn operators_undo_and_paste() {
    let mut ed: Editor = editor("foo bar baz\nqux");
    keys(&mut ed, "dw");
    assert_eq!(text(&ed), "bar baz\nqux");
    keys(&mut ed, "wcwX\x1b");
    assert_eq!(text(&ed), "bar X\nqux");
    keys(&mut ed, "ddp");
    assert_eq!(text(&ed), "qux\nbar X");
    keys(&mut ed, "uu");
    assert_eq!(text(&ed), "bar X\nqux");
    keys(&mut ed, "u");
    assert_eq!(text(&ed), "bar baz\nqux");
    ed.handle(Key::Ctrl('r'));
    assert_eq!(text(&ed), "bar X\nqux");
    keys(&mut ed, "ggVjd");
    assert_eq!(text(&ed), "");
    assert!(ed.buf().is_dirty());
}

#[test]
fn counts_finds_and_visual() {
    let mut ed: Editor = editor("a b c d e");
    keys(&mut ed, "2dw");
    assert_eq!(text(&ed), "c d e");
    keys(&mut ed, "dt ");
    assert_eq!(text(&ed), " d e");
    keys(&mut ed, "$vhhy0P");
    assert_eq!(text(&ed), "d e d e");
    keys(&mut ed, "3x"); // `P` left the cursor on the last pasted char
    assert_eq!(text(&ed), "d  e");
    keys(&mut ed, "0~");
    assert_eq!(text(&ed), "D  e");
}

#[test]
fn insert_autopairs_and_indent() {
    let mut ed: Editor = editor("");
    keys(&mut ed, "ifn(\x1b");
    assert_eq!(text(&ed), "fn()");
    keys(&mut ed, "A {\nx\x1b");
    assert_eq!(text(&ed), "fn() {\n    x\n}");
    keys(&mut ed, "o\x1b");
    assert_eq!(text(&ed), "fn() {\n    x\n\n}");
    keys(&mut ed, "ggA\"\x08\x1b");
    assert_eq!(text(&ed), "fn() {\n    x\n\n}");
    keys(&mut ed, "ggi'\x1b"); // no pair before a word char
    assert_eq!(text(&ed), "'fn() {\n    x\n\n}");
    keys(&mut ed, "O'\x1b");
    assert_eq!(text(&ed), "''\n'fn() {\n    x\n\n}");
}

#[test]
fn search_and_commands() {
    let mut ed: Editor = editor("alpha beta\nBeta\nbeta");
    keys(&mut ed, "/beta\n");
    assert_eq!(ed.buf().cursor, Pos::new(0, 6));
    keys(&mut ed, "n");
    assert_eq!(ed.buf().cursor, Pos::new(1, 0));
    keys(&mut ed, "N");
    assert_eq!(ed.buf().cursor, Pos::new(0, 6));
    keys(&mut ed, "/zzz\n");
    assert_eq!(ed.message, Some(Message::Error("E486: Pattern not found: zzz".into())));
    keys(&mut ed, ":3\n");
    assert_eq!(ed.buf().cursor.line, 2);
    keys(&mut ed, "x:q\n");
    assert!(!ed.quit);
    keys(&mut ed, ":q!\n");
    assert!(ed.quit);
}

#[test]
fn vertical_motion_keeps_column() {
    let mut ed: Editor = editor("long line here\nab\nanother long one");
    keys(&mut ed, "$jj");
    assert_eq!(ed.buf().cursor, Pos::new(2, 15));
    keys(&mut ed, "0wk");
    assert_eq!(ed.buf().cursor, Pos::new(1, 1));
    keys(&mut ed, "k");
    assert_eq!(ed.buf().cursor, Pos::new(0, 8));
}

#[test]
fn dot_repeat_and_macros() {
    let mut ed: Editor = editor("a1\na2\na3\na4");
    keys(&mut ed, "A;\x1bj.");
    assert_eq!(text(&ed), "a1;\na2;\na3\na4");
    keys(&mut ed, "jddj.");
    assert_eq!(text(&ed), "a1;\na2;");
    let mut ed: Editor = editor("x\ny\nz");
    keys(&mut ed, "qaI- \x1bjq2@a");
    assert_eq!(text(&ed), "- x\n- y\n- z");
    assert_eq!(ed.recording(), None);
}

#[test]
fn text_objects_star_and_percent() {
    let mut ed: Editor = editor("call(one, \"two\") end");
    keys(&mut ed, "fodi(");
    assert_eq!(text(&ed), "call() end");
    keys(&mut ed, "u0f\"ci\"X\x1b");
    assert_eq!(text(&ed), "call(one, \"X\") end");
    keys(&mut ed, "0%");
    assert_eq!(ed.buf().cursor.col, 13);
    keys(&mut ed, "$daw"); // no trailing blank: `aw` takes the leading one
    assert_eq!(text(&ed), "call(one, \"X\")");
    let mut ed: Editor = editor("foo food\nfoo");
    keys(&mut ed, "*");
    assert_eq!(ed.buf().cursor, Pos::new(1, 0));
    keys(&mut ed, "viwy$p");
    assert_eq!(text(&ed), "foo food\nfoofoo");
}

#[test]
fn comment_toggle() {
    let mut ed: Editor = editor("");
    ed.buffers[0] = Buffer::from_text(Some("x.rs".into()), "fn a() {\n    b();\n\n    c();\n}");
    keys(&mut ed, "jgcj");
    assert_eq!(text(&ed), "fn a() {\n    // b();\n\n    c();\n}");
    keys(&mut ed, "Vjjgc");
    assert_eq!(text(&ed), "fn a() {\n    // // b();\n\n    // c();\n}");
    keys(&mut ed, "gg2 /");
    assert_eq!(text(&ed), "// fn a() {\n//     // // b();\n\n    // c();\n}");
    keys(&mut ed, "uuggjgcc");
    assert_eq!(text(&ed), "fn a() {\n    b();\n\n    c();\n}");

    // Block-comment-only languages wrap each line.
    ed.buffers[0] = Buffer::from_text(Some("x.html".into()), "<ul>\n  <li>a</li>\n</ul>");
    keys(&mut ed, "ggjgcc");
    assert_eq!(text(&ed), "<ul>\n  <!-- <li>a</li> -->\n</ul>");
    keys(&mut ed, "gcc");
    assert_eq!(text(&ed), "<ul>\n  <li>a</li>\n</ul>");
    ed.buffers[0] = Buffer::from_text(Some("x.css".into()), "a { }\nb { }");
    keys(&mut ed, "gggcj");
    assert_eq!(text(&ed), "/* a { } */\n/* b { } */");
    keys(&mut ed, "gcj");
    assert_eq!(text(&ed), "a { }\nb { }");
}

#[test]
fn dot_repeats_replace_and_recover_repairs() {
    let mut ed: Editor = editor("ab\ncd");
    keys(&mut ed, "r.j.");
    assert_eq!(text(&ed), ".b\n.d");
    keys(&mut ed, "..uuu"); // each `.` is its own undo step
    assert_eq!(text(&ed), ".b\ncd");
    ed.buf_mut().cursor = Pos::new(99, 99);
    ed.buf_mut().top = 50;
    ed.mode = Mode::Insert;
    ed.recover("test");
    assert_eq!((ed.buf().cursor, ed.buf().top, ed.mode), (Pos::new(1, 1), 1, Mode::Normal));
    assert!(ed.message_text().starts_with("internal error recovered (test); unsaved buffers copied to "));
    let backup: PathBuf = ed.message_text().rsplit(' ').next().map(PathBuf::from).unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), ".b\ncd\n");
    std::fs::remove_file(backup).unwrap();
}

#[test]
fn buffers_and_windows() {
    let mut ed: Editor = editor("one");
    ed.buffers.push(Buffer::from_text(Some("beta.rs".into()), "two"));
    ed.buffers.push(Buffer::from_text(Some("gamma.rs".into()), "three"));
    keys(&mut ed, "]b");
    assert_eq!(ed.current, 1);
    keys(&mut ed, ">b");
    assert_eq!((ed.current, ed.buf().name()), (2, "beta.rs".to_owned()));
    assert_eq!(ed.pick_labels(), vec!['a', 'g', 'b']);
    keys(&mut ed, " bbg");
    assert_eq!(ed.buf().name(), "gamma.rs");
    keys(&mut ed, " bp");
    assert_eq!(ed.buf().name(), "beta.rs");
    keys(&mut ed, "|");
    assert_eq!((ed.windows.len(), ed.active), (2, 1));
    keys(&mut ed, "[b");
    assert_eq!(ed.windows[1].buffer, 1);
    assert_eq!(ed.windows[0].buffer, 2);
    ed.handle(Key::Ctrl('h'));
    assert_eq!((ed.active, ed.buf().name()), (0, "beta.rs".to_owned()));
    keys(&mut ed, ":q\n");
    assert_eq!((ed.windows.len(), ed.quit), (1, false));
    keys(&mut ed, " bc");
    assert_eq!(ed.buffers.len(), 1);
    assert_eq!(ed.windows[0].buffer, 0);
    keys(&mut ed, ":q\n");
    assert!(ed.quit);
}

/// Plugin commands complete on the command line; tests may not spawn, so running one stops at that guard.
#[test]
fn user_commands_complete_and_run() {
    let (config, warnings): (Config, Vec<String>) = Config::from_str(
        r#"{ "commands": { "Sort": { "run": ["sort"], "output": "replace", "desc": "Sort lines" } } }"#,
    );
    assert_eq!(warnings, Vec::<String>::new());
    let mut ed: Editor = editor("b\na");
    ed.configure(&config);
    keys(&mut ed, ":So");
    assert_eq!(ed.cmd_completion.as_ref().map(|c| c.items.clone()), Some(vec![("Sort".into(), "Sort lines".into())]));
    keys(&mut ed, "rt\n");
    assert_eq!(ed.message, Some(Message::Error("Sort: cannot run programs here".into())));
    keys(&mut ed, ":Nope\n");
    assert_eq!(ed.message, Some(Message::Error("E492: Not an editor command: Nope".into())));
    assert_eq!(text(&ed), "b\na");
}

#[test]
fn user_keymaps_and_leader() {
    let (config, warnings): (Config, Vec<String>) = Config::from_str(
        r#"{ "options": { "mapleader": "," },
             "keymaps": { "i": { "jk": "<Esc>" }, "n": { "<Leader>d": "dd", "Q": false, "H": "^" } } }"#,
    );
    assert_eq!(warnings, Vec::<String>::new());
    let mut ed: Editor = editor("  abc\nxyz");
    ed.configure(&config);
    keys(&mut ed, "$H");
    assert_eq!(ed.buf().cursor.col, 2);
    keys(&mut ed, "Ajk");
    assert_eq!((ed.mode, text(&ed).as_str()), (Mode::Normal, "  abc\nxyz"));
    keys(&mut ed, "Aj");
    assert_eq!(text(&ed), "  abc\nxyz"); // `j` waits for a possible `k`
    ed.typeahead_at -= Duration::from_secs(1);
    ed.tick();
    assert_eq!(text(&ed), "  abcj\nxyz");
    keys(&mut ed, "x\x1b,d");
    assert_eq!(text(&ed), "xyz");
    keys(&mut ed, "Q");
    assert!(!ed.quit);
    keys(&mut ed, " ");
    assert_eq!(ed.buf().cursor.col, 0); // space is no longer the leader
}

#[test]
fn picker_explorer_and_dashboard() {
    let mut ed: Editor = editor("alpha\nbeta\ngamma");
    ed.buffers.push(Buffer::from_text(Some("other.rs".into()), "x"));
    keys(&mut ed, " fl");
    assert_eq!(ed.mode, Mode::Picker);
    keys(&mut ed, "gm\n");
    assert_eq!((ed.mode, ed.buf().cursor.line), (Mode::Normal, 2));
    keys(&mut ed, " fbothe");
    assert_eq!(ed.picker.as_ref().map(|p| p.matches.len()), Some(1));
    ed.handle(Key::Enter);
    assert_eq!(ed.buf().name(), "other.rs");
    keys(&mut ed, " f\n");
    assert_eq!(ed.picker.as_ref().map(|p| p.query.as_str()), Some("othe"));
    ed.handle(Key::Esc);

    let dir: PathBuf = std::env::temp_dir().join(format!("lim-tree-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/inner.txt"), "inside").unwrap();
    std::fs::write(dir.join("top.txt"), "top").unwrap();
    ed.explorer = Some(Explorer::new(dir.clone()));
    ed.mode = Mode::Explorer;
    let names =
        |ed: &Editor| ed.explorer.as_ref().map(|e| e.entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
    assert_eq!(names(&ed), Some(vec!["sub".to_owned(), "top.txt".to_owned()]));
    keys(&mut ed, "ljl");
    assert_eq!(names(&ed), Some(vec!["sub".to_owned(), "inner.txt".to_owned(), "top.txt".to_owned()]));
    assert_eq!((ed.mode, ed.buf().name()), (Mode::Normal, "inner.txt".to_owned()));
    std::fs::remove_dir_all(dir).unwrap();

    let mut ed: Editor = Editor::new(&[]);
    assert!(ed.dashboard);
    keys(&mut ed, "ix");
    assert_eq!(text(&ed), "");
    keys(&mut ed, "n");
    assert!(!ed.dashboard);
}

#[test]
fn recursive_macro_is_bounded() {
    let mut ed: Editor = editor("x\ny");
    keys(&mut ed, "qaj99@aq@a");
    assert_eq!(ed.buf().cursor.line, 1);
    assert_eq!(ed.message_text(), "Replay stopped: too many keys");
}

#[test]
fn registers_marks_and_search_back() {
    let mut ed: Editor = editor("one\ntwo\nthree");
    keys(&mut ed, "\"ayyj\"byy\"_ddgg\"bp");
    assert_eq!(text(&ed), "one\ntwo\nthree");
    keys(&mut ed, "\"ap");
    assert_eq!(text(&ed), "one\ntwo\none\nthree");
    keys(&mut ed, "Gma");
    keys(&mut ed, "gg'a");
    assert_eq!(ed.buf().cursor.line, 3);
    keys(&mut ed, "''");
    assert_eq!(ed.buf().cursor.line, 0);
    keys(&mut ed, "G?on\n");
    assert_eq!(ed.buf().cursor, Pos::new(2, 0));
    keys(&mut ed, "n");
    assert_eq!(ed.buf().cursor, Pos::new(0, 0));
    ed.handle(Key::Ctrl('o'));
    assert_eq!(ed.buf().cursor.line, 2);
}

#[test]
fn more_motions_and_case() {
    let mut ed: Editor = editor("a.b c-d e,f\nx = 41;");
    keys(&mut ed, "W");
    assert_eq!(ed.buf().cursor.col, 4);
    keys(&mut ed, "E");
    assert_eq!(ed.buf().cursor.col, 6);
    keys(&mut ed, "0f-;");
    assert_eq!(ed.buf().cursor.col, 5);
    keys(&mut ed, "0fe,");
    assert_eq!(ed.buf().cursor.col, 8);
    keys(&mut ed, "0gUiw");
    assert_eq!(text(&ed).lines().next(), Some("A.b c-d e,f"));
    keys(&mut ed, "g~~");
    assert_eq!(text(&ed).lines().next(), Some("a.B C-D E,F"));
    keys(&mut ed, "j");
    ed.handle(Key::Ctrl('a'));
    assert_eq!(text(&ed).lines().nth(1), Some("x = 42;"));
    keys(&mut ed, "5");
    ed.handle(Key::Ctrl('x'));
    assert_eq!(text(&ed).lines().nth(1), Some("x = 37;"));
    keys(&mut ed, "0Rab\x08z\x1b");
    assert_eq!(text(&ed).lines().nth(1), Some("az= 37;"));
}

#[test]
fn ex_ranges_and_substitute() {
    let mut ed: Editor = editor("foo bar foo\nfoo\nbaz");
    keys(&mut ed, ":s/foo/X/\n");
    assert_eq!(text(&ed), "X bar foo\nfoo\nbaz");
    keys(&mut ed, ":%s/foo/<&>/g\n");
    assert_eq!(text(&ed), "X bar <foo>\n<foo>\nbaz");
    keys(&mut ed, ":2,3d\n");
    assert_eq!(text(&ed), "X bar <foo>");
    let mut ed: Editor = editor("a\nb\nc\nd");
    keys(&mut ed, "jVj:>\n");
    assert_eq!(text(&ed), "a\n    b\n    c\nd");
    keys(&mut ed, ":$\n");
    assert_eq!(ed.buf().cursor.line, 3);
    keys(&mut ed, "ggjVj\x1bgg");
    keys(&mut ed, "gvd");
    assert_eq!(text(&ed), "a\nd");
}

#[test]
fn visual_block() {
    let mut ed: Editor = editor("abcd\nefgh\nijkl");
    ed.handle(Key::Char('l'));
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jld");
    assert_eq!(text(&ed), "ad\neh\nijkl");
    keys(&mut ed, "ggP");
    assert_eq!(text(&ed), "bcad\nfgeh\nijkl");
    keys(&mut ed, "gg");
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jjI> \x1b");
    assert_eq!(text(&ed), "> bcad\n> fgeh\n> ijkl");
    // Columns are screen cells: a tab and 4 spaces line up (tabstop 4), a CJK char is two cells wide.
    let mut ed: Editor = editor("\tab\n    cd");
    ed.handle(Key::Char('l'));
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jfcd");
    assert_eq!(text(&ed), "\tb\n    d");
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jfdI|\x1b");
    assert_eq!(text(&ed), "\t|b\n    |d");
    let mut ed: Editor = editor("日本\nabcd");
    ed.handle(Key::Char('l'));
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jld");
    assert_eq!(text(&ed), "日\nab");
}

#[test]
fn insert_extras_and_astro_keys() {
    let mut ed: Editor = editor("");
    ed.buffers[0] = Buffer::from_text(Some("x.rs".into()), "let foo = 1;\nfoo + foo");
    keys(&mut ed, "yiwo");
    ed.handle(Key::Ctrl('r'));
    keys(&mut ed, "\"\x1b");
    assert_eq!(text(&ed).lines().nth(1), Some("let"));
    keys(&mut ed, "ggwgco");
    assert_eq!(ed.mode, Mode::Insert);
    keys(&mut ed, "hi\x1b");
    assert_eq!(text(&ed).lines().nth(1), Some("// hi"));
    keys(&mut ed, "ggw]r");
    assert_eq!(ed.buf().cursor, Pos::new(3, 0));
    keys(&mut ed, "]r");
    assert_eq!(ed.buf().cursor, Pos::new(3, 6));
    keys(&mut ed, "A ");
    ed.handle(Key::Ctrl('o'));
    keys(&mut ed, "0");
    assert_eq!((ed.mode, ed.buf().cursor.col), (Mode::Insert, 0));
}

#[test]
fn themes_cheatsheet_prompt_and_layout() {
    let mut ed: Editor = editor("a");
    ed.set_colorscheme("dracula", false);
    assert_eq!(ed.colorscheme, "dracula");
    keys(&mut ed, " ub");
    assert_eq!(ed.colorscheme, "astrolight");
    keys(&mut ed, " ft");
    keys(&mut ed, "nord");
    assert_eq!(ed.colorscheme, "nord");
    ed.handle(Key::Esc);
    assert_eq!(ed.colorscheme, "astrolight");
    ed.set_colorscheme("nope", false);
    assert!(ed.message_text().contains("E185"));

    ed.handle(Key::F(1));
    assert_eq!(ed.mode, Mode::Cheatsheet);
    keys(&mut ed, "macro");
    let cards: Vec<Card> = ed.cheatsheet_cards();
    assert!(cards.iter().all(|c| {
        c.rows
            .iter()
            .all(|(k, d)| (k.clone() + d).to_lowercase().contains("macro") || c.title.to_lowercase().contains("macro"))
    }));
    assert!(!cards.is_empty());
    ed.handle(Key::Esc);

    let path: PathBuf = std::env::temp_dir().join(format!("lim-rename-{}.txt", std::process::id()));
    let target: PathBuf = path.with_extension("md");
    std::fs::write(&path, "x").unwrap();
    ed.buffers[0] = Buffer::open(&path).unwrap();
    keys(&mut ed, " R");
    assert_eq!(ed.mode, Mode::Prompt);
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, &format!("{}\n", target.display()));
    assert!(target.exists() && !path.exists());
    assert_eq!(ed.buf().path.as_deref(), Some(target.as_path()));
    std::fs::remove_file(target).unwrap();

    keys(&mut ed, "|");
    ed.handle(Key::CtrlRight);
    assert!(matches!(&ed.layout, Layout::Split { adjust, .. } if adjust == &vec![-2, 2]));
    keys(&mut ed, " uZ");
    assert!(ed.zen);
}

#[test]
fn marks_and_jumps_follow_edits() {
    let mut ed: Editor = editor("a\nb\nc\nd\ne");
    keys(&mut ed, "3Gmagg");
    keys(&mut ed, "OX\x1bOY\x1b");
    keys(&mut ed, "'a");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "c");
    keys(&mut ed, "ggdd'a");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "c");
    keys(&mut ed, "u'a");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "c");
    // A jump recorded on "d" still lands on "d" after lines above it are deleted.
    keys(&mut ed, "/d\nG");
    keys(&mut ed, "ggdj");
    ed.handle(Key::Ctrl('o'));
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "e");
    ed.handle(Key::Ctrl('o'));
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "d");
    // Deleting the marked line drops the mark.
    keys(&mut ed, "/c\nmadd'a");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "d");
}

#[test]
fn buffer_word_completion() {
    let mut ed: Editor = editor("restock result\n");
    keys(&mut ed, "Gores");
    let items: Option<Vec<String>> = ed.completion.as_ref().map(|c| c.items.clone());
    assert_eq!(items, Some(vec!["result".to_owned(), "restock".to_owned()]));
    ed.handle(Key::Tab);
    ed.handle(Key::Tab);
    ed.handle(Key::Enter);
    assert_eq!(ed.buf().line(1), "restock");
    assert!(ed.completion.is_none());
    // No selection: Enter is a plain newline.
    keys(&mut ed, " re\n");
    assert_eq!(text(&ed), "restock result\nrestock re\n");
    keys(&mut ed, "xyz");
    assert!(ed.completion.is_none());
    keys(&mut ed, "\x1b");
    ed.options.completion = false;
    keys(&mut ed, "ores");
    assert!(ed.completion.is_none());
    ed.handle(Key::Ctrl(' '));
    assert!(ed.completion.is_some());
}

#[test]
fn ctrl_w_w_cycles_windows() {
    let mut ed: Editor = editor("a");
    keys(&mut ed, "|");
    assert_eq!(ed.active, 1);
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Char('w'));
    assert_eq!(ed.active, 0);
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Ctrl('w'));
    assert_eq!(ed.active, 1);
    // With the file tree open, the cycle goes through it.
    ed.explorer = Some(Explorer::new(std::env::temp_dir()));
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Char('w'));
    assert_eq!(ed.mode, Mode::Explorer);
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Char('w'));
    assert_eq!((ed.mode, ed.active), (Mode::Normal, 0));
}

#[test]
fn ctrl_w_arrow_focuses_window() {
    let mut ed: Editor = editor("a");
    keys(&mut ed, "|");
    assert_eq!(ed.active, 1);
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Left);
    assert_eq!(ed.active, 0);
    ed.handle(Key::Ctrl('w'));
    ed.handle(Key::Right);
    assert_eq!(ed.active, 1);
}

#[test]
fn upgrade_from_a_command_or_the_leader() {
    // Tests may not start programs, so both paths reach the update and stop at that guard.
    for typed in [":upgrade\n", " pu"] {
        let mut ed: Editor = editor("a");
        keys(&mut ed, typed);
        assert_eq!(ed.message, Some(Message::Error("upgrade: cannot run programs here".into())), "{typed}");
    }
}

#[test]
fn command_line_completion() {
    let mut ed: Editor = editor("a");
    keys(&mut ed, ":sett");
    assert!(ed.cmd_completion.is_some());
    ed.handle(Key::Tab);
    assert_eq!(ed.cmdline, "settings");
    ed.handle(Key::Esc);
    keys(&mut ed, ":b");
    ed.handle(Key::Tab);
    ed.handle(Key::Tab);
    ed.handle(Key::BackTab);
    assert_eq!(ed.cmdline, "base32");
    ed.handle(Key::Esc);
    assert!(ed.cmd_completion.is_none());
}

#[test]
fn leader_u_toggles() {
    type Flag = fn(&Options) -> bool;
    let mut ed: Editor = editor("a");
    let flips: [(&str, Flag); 13] = [
        ("a", |o| o.autopairs),
        ("g", |o| o.signcolumn),
        ("l", |o| o.laststatus),
        ("N", |o| o.notifications),
        ("t", |o| o.showtabline),
        ("u", |o| o.highlighturl),
        ("y", |o| o.syntax),
        ("r", |o| o.wordhighlight),
        ("c", |o| o.completion),
        ("C", |o| o.completion),
        ("z", |o| o.colorhighlight),
        (">", |o| o.foldcolumn),
        ("|", |o| o.indentguides),
    ];
    for (key, get) in flips {
        let before: bool = get(&ed.options);
        keys(&mut ed, &format!(" u{key}"));
        assert_ne!(get(&ed.options), before, "<Leader>u{key}");
        assert!(ed.message_text().ends_with(if before { "disabled" } else { "enabled" }), "<Leader>u{key}");
    }
    keys(&mut ed, " uD");
    assert_eq!(ed.message_text(), "");

    ed.options.autopairs = true;
    keys(&mut ed, " up");
    assert!(!ed.options.autopairs && !ed.options.autoindent);
    keys(&mut ed, " up");
    assert!(ed.options.autopairs && ed.options.autoindent);

    let start: (bool, bool) = (ed.options.number, ed.options.relativenumber);
    let mut seen: Vec<(bool, bool)> = Vec::new();
    for _ in 0..4 {
        keys(&mut ed, " un");
        seen.push((ed.options.number, ed.options.relativenumber));
    }
    assert_eq!(seen.last(), Some(&start), "four presses cycle back");
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 4);
}

#[test]
fn leader_ui_sets_indent() {
    let mut ed: Editor = editor("a");
    ed.options.expandtab = true;
    keys(&mut ed, " ui");
    assert_eq!(ed.prompt.as_ref().map(|p| p.text.clone()), Some(ed.options.indent_width().to_string()));
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, "-8\n");
    assert_eq!((ed.options.expandtab, ed.options.tabstop, ed.options.shiftwidth), (false, 8, 8));
    keys(&mut ed, " ui");
    assert_eq!(ed.prompt.as_ref().map(|p| p.text.as_str()), Some("-8"));
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, "99\n");
    assert_eq!((ed.options.expandtab, ed.options.tabstop), (true, 16), "clamped to 16");
    keys(&mut ed, " ui");
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, "wide\n");
    assert_eq!(ed.message_text(), "Invalid indent: wide");
}

#[test]
fn sort_buffers_keeps_focus() {
    let mut ed: Editor = editor("");
    ed.buffers = ["b.rs", "a.txt", "c.md"].map(|n| Buffer::from_text(Some(n.into()), n)).into();
    ed.current = 0;
    let names = |ed: &Editor| ed.buffers.iter().map(Buffer::name).collect::<Vec<_>>();
    keys(&mut ed, " bse");
    assert_eq!(names(&ed), ["c.md", "b.rs", "a.txt"]);
    assert_eq!(ed.buf().name(), "b.rs");
    assert_eq!(ed.windows[0].buffer, ed.current);
    keys(&mut ed, " bsp");
    assert_eq!(names(&ed), ["a.txt", "b.rs", "c.md"]);
    keys(&mut ed, " bsr");
    assert_eq!(names(&ed), ["a.txt", "b.rs", "c.md"]);
    keys(&mut ed, " bsi");
    assert_eq!(names(&ed), ["a.txt", "b.rs", "c.md"]);
    keys(&mut ed, " bsm");
    assert_eq!(names(&ed), ["a.txt", "b.rs", "c.md"], "unsaved files tie and keep their order");
    assert_eq!(ed.buf().name(), "b.rs");
}

#[test]
fn buffer_cycling_wraps_with_counts() {
    let mut ed: Editor = editor("");
    ed.buffers = ["0", "1", "2"].map(|n| Buffer::from_text(Some(n.into()), n)).into();
    keys(&mut ed, "2]b");
    assert_eq!(ed.current, 2);
    keys(&mut ed, "]b");
    assert_eq!(ed.current, 0);
    keys(&mut ed, "[b");
    assert_eq!(ed.current, 2);
    ed.handle(Key::Tab);
    assert_eq!(ed.current, 0);
    keys(&mut ed, "2");
    ed.handle(Key::BackTab);
    assert_eq!(ed.current, 1);
    keys(&mut ed, "7]b");
    assert_eq!(ed.current, 2, "counts larger than the list wrap");
    keys(&mut ed, ":bn\n");
    assert_eq!(ed.current, 0);
    keys(&mut ed, ":bp\n");
    assert_eq!(ed.current, 2);
}

#[test]
fn scrolling_moves_view_and_cursor() {
    let lines: Vec<String> = (0..100).map(|i| i.to_string()).collect();
    let mut ed: Editor = editor(&lines.join("\n"));
    ed.view_height = 10;
    keys(&mut ed, "5");
    ed.handle(Key::Ctrl('e'));
    assert_eq!((ed.buf().top, ed.buf().cursor.line), (5, 5), "the cursor is dragged into view");
    keys(&mut ed, "2");
    ed.handle(Key::Ctrl('y'));
    assert_eq!((ed.buf().top, ed.buf().cursor.line), (3, 5));
    keys(&mut ed, "9");
    ed.handle(Key::Ctrl('y'));
    assert_eq!(ed.buf().top, 0);
    keys(&mut ed, "500");
    ed.handle(Key::Ctrl('e'));
    assert_eq!((ed.buf().top, ed.buf().cursor.line), (99, 99));
    keys(&mut ed, "51Gzt");
    assert_eq!(ed.buf().top, 50);
    keys(&mut ed, "zb");
    assert_eq!(ed.buf().top, 41);
    keys(&mut ed, "zz");
    assert_eq!(ed.buf().top, 45);
}

#[test]
fn ctrl_a_and_ctrl_x_count() {
    let mut ed: Editor = editor("x 41 y");
    ed.handle(Key::Ctrl('a'));
    assert_eq!(text(&ed), "x 42 y");
    keys(&mut ed, "05");
    ed.handle(Key::Ctrl('x'));
    assert_eq!(text(&ed), "x 37 y");
    keys(&mut ed, "50");
    ed.handle(Key::Ctrl('x'));
    assert_eq!(text(&ed), "x -13 y");
}

#[test]
fn prompt_line_editing() {
    let mut ed: Editor = editor("a");
    keys(&mut ed, " R");
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, "abc");
    ed.handle(Key::Left);
    ed.handle(Key::Left);
    keys(&mut ed, "X");
    let prompt = |ed: &Editor| ed.prompt.as_ref().map(|p| (p.text.clone(), p.cursor));
    assert_eq!(prompt(&ed), Some(("aXbc".into(), 2)));
    for key in [Key::Home, Key::Delete, Key::End, Key::Backspace, Key::Right, Key::Ctrl('a'), Key::Ctrl('e')] {
        ed.handle(key);
    }
    assert_eq!(prompt(&ed), Some(("Xb".into(), 2)));
    ed.handle(Key::Esc);
    assert_eq!((ed.mode, ed.prompt.is_none(), ed.buf().path.is_none()), (Mode::Normal, true, true));
    keys(&mut ed, " R");
    ed.handle(Key::Ctrl('u'));
    keys(&mut ed, "  \n");
    assert!(ed.buf().path.is_none(), "blank input does nothing");
    keys(&mut ed, " R");
    keys(&mut ed, "named.rs\n");
    assert_eq!(ed.buf().path.as_deref(), Some(Path::new("named.rs")));
}

#[test]
fn tree_prompts_change_files() {
    let dir: PathBuf = std::env::temp_dir().join(format!("lim-tree-ops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.txt"), "f").unwrap();
    let mut ed: Editor = editor("");
    ed.explorer = Some(Explorer::new(dir.clone()));
    ed.mode = Mode::Explorer;
    let select = |ed: &mut Editor, name: &str| {
        let tree: &mut Explorer = ed.explorer.as_mut().unwrap();
        tree.cursor = tree.entries.iter().position(|e| e.name == name).unwrap();
    };
    let answer = |ed: &mut Editor, key: &str, input: &str| {
        keys(ed, key);
        ed.handle(Key::Ctrl('u'));
        keys(ed, &format!("{input}\n"));
        assert_eq!(ed.mode, Mode::Explorer, "{}", ed.message_text());
    };

    answer(&mut ed, "a", "new.txt");
    assert!(dir.join("new.txt").is_file());
    answer(&mut ed, "A", "sub");
    assert!(dir.join("sub").is_dir());
    select(&mut ed, "f.txt");
    answer(&mut ed, "r", "g.txt");
    assert!(dir.join("g.txt").is_file() && !dir.join("f.txt").exists());
    select(&mut ed, "g.txt");
    answer(&mut ed, "c", "sub/copy.txt");
    assert_eq!(std::fs::read_to_string(dir.join("sub/copy.txt")).unwrap(), "f");
    select(&mut ed, "g.txt");
    answer(&mut ed, "m", "moved.txt");
    assert!(dir.join("moved.txt").is_file() && !dir.join("g.txt").exists());
    select(&mut ed, "moved.txt");
    answer(&mut ed, "r", "new.txt");
    assert!(ed.message_text().contains("E13"), "renaming over a file is refused");
    select(&mut ed, "moved.txt");
    answer(&mut ed, "d", "n");
    assert!(dir.join("moved.txt").exists(), "only y deletes");
    answer(&mut ed, "d", "y");
    assert!(!dir.join("moved.txt").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn named_registers_append_and_black_hole() {
    let mut ed: Editor = editor("one\ntwo\nthree");
    keys(&mut ed, "\"ayyj\"Ayy");
    keys(&mut ed, "yy\"_ddG\"ap");
    assert_eq!(text(&ed), "one\nthree\none\ntwo");
    keys(&mut ed, "p");
    assert_eq!(text(&ed), "one\nthree\none\ntwo\ntwo", "\"_ left the unnamed register alone");
}

#[test]
fn command_and_search_history() {
    let mut ed: Editor = editor("foo bar foo");
    keys(&mut ed, ":bn\n:bp\n:bnext\n/bar\n");
    keys(&mut ed, ":");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "bnext");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "bp");
    ed.handle(Key::Down);
    assert_eq!(ed.cmdline, "bnext");
    ed.handle(Key::Down);
    assert_eq!(ed.cmdline, "", "past the newest: back to what was typed");
    ed.handle(Key::Esc);

    keys(&mut ed, ":bn");
    ed.handle(Key::ShiftUp);
    assert_eq!(ed.cmdline, "bnext", "the typed prefix filters");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "bn");
    keys(&mut ed, "e");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "bnext", "typing restarts the search from the new text");
    ed.handle(Key::Esc);

    keys(&mut ed, "0/foo\n0/");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "foo", "searches have their own history");
    ed.handle(Key::Up);
    assert_eq!(ed.cmdline, "bar");
    ed.handle(Key::Enter);
    assert_eq!((ed.search.as_str(), ed.buf().cursor.col), ("bar", 4));
}

#[test]
fn shift_arrows_move_by_word_and_page() {
    let lines: Vec<String> =
        std::iter::once("one two three".to_owned()).chain((1..50).map(|i| i.to_string())).collect();
    let mut ed: Editor = editor(&lines.join("\n"));
    ed.view_height = 10;
    ed.handle(Key::ShiftRight);
    assert_eq!(ed.buf().cursor.col, 4);
    ed.handle(Key::ShiftLeft);
    assert_eq!(ed.buf().cursor.col, 0);
    ed.handle(Key::ShiftDown);
    assert_eq!(ed.buf().cursor.line, 10);
    ed.handle(Key::ShiftUp);
    assert_eq!(ed.buf().cursor.line, 0);

    keys(&mut ed, "d");
    ed.handle(Key::ShiftRight);
    assert_eq!(ed.buf().line(0), "two three", "an operator takes it as w");
    keys(&mut ed, "v");
    ed.handle(Key::ShiftRight);
    keys(&mut ed, "d");
    assert_eq!(ed.buf().line(0), "hree", "visual selection grows a word at a time");

    keys(&mut ed, "uui");
    ed.handle(Key::ShiftRight);
    keys(&mut ed, "X");
    assert_eq!(ed.buf().line(0), "one Xtwo three");
    ed.handle(Key::ShiftDown);
    assert_eq!(ed.buf().cursor.line, 10);
    ed.handle(Key::ShiftUp);
    assert_eq!(ed.buf().cursor.line, 0);
    ed.handle(Key::End);
    ed.handle(Key::ShiftLeft);
    assert_eq!(ed.buf().cursor, Pos::new(0, 9), "back to the start of the last word");
}

#[test]
fn setup_screen_changes_options_live() {
    let mut ed: Editor = editor("x");
    keys(&mut ed, " ,");
    assert_eq!(ed.mode, Mode::Setup);
    keys(&mut ed, "l");
    assert_eq!(ed.colorscheme, palettes::THEMES[1].name);
    assert_eq!(ed.options.colorscheme, None, "remembered like the theme picker, not pinned in the config");
    keys(&mut ed, "hh");
    assert_eq!(ed.colorscheme, palettes::THEMES[palettes::THEMES.len() - 1].name);
    let tabstop: usize = SETTINGS.iter().position(|s| s.name == "tabstop").unwrap();
    keys(&mut ed, &"j".repeat(tabstop));
    keys(&mut ed, "ll");
    assert_eq!(ed.options.tabstop, 6);
    assert_eq!(ed.setup_value("tabstop"), "6");
    keys(&mut ed, "Gjl");
    assert!(!ed.options.gitsigns);
    keys(&mut ed, "w\x1b");
    assert_eq!(ed.mode, Mode::Normal);
    assert_eq!(ed.options.tabstop, 6);

    // Search as in VS Code's settings: `/` filters names, descriptions and sections.
    keys(&mut ed, ":settings\n/TABLINE");
    assert_eq!(ed.setup.as_ref().map(|s| s.shown().len()), Some(1));
    keys(&mut ed, "\nl");
    assert!(!ed.options.showtabline);
    keys(&mut ed, "/zzz\x1b");
    assert_eq!(ed.setup.as_ref().map(|s| s.shown().len()), Some(SETTINGS.len()), "Esc clears the search");
    keys(&mut ed, "\x1b");
    assert_eq!(ed.mode, Mode::Normal);
}

/// Vim regexes in `/` and `:s`, the `:s` flags, `\1` groups, and the shifting delete registers.
#[test]
fn regex_substitute_flags_and_registers() {
    let mut ed: Editor = editor("foo1 bar22\nFOO3");
    keys(&mut ed, "/\\d\\+\n");
    assert_eq!(ed.buf().cursor, Pos::new(0, 3));
    keys(&mut ed, ":%s/foo/x/gn\n");
    assert_eq!(ed.message_text(), "2 matches on 2 lines");
    assert_eq!(text(&ed), "foo1 bar22\nFOO3", "n only counts");
    keys(&mut ed, ":%s/\\v(\\a+)(\\d+)/\\2-\\1/g\n");
    assert_eq!(text(&ed), "1-foo 22-bar\n3-FOO");
    keys(&mut ed, ":%s/foo/X/I\n");
    assert_eq!(text(&ed), "1-X 22-bar\n3-FOO", "I matches case");
    keys(&mut ed, ":s/zzz/y/e\n");
    assert!(!ed.message_text().contains("E486"), "e: no error when nothing matches");
    keys(&mut ed, ":s/a/b/x\n");
    assert!(ed.message_text().starts_with("E488"), "unknown flag");
    keys(&mut ed, "/\\(\n");
    assert!(ed.message_text().starts_with("E54"), "{}", ed.message_text());

    let mut ed: Editor = editor("a\nb\nc");
    keys(&mut ed, "dddd\"2p");
    assert_eq!(text(&ed), "c\na", "\"2 holds the delete before last");
}

/// `:g` / `:v` visit marked lines through deletions as one undo step; `:norm` types keys on each line.
#[test]
fn global_and_normal_commands() {
    let mut ed: Editor = editor("a1\nb\na2\nb\na3");
    keys(&mut ed, ":g/^a/d\n");
    assert_eq!(text(&ed), "b\nb");
    keys(&mut ed, "u");
    assert_eq!(text(&ed), "a1\nb\na2\nb\na3", "one undo step for the whole :g");
    keys(&mut ed, ":v/a/d\n");
    assert_eq!(text(&ed), "a1\na2\na3");
    keys(&mut ed, "u:g/a/norm A!\n");
    assert_eq!(text(&ed), "a1!\nb\na2!\nb\na3!");
    keys(&mut ed, ":g/b/s/b/B/\n");
    assert_eq!(text(&ed), "a1!\nB\na2!\nB\na3!");
    keys(&mut ed, ":2,3norm! Ix\n");
    assert_eq!(text(&ed), "a1!\nxB\nxa2!\nB\na3!");
    keys(&mut ed, ":g/a/.,+1d\n");
    assert_eq!(text(&ed), "", "a line deleted by an earlier round is skipped");
    keys(&mut ed, "u:g/zz/d\n");
    assert!(ed.message_text().starts_with("E486"));
    keys(&mut ed, ":g/a/g/x/d\n");
    assert!(ed.message_text().starts_with("E147"));
    keys(&mut ed, ":norm ci\n");
    assert_eq!(ed.mode, Mode::Normal, "an unfinished command is cancelled");
}

/// Paragraph, tag and WORD text objects with operators and in visual mode.
#[test]
fn paragraph_tag_and_word_objects() {
    let mut ed: Editor = editor("a\nb\n\nc");
    keys(&mut ed, "dap");
    assert_eq!(text(&ed), "c");
    let mut ed: Editor = editor("<p>hi <b>you</b></p>");
    keys(&mut ed, "fycitme\x1b");
    assert_eq!(text(&ed), "<p>hi <b>me</b></p>");
    keys(&mut ed, "0dat");
    assert_eq!(text(&ed), "");
    let mut ed: Editor = editor("x foo.bar(1) y");
    keys(&mut ed, "fbdiW");
    assert_eq!(text(&ed), "x  y");
    let mut ed: Editor = editor("a\nb\n\nc");
    keys(&mut ed, "vipd");
    assert_eq!(text(&ed), "\nc");
}

/// `gn` with `cgn` and `.`, `:sort`, `:m`, `:t`, `:r`, `:e!`, `:saveas`, `:b`, `:ls`, `<C-^>`, `gf` and `$` in a block.
#[test]
fn gn_line_commands_files_and_block_end() {
    let mut ed: Editor = editor("foo bar foo\nfoo");
    keys(&mut ed, "/foo\nggcgnX\x1b..");
    assert_eq!(text(&ed), "X bar X\nX");

    let mut ed: Editor = editor("b2\na10\nB1\nb2");
    keys(&mut ed, ":sort\n");
    assert_eq!(text(&ed), "B1\na10\nb2\nb2");
    keys(&mut ed, ":sort! n u\n");
    assert_eq!(text(&ed), "a10\nb2\nB1");
    keys(&mut ed, ":sort i\n");
    assert_eq!(text(&ed), "a10\nB1\nb2", "case folded: b1 before b2");
    keys(&mut ed, ":1m$\n");
    assert_eq!(text(&ed), "B1\nb2\na10");
    keys(&mut ed, ":3m0\n");
    assert_eq!(text(&ed), "a10\nB1\nb2");
    keys(&mut ed, ":1,2t$\n");
    assert_eq!(text(&ed), "a10\nB1\nb2\na10\nB1");
    keys(&mut ed, ":1,2m1\n");
    assert!(ed.message_text().starts_with("E134"));

    // The built-in text commands, on the buffer or a range.
    let mut ed: Editor = editor("a\na\nb\n2*(3+4)");
    keys(&mut ed, ":1,3uniq\n");
    assert_eq!(text(&ed), "a\nb\n2*(3+4)");
    keys(&mut ed, ":$calc\n");
    assert_eq!(ed.message_text(), "14");
    keys(&mut ed, ":1calc\n");
    assert!(ed.message_text().starts_with("E15"));
    keys(&mut ed, ":wc\n");
    assert_eq!(ed.message_text(), "3 lines, 3 words, 12 chars");
    keys(&mut ed, ":base64\n");
    assert_eq!(text(&ed), "YQpiCjIqKDMrNCkK");
    keys(&mut ed, ":base64!\n");
    assert_eq!(text(&ed), "a\nb\n2*(3+4)");
    keys(&mut ed, "u");
    assert_eq!(text(&ed), "YQpiCjIqKDMrNCkK");
    keys(&mut ed, ":base64!\n:1sha256\n");
    assert_eq!(ed.message_text(), "87428fc522803d31065e7bce3cf03fe475096631e5e07bbd7a0fde60c4cf25c7");

    // A range leaves the other lines alone; a fraction keeps its decimals; bad input changes nothing.
    let mut ed: Editor = editor("@\nhi\n@\n1/4");
    keys(&mut ed, ":2base64\n");
    assert_eq!(text(&ed), "@\naGkK\n@\n1/4");
    keys(&mut ed, ":$calc\n");
    assert_eq!(ed.message_text(), "0.25");
    keys(&mut ed, ":1base64!\n");
    assert!(ed.message_text().starts_with("E475"), "@ is not base64");
    let mut ed: Editor = editor("/w==");
    keys(&mut ed, ":base64!\n");
    assert!(ed.message_text().starts_with("E475"), "byte 0xff is not UTF-8");
    assert_eq!(text(&ed), "/w==");
    keys(&mut ed, "cc=\x1b:base64!\n");
    assert_eq!(text(&ed), "", "nothing decodes to one empty line");
    keys(&mut ed, ":wc\n");
    assert_eq!(ed.message_text(), "1 lines, 0 words, 1 chars");

    let dir: PathBuf = std::env::temp_dir().join(format!("lim-ex-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (one, two): (PathBuf, PathBuf) = (dir.join("one.txt"), dir.join("two.txt"));
    std::fs::write(&one, "x\ny\n").unwrap();
    let mut ed: Editor = editor("top\nbottom");
    keys(&mut ed, &format!(":r {}\n", one.display()));
    assert_eq!(text(&ed), "top\nx\ny\nbottom");
    keys(&mut ed, &format!(":saveas {}\n", two.display()));
    assert_eq!(std::fs::read_to_string(&two).unwrap(), "top\nx\ny\nbottom\n");
    assert_eq!(ed.buf().path.as_deref(), Some(two.as_path()));
    keys(&mut ed, "ggdd:e!\n");
    assert_eq!(text(&ed), "top\nx\ny\nbottom", ":e! drops the change");
    assert!(!ed.buf().is_dirty());
    keys(&mut ed, &format!(":e {}\n", one.display()));
    keys(&mut ed, ":b two\n");
    assert_eq!(ed.buf().path.as_deref(), Some(two.as_path()));
    ed.handle(Key::Ctrl('^'));
    assert_eq!(ed.buf().path.as_deref(), Some(one.as_path()));
    keys(&mut ed, ":b 9\n");
    assert!(ed.message_text().starts_with("E86"));
    // `:rename` moves the file on disk and keeps the buffer on it; it never overwrites another file.
    let three: PathBuf = dir.join("sub").join("three.txt");
    keys(&mut ed, &format!(":rename {}\n", three.display()));
    assert_eq!(ed.buf().path.as_deref(), Some(three.as_path()));
    assert!(!one.exists());
    assert_eq!(std::fs::read_to_string(&three).unwrap(), "x\ny\n");
    keys(&mut ed, &format!(":rename {}\n", two.display()));
    assert!(ed.message_text().starts_with("E13"));
    assert!(three.exists());
    keys(&mut ed, ":ls\n");
    assert_eq!(ed.mode, Mode::Picker);
    ed.handle(Key::Esc);

    let mut ed: Editor = editor(&format!("see {}:2 for details", two.display()));
    keys(&mut ed, "wwgf");
    // `gf` resolves the path: macOS' temp dir is behind the /var symlink, Windows' may be an 8.3 short name.
    assert_eq!(ed.buf().path.as_deref(), Some(crate::buffer::canonical(&two).as_path()));
    assert_eq!(ed.buf().cursor.line, 1, "path:line jumps to the line");
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dir);

    let mut ed: Editor = editor("ab\nabcd\na");
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jj$A;\x1b");
    assert_eq!(text(&ed), "ab;\nabcd;\na;");
    keys(&mut ed, "gg0l");
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jj$d");
    assert_eq!(text(&ed), "a\na\na", "$ takes each line to its own end");
}

/// `"on": "save"` commands run before writing, only for their extensions; a failure is shown and the file is
/// still written (tests cannot spawn, so the run itself fails here).
#[test]
fn format_on_save() {
    let (config, warnings): (Config, Vec<String>) = Config::from_str(
        r#"{ "commands": { "Fmt": { "run": ["fmt"], "output": "replace", "on": "save", "files": ["*.rs"] } } }"#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let dir: PathBuf = std::env::temp_dir().join(format!("lim-onsave-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ed: Editor = editor("x");
    ed.configure(&config);
    keys(&mut ed, &format!(":w {}\n", dir.join("a.rs").display()));
    assert_eq!(ed.message_text(), "Fmt: cannot run programs here");
    assert_eq!(std::fs::read_to_string(dir.join("a.rs")).unwrap(), "x\n", "written anyway");
    keys(&mut ed, &format!(":saveas {}\n", dir.join("b.txt").display()));
    assert!(ed.message_text().contains("written"), "not run for other extensions: {}", ed.message_text());
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dir);
    let (_, warnings): (Config, Vec<String>) =
        Config::from_str(r#"{ "commands": { "Bad": { "run": ["x"], "on": "open" } } }"#);
    assert_eq!(warnings.len(), 1);
}

/// Found by the fuzzer: `gv` after its lines were deleted, then an operator; an address with junk after `-`.
#[test]
fn fuzz_regressions() {
    let mut ed: Editor = editor("a\nb\nc\nd\ne");
    keys(&mut ed, "Gvk\x1bggdGgvs");
    assert_eq!(ed.mode, Mode::Insert);
    let mut ed: Editor = editor("é\nx");
    keys(&mut ed, ":co-rsp>1Namenéls\n");
    assert_eq!(text(&ed), "é\nx");
    keys(&mut ed, ":1t.-é\n");
    assert_eq!(text(&ed), "é\nx");
    // Seed 162: `ggVGJ` on a big document joined line by line, copying the growing line each time.
    let mut ed: Editor = editor(&vec!["word"; 50_000].join("\n"));
    let start: Instant = Instant::now();
    keys(&mut ed, "ggVGJ");
    assert!(start.elapsed().as_secs() < 1, "{:?}", start.elapsed());
    assert_eq!((ed.buf().line_count(), ed.buf().line_len(0)), (1, 50_000 * 5 - 1));
}

#[test]
fn join_trims_spaces_or_glues() {
    let mut ed: Editor = editor("foo  \n  bar\n)x\n\nbaz");
    keys(&mut ed, "5J");
    assert_eq!((text(&ed).as_str(), ed.buf().cursor), ("foo bar)x baz", Pos::new(0, 9)), "none before ) or empty");
    keys(&mut ed, "u");
    assert_eq!(text(&ed), "foo  \n  bar\n)x\n\nbaz", "one undo step");
    keys(&mut ed, "gJ");
    assert_eq!((text(&ed).as_str(), ed.buf().cursor.col), ("foo    bar\n)x\n\nbaz", 5));
    keys(&mut ed, "GJ");
    assert_eq!(text(&ed), "foo    bar\n)x\n\nbaz", "nothing below the last line");
}

/// `:norm` replays keys without eating the replay budget of `.` and macros it runs.
#[test]
fn norm_runs_dot_and_macros() {
    let mut ed: Editor = editor("a\nb\nc");
    keys(&mut ed, "A!\x1b:2,3norm .\n");
    assert_eq!(text(&ed), "a!\nb!\nc!");
    keys(&mut ed, "qqI-\x1bqu:g/!/norm @q\n");
    assert_eq!(text(&ed), "-a!\n-b!\n-c!");
}

/// `:g/x/d` deletes in one pass but leaves what Vim's line-by-line deletes leave: marks on their lines,
/// the last deletes in `"1`…, the cursor where the last one was, one undo step.
#[test]
fn global_delete_in_one_pass() {
    let mut ed: Editor = editor("x1\nkeep a\nx2\nkeep b\nx3\nkeep c");
    keys(&mut ed, "jjjmbgg:g/^x/d\n");
    assert_eq!(text(&ed), "keep a\nkeep b\nkeep c");
    assert_eq!(ed.buf().cursor.line, 2, "on the line after the last delete");
    keys(&mut ed, "gg'b");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "keep b", "the mark followed its line");
    keys(&mut ed, "\"1p");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "x3");
    keys(&mut ed, "u\"2p");
    assert_eq!(ed.buf().line(ed.buf().cursor.line), "x2");
    keys(&mut ed, "uu");
    assert_eq!(text(&ed), "x1\nkeep a\nx2\nkeep b\nx3\nkeep c", "one undo step");
    // Found by the fuzzer: every line deleted.
    keys(&mut ed, "ma:g/./d\n");
    assert_eq!((text(&ed).as_str(), ed.buf().cursor.line), ("", 0));
}

/// `:s///c`: `y` replaces, `n` skips, `l` replaces and stops, `a` does the rest, one undo step for it all.
#[test]
fn substitute_with_confirmation() {
    let mut ed: Editor = editor("a a\na\na a");
    keys(&mut ed, ":%s/a/X/gc\n");
    assert_eq!(ed.mode, Mode::Confirm);
    assert!(ed.message_text().starts_with("replace with X"));
    keys(&mut ed, "yny");
    assert_eq!(text(&ed), "X a\nX\na a");
    keys(&mut ed, "a");
    assert_eq!((ed.mode, text(&ed).as_str()), (Mode::Normal, "X a\nX\nX X"));
    assert_eq!(ed.message_text(), "4 substitutions");
    keys(&mut ed, "u");
    assert_eq!(text(&ed), "a a\na\na a", "one undo step");
    keys(&mut ed, ":%s/a/\\r/c\nl");
    assert_eq!((ed.mode, text(&ed).as_str()), (Mode::Normal, "\n a\na\na a"), "l replaces one and stops; \\r splits");
    keys(&mut ed, ":s/zz/y/c\n");
    assert!(ed.message_text().starts_with("E486"));
    keys(&mut ed, "u:%s/a/aa/gc\na");
    assert_eq!(text(&ed), "aa aa\naa\naa aa", "a replacement containing the pattern is not matched again");
}

#[test]
fn mouse_clicks_drags_and_scrolls() {
    use crate::render::Screen;
    use crate::term::{ColorDepth, Mouse, MouseKind, Size};
    let mut ed: Editor = editor(&(0..100).map(|i| format!("line {i}")).collect::<Vec<String>>().join("\n"));
    let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
    assert!(screen.draw(&mut ed, Size { cols: 80, rows: 20 }).contains("\x1b[?1002h\x1b[?1006h"));
    let pane: Pane = ed.panes[0];
    let at = |kind: MouseKind, col: usize, row: usize| {
        Key::Mouse(Mouse { kind, col: pane.area.x + pane.gutter + col, row: pane.area.y + row })
    };
    ed.handle(at(MouseKind::Press, 2, 1));
    assert_eq!(ed.buf().cursor, Pos::new(1, 2));
    ed.handle(at(MouseKind::Drag, 40, 2));
    assert_eq!((ed.mode, ed.anchor, ed.buf().cursor), (Mode::Visual, Pos::new(1, 2), Pos::new(2, 5)), "past the end");
    ed.handle(at(MouseKind::Release, 40, 2));
    keys(&mut ed, "d");
    assert_eq!(ed.buf().line(1), "li");
    ed.handle(at(MouseKind::ScrollDown, 0, 0));
    assert_eq!(ed.buf().top, 3);
    ed.handle(at(MouseKind::Drag, 0, 0));
    assert_eq!(ed.mode, Mode::Normal, "a drag without a press selects nothing");
    ed.handle(at(MouseKind::Press, 0, 0));
    keys(&mut ed, "j");
    ed.handle(at(MouseKind::Drag, 3, 0));
    assert_eq!(ed.mode, Mode::Normal, "a key between press and drag (maybe :bnext) drops the anchor");
    ed.options.mouse = false;
    assert!(screen.draw(&mut ed, Size { cols: 80, rows: 20 }).contains("\x1b[?1002l"));
    let before: Pos = ed.buf().cursor;
    ed.handle(at(MouseKind::Press, 0, 5));
    assert_eq!(ed.buf().cursor, before, "off: clicks are ignored");
}

#[test]
fn mouse_on_the_tabline_switches_and_closes_buffers() {
    use crate::render::Screen;
    use crate::term::{ColorDepth, Mouse, MouseKind, Size};
    let mut ed: Editor = editor("one");
    keys(&mut ed, ":enew\n:enew\n");
    let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
    let click = |ed: &mut Editor, screen: &mut Screen, wanted: TabClick| {
        screen.draw(ed, Size { cols: 120, rows: 20 });
        let hit: TabHit = *ed.tab_hits.iter().find(|h| h.click == wanted).expect("drawn");
        ed.handle(Key::Mouse(Mouse { kind: MouseKind::Press, col: hit.from, row: 0 }));
    };
    click(&mut ed, &mut screen, TabClick::Buffer(0));
    assert_eq!((ed.current, text(&ed).as_str()), (0, "one"));
    click(&mut ed, &mut screen, TabClick::Close(2));
    assert_eq!(ed.buffers.len(), 2);
    keys(&mut ed, "x");
    click(&mut ed, &mut screen, TabClick::CloseAll);
    assert_eq!((ed.buffers.len(), text(&ed).as_str()), (1, "ne"), "the modified buffer stays");
    screen.draw(&mut ed, Size { cols: 120, rows: 20 });
    let last: usize = ed.tab_hits.iter().map(|h| h.to).max().unwrap_or(0);
    assert_eq!(last, 120, "the buttons end the row");
}

#[test]
fn mouse_in_another_window_ends_visual_mode() {
    use crate::render::Screen;
    use crate::term::{ColorDepth, Mouse, MouseKind, Size};
    let mut ed: Editor = editor("one\ntwo\nthree");
    keys(&mut ed, ":vsplit\n:enew\n");
    ed.handle(Key::Ctrl('w'));
    keys(&mut ed, "wGv");
    Screen::new(ColorDepth::TrueColor).draw(&mut ed, Size { cols: 80, rows: 20 });
    let other: Pane = *ed.panes.iter().find(|p| p.window != ed.active).expect("two windows");
    ed.handle(Key::Mouse(Mouse { kind: MouseKind::ScrollDown, col: other.area.x, row: other.area.y }));
    assert_eq!((ed.active, ed.mode), (other.window, Mode::Normal));
    keys(&mut ed, "~");
}

/// A binary file opens as a hex dump, its edits and `:pack`/`:xor` are written back as bytes, and a dump that is
/// not hex is refused instead of written.
#[test]
fn hex_buffers_and_reverse_engineering() {
    let path: PathBuf = std::env::temp_dir().join(format!("lim-hex-{}.bin", std::process::id()));
    let mut bytes: Vec<u8> = vec![0; 32];
    bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
    (bytes[16], bytes[18]) = (2, 0x3e);
    bytes[20..26].copy_from_slice(b"hello\xff");
    std::fs::write(&path, &bytes).unwrap();
    let mut ed: Editor = editor("");
    keys(&mut ed, &format!(":e {}\n", path.display()));
    assert!(ed.buf().hex);
    assert!(text(&ed).starts_with("00000000: 7f 45 4c 46 02 01 00 00"), "{}", text(&ed));
    keys(&mut ed, ":detect\n");
    assert_eq!(ed.message_text(), "ELF 64-bit LSB executable x86-64 · 32 bytes · entropy 2.53");
    keys(&mut ed, "j022l:inspect\n");
    assert!(ed.message_text().starts_with("0x14: u8 104 i8 104 · u16 25960/26725"), "{}", ed.message_text());
    keys(&mut ed, ":pack u16be 0x4142\n:pack u24le 1\n");
    assert!(ed.message_text().starts_with("E475"));
    keys(&mut ed, "gg010lr0lr0:w\n");
    let written: Vec<u8> = std::fs::read(&path).unwrap();
    assert_eq!((written[0], &written[20..22]), (0, &b"AB"[..]));
    keys(&mut ed, ":2xor 0x01\n:w\n");
    assert_eq!(std::fs::read(&path).unwrap()[16..21], [3, 1, 0x3f, 1, b'@']);
    keys(&mut ed, "u:w\n");
    assert_eq!(std::fs::read(&path).unwrap(), written, "undo restores the bytes");
    keys(&mut ed, "gg010liz\x1b:w\n");
    assert!(ed.message_text().starts_with("E513"), "{}", ed.message_text());
    assert_eq!(std::fs::read(&path).unwrap(), written, "a broken dump is never written");
    keys(&mut ed, "u:strings\n");
    assert_eq!(text(&ed), "00000014: ABllo");
    let _: std::io::Result<()> = std::fs::remove_file(&path);

    let mut ed: Editor = editor("hi");
    keys(&mut ed, ":inspect\n");
    assert!(ed.message_text().starts_with("E21"));
    keys(&mut ed, ":hexedit\n");
    assert_eq!(text(&ed), format!("00000000: 68 69 0a{}|hi.|", " ".repeat(42)));
    keys(&mut ed, ":hexedit\n");
    assert_eq!(text(&ed), "hi");
    keys(&mut ed, ":hex\n");
    assert_eq!(text(&ed), "68690a");
    keys(&mut ed, ":hex!\n:base32\n");
    assert_eq!(text(&ed), "NBUQU===");
    keys(&mut ed, ":base32!\ncca b\x1b:url\n");
    assert_eq!(text(&ed), "a%20b");
    keys(&mut ed, ":url!\n:rot13\n");
    assert_eq!(text(&ed), "n o");
}

/// A `:Name` command runs in the background: its output lands when it arrives, unless the text changed meanwhile.
#[test]
fn background_commands_apply_when_done() {
    use crate::config::CommandOutput;
    use std::sync::mpsc::{self, Receiver, Sender};
    type Outcome = Result<String, String>;
    let mut ed: Editor = editor("b\na");
    let all: vim::LineRange = vim::LineRange { first: 0, last: 1 };
    let (send, result): (Sender<Outcome>, Receiver<Outcome>) = mpsc::channel();
    ed.start_job("Sort", CommandOutput::Replace, all, result);
    keys(&mut ed, "l");
    assert_eq!(text(&ed), "b\na", "still running");
    assert!(ed.deadline().is_some(), "the loop wakes up to check it");
    let mut screen: crate::render::Screen = crate::render::Screen::new(crate::term::ColorDepth::TrueColor);
    let mut frame = |ed: &mut Editor| screen.draw(ed, crate::term::Size { cols: 80, rows: 10 });
    let running: String = frame(&mut ed);
    assert!(crate::render::SPINNER.iter().any(|f| running.contains(&format!("{f} Sort"))), "the hydra spins");
    send.send(Ok("a\nb\n".into())).unwrap();
    ed.tick();
    assert_eq!(text(&ed), "a\nb");
    assert!(ed.running_jobs().is_none() && !frame(&mut ed).contains(" Sort  "), "and stops when done");
    let (send, result): (Sender<Outcome>, Receiver<Outcome>) = mpsc::channel();
    ed.start_job("Sort", CommandOutput::Replace, all, result);
    keys(&mut ed, "x");
    send.send(Ok("z\n".into())).unwrap();
    ed.tick();
    assert_eq!(text(&ed), "\nb");
    assert!(ed.message_text().contains("output dropped"), "{}", ed.message_text());
    let (send, result): (Sender<Outcome>, Receiver<Outcome>) = mpsc::channel();
    ed.start_job("Count", CommandOutput::Message, all, result);
    drop(send);
    ed.tick();
    assert!(ed.message_text().contains("stopped"));
}

/// `<Leader>x`: tools are windows whose buffer keeps every question and answer; `<CR>` answers the line under the
/// cursor (again, after an edit), answers are yanked, and `:q` closes the window without asking to save.
#[test]
fn leader_tools() {
    let mut ed: Editor = editor("fooBar x");
    keys(&mut ed, " xc2+3\nans*2\n1/0\n");
    assert_eq!((ed.mode, ed.windows.len(), ed.buf().title.as_deref()), (Mode::Insert, 2, Some("[Calculator]")));
    assert_eq!(text(&ed), "2+3\n= 5\nans*2\n= 10\n1/0\n! infinite result (a division by zero or an overflow)\n");
    keys(&mut ed, "\x1bggA*2\x1b\n");
    assert_eq!(ed.buf().line(1), "= 8", "an edited question is answered again, in place");
    keys(&mut ed, "gg+\n");
    assert_eq!(ed.buf().line(1), "= 8", "<CR> on an answer answers the question above");
    let history: usize = ed.buf().line_count();
    keys(&mut ed, "G\n");
    assert_eq!(ed.buf().line_count(), history, "an empty line asks nothing");
    keys(&mut ed, "ggi= 1\x1b\n");
    assert_eq!(ed.buf().line(0), "= 12+3*2", "an answer on the first line has no question to answer");
    keys(&mut ed, "u");
    keys(&mut ed, ":q\n");
    assert_eq!((ed.windows.len(), ed.buf().title.as_deref()), (1, None), "closed without E37");
    keys(&mut ed, "p");
    assert_eq!(text(&ed), "f8ooBar x", "the last answer was yanked");
    keys(&mut ed, "u xc\x1b");
    assert_eq!(ed.buf().line_count(), history, "the window comes back with its history");
    keys(&mut ed, ":q\n xu5 ft to cm\n\x1b:q\n xt\n\x1b");
    assert!(ed.buf().line(1).starts_with("= "), "an empty line in Time is now: {}", ed.buf().line(1));
    keys(&mut ed, ":q\n");
    let convert: usize = ed.buffers.iter().position(|b| b.title.as_deref() == Some("[Convert]")).unwrap();
    assert_eq!(ed.buffers[convert].line(1), "= 152.4 cm");
    keys(&mut ed, " xksn\n");
    assert_eq!(text(&ed), "foo_bar x");
    assert_eq!(ed.mode, Mode::Normal);
    keys(&mut ed, "0 xi");
    assert_eq!(text(&ed).len(), "foo_bar x".len() + 36);
    keys(&mut ed, "u:%case constant\n");
    assert_eq!(text(&ed), "FOO_BAR_X");
    let mut ed: Editor = editor("{\"a\":[1,2],\"b\":{}}");
    keys(&mut ed, " xj");
    assert_eq!(text(&ed), "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {}\n}");
    keys(&mut ed, " xJ");
    assert_eq!(text(&ed), "{\"a\":[1,2],\"b\":{}}");
    keys(&mut ed, "A,\x1b xj");
    assert!(ed.message_text().contains("E474"), "invalid JSON is left alone");
}

/// Block edits on ragged lines: `A` pads short lines, `r` skips them, a put runs past the last line.
#[test]
fn visual_block_ragged_lines() {
    let mut ed: Editor = editor("ab\nx\nabc");
    ed.handle(Key::Char('l'));
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jjA!\x1b");
    assert_eq!(text(&ed), "ab!\nx !\nab!c");
    let mut ed: Editor = editor("abc\nx\nabc");
    ed.handle(Key::Char('l'));
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jjlr*");
    assert_eq!(text(&ed), "a**\nx\na**");
    let mut ed: Editor = editor("ab\ncd");
    ed.handle(Key::Ctrl('v'));
    keys(&mut ed, "jyG$p");
    assert_eq!(text(&ed), "ab\ncda\n  c");
}

/// Config commands end to end, through a stand-in for the process runner (no program starts in tests).
#[test]
fn user_commands_with_a_stand_in_runner() {
    let (config, warnings): (Config, Vec<String>) = Config::from_str(
        r#"{ "commands": {
            "Up": { "run": ["up", "${line}"], "output": "replace" },
            "Say": { "run": ["say"] },
            "Quiet": { "run": ["quiet"] },
            "Empty": { "run": ["empty"], "output": "replace" },
            "Fail": { "run": ["fail"] } } }"#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut ed: Editor = editor("a\nb");
    ed.configure(&config);
    ed.runner = Some(|argv: &[String], input: &str, _| match argv[0].as_str() {
        "up" => Ok(format!("{}{}", input.to_uppercase(), argv[1])),
        "say" => Ok(format!("{} lines\nok\n", input.lines().count())),
        "quiet" | "empty" => Ok(String::new()),
        _ => Err("it failed".into()),
    });
    ed.run_user_command("", "Up", &[], false).unwrap();
    assert_eq!(text(&ed), "A\nB\n1", "replaced; the cursor line number expanded");
    ed.run_user_command("", "Say", &[], false).unwrap();
    assert_eq!(ed.message_text(), "3 lines | ok");
    ed.run_user_command("", "Quiet", &[], false).unwrap();
    assert_eq!(ed.message_text(), "Quiet: done");
    assert!(ed.run_user_command("", "Empty", &[], false).unwrap_err().to_string().contains("no output"));
    assert!(ed.run_user_command("", "Fail", &[], false).unwrap_err().to_string().contains("it failed"));
    assert!(matches!(ed.run_user_command("zz", "Up", &[], false), Err(Error::NotACommand(_))));
    assert!(matches!(ed.run_user_command("", "Nope", &[], false), Err(Error::NotACommand(_))));
    // Typed by hand they run in the background, and land on a later tick.
    keys(&mut ed, ":Say\n");
    assert_eq!(ed.message_text(), "Say: running");
    for _ in 0..200 {
        ed.tick();
        if ed.jobs.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(ed.message_text(), "3 lines | ok");
}

/// `<Leader>c` on a tool closes its window with it, as `:q` does; other buffers keep their windows.
#[test]
fn closing_a_tool_buffer_closes_its_window() {
    let mut ed: Editor = editor("code");
    keys(
        &mut ed, " xc2+2
",
    );
    assert_eq!((ed.buffers.len(), ed.windows.len()), (2, 2));
    keys(&mut ed, " c");
    assert_eq!((ed.buffers.len(), ed.windows.len(), text(&ed).as_str()), (1, 1, "code"));
    keys(
        &mut ed,
        ":vsplit
:enew
 c",
    );
    assert_eq!(ed.windows.len(), 2, "a plain buffer leaves its window to the next one");
    keys(
        &mut ed,
        ":only
 xu:only
 c",
    );
    assert_eq!((ed.buffers.len(), ed.windows.len(), text(&ed).as_str()), (1, 1, "code"), "the last window stays");
}

/// A tool opens in a small popup over the windows, outside the layout; Esc hides it, its key brings it back, and
/// the tools share one popup.
#[test]
fn tools_open_in_one_small_popup() {
    use crate::render::Screen;
    use crate::term::{ColorDepth, Size};
    let mut ed: Editor = editor("code");
    let layout: Layout = ed.layout.clone();
    keys(&mut ed, " xc2+2\n");
    assert_eq!((ed.windows.len(), ed.windows[ed.active].floating), (2, true));
    assert_eq!(ed.layout, layout, "the split windows stay as they were");
    let frame: String = Screen::new(ColorDepth::TrueColor).draw(&mut ed, Size { cols: 120, rows: 40 });
    assert!(frame.contains("Calculator") && frame.contains("= 4"), "the popup shows the tool");
    assert!(ed.view_height < 10, "{}", ed.view_height);
    keys(&mut ed, "\x1b\x1b");
    assert_eq!((ed.windows.len(), text(&ed).as_str()), (1, "code"), "Esc in normal mode hides it");
    keys(&mut ed, " xc\x1b");
    assert_eq!((ed.windows.len(), ed.buf().line(1)), (2, "= 4"), "back with its history");
    keys(&mut ed, " xu");
    assert_eq!((ed.windows.len(), ed.buf().title.as_deref()), (2, Some("[Convert]")), "one popup for all tools");
}
