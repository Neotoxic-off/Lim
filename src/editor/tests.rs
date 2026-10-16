//! Key-sequence tests: type like a user, check the text.

use super::*;

fn editor(text: &str) -> Editor {
    let mut ed = Editor::new(&[]);
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
}

fn text(ed: &Editor) -> String {
    ed.buf().lines().join("\n")
}

#[test]
fn operators_undo_and_paste() {
    let mut ed = editor("foo bar baz\nqux");
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
    let mut ed = editor("a b c d e");
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
    let mut ed = editor("");
    keys(&mut ed, "ifn(\x1b");
    assert_eq!(text(&ed), "fn()");
    keys(&mut ed, "A {\nx\x1b");
    assert_eq!(text(&ed), "fn() {\n  x\n}");
    keys(&mut ed, "o\x1b");
    assert_eq!(text(&ed), "fn() {\n  x\n\n}");
    keys(&mut ed, "ggA\"\x08\x1b");
    assert_eq!(text(&ed), "fn() {\n  x\n\n}");
    keys(&mut ed, "ggi'\x1b"); // no pair before a word char
    assert_eq!(text(&ed), "'fn() {\n  x\n\n}");
    keys(&mut ed, "O'\x1b");
    assert_eq!(text(&ed), "''\n'fn() {\n  x\n\n}");
}

#[test]
fn search_and_commands() {
    let mut ed = editor("alpha beta\nBeta\nbeta");
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
    let mut ed = editor("long line here\nab\nanother long one");
    keys(&mut ed, "$jj");
    assert_eq!(ed.buf().cursor, Pos::new(2, 15));
    keys(&mut ed, "0wk");
    assert_eq!(ed.buf().cursor, Pos::new(1, 1));
    keys(&mut ed, "k");
    assert_eq!(ed.buf().cursor, Pos::new(0, 8));
}

#[test]
fn dot_repeat_and_macros() {
    let mut ed = editor("a1\na2\na3\na4");
    keys(&mut ed, "A;\x1bj.");
    assert_eq!(text(&ed), "a1;\na2;\na3\na4");
    keys(&mut ed, "jddj.");
    assert_eq!(text(&ed), "a1;\na2;");
    let mut ed = editor("x\ny\nz");
    keys(&mut ed, "qaI- \x1bjq2@a");
    assert_eq!(text(&ed), "- x\n- y\n- z");
    assert_eq!(ed.recording(), None);
}

#[test]
fn text_objects_star_and_percent() {
    let mut ed = editor("call(one, \"two\") end");
    keys(&mut ed, "fodi(");
    assert_eq!(text(&ed), "call() end");
    keys(&mut ed, "u0f\"ci\"X\x1b");
    assert_eq!(text(&ed), "call(one, \"X\") end");
    keys(&mut ed, "0%");
    assert_eq!(ed.buf().cursor.col, 13);
    keys(&mut ed, "$daw"); // no trailing blank: `aw` takes the leading one
    assert_eq!(text(&ed), "call(one, \"X\")");
    let mut ed = editor("foo food\nfoo");
    keys(&mut ed, "*");
    assert_eq!(ed.buf().cursor, Pos::new(1, 0));
    keys(&mut ed, "viwy$p");
    assert_eq!(text(&ed), "foo food\nfoofoo");
}

#[test]
fn comment_toggle() {
    let mut ed = editor("");
    ed.buffers[0] = Buffer::from_text(Some("x.rs".into()), "fn a() {\n    b();\n\n    c();\n}");
    keys(&mut ed, "jgcj");
    assert_eq!(text(&ed), "fn a() {\n    // b();\n\n    c();\n}");
    keys(&mut ed, "Vjjgc");
    assert_eq!(text(&ed), "fn a() {\n    // // b();\n\n    // c();\n}");
    keys(&mut ed, "gg2 /");
    assert_eq!(text(&ed), "// fn a() {\n//     // // b();\n\n    // c();\n}");
    keys(&mut ed, "uuggjgcc");
    assert_eq!(text(&ed), "fn a() {\n    b();\n\n    c();\n}");
}

#[test]
fn dot_repeats_replace_and_recover_repairs() {
    let mut ed = editor("ab\ncd");
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
    let backup = ed.message_text().rsplit(' ').next().map(std::path::PathBuf::from).unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), ".b\ncd\n");
    std::fs::remove_file(backup).unwrap();
}

#[test]
fn buffers_and_windows() {
    let mut ed = editor("one");
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

#[test]
fn user_keymaps_and_leader() {
    let (config, warnings) = crate::config::Config::from_str(
        r#"{ "options": { "mapleader": "," },
             "keymaps": { "i": { "jk": "<Esc>" }, "n": { "<Leader>d": "dd", "Q": false, "H": "^" } } }"#,
    );
    assert!(warnings.is_empty());
    let mut ed = editor("  abc\nxyz");
    ed.configure(&config);
    keys(&mut ed, "$H");
    assert_eq!(ed.buf().cursor.col, 2);
    keys(&mut ed, "Ajk");
    assert_eq!((ed.mode, text(&ed).as_str()), (Mode::Normal, "  abc\nxyz"));
    keys(&mut ed, "Aj");
    assert_eq!(text(&ed), "  abc\nxyz"); // `j` waits for a possible `k`
    ed.typeahead_at -= std::time::Duration::from_secs(1);
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
    let mut ed = editor("alpha\nbeta\ngamma");
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

    let dir = std::env::temp_dir().join(format!("lim-tree-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/inner.txt"), "inside").unwrap();
    std::fs::write(dir.join("top.txt"), "top").unwrap();
    ed.explorer = Some(Explorer::new(dir.clone()));
    ed.mode = Mode::Explorer;
    let names = |ed: &Editor| ed.explorer.as_ref().map(|e| e.entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
    assert_eq!(names(&ed), Some(vec!["sub".to_owned(), "top.txt".to_owned()]));
    keys(&mut ed, "ljl");
    assert_eq!(names(&ed), Some(vec!["sub".to_owned(), "inner.txt".to_owned(), "top.txt".to_owned()]));
    assert_eq!((ed.mode, ed.buf().name()), (Mode::Normal, "inner.txt".to_owned()));
    std::fs::remove_dir_all(dir).unwrap();

    let mut ed = Editor::new(&[]);
    assert!(ed.dashboard);
    keys(&mut ed, "ix");
    assert_eq!(text(&ed), "");
    keys(&mut ed, "n");
    assert!(!ed.dashboard);
}

#[test]
fn recursive_macro_is_bounded() {
    let mut ed = editor("x\ny");
    keys(&mut ed, "qaj99@aq@a");
    assert_eq!(ed.buf().cursor.line, 1);
    assert_eq!(ed.message_text(), "Replay stopped: too many keys");
}

#[test]
fn registers_marks_and_search_back() {
    let mut ed = editor("one\ntwo\nthree");
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
    let mut ed = editor("a.b c-d e,f\nx = 41;");
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
    let mut ed = editor("foo bar foo\nfoo\nbaz");
    keys(&mut ed, ":s/foo/X/\n");
    assert_eq!(text(&ed), "X bar foo\nfoo\nbaz");
    keys(&mut ed, ":%s/foo/<&>/g\n");
    assert_eq!(text(&ed), "X bar <foo>\n<foo>\nbaz");
    keys(&mut ed, ":2,3d\n");
    assert_eq!(text(&ed), "X bar <foo>");
    let mut ed = editor("a\nb\nc\nd");
    keys(&mut ed, "jVj:>\n");
    assert_eq!(text(&ed), "a\n  b\n  c\nd");
    keys(&mut ed, ":$\n");
    assert_eq!(ed.buf().cursor.line, 3);
    keys(&mut ed, "ggjVj\x1bgg");
    keys(&mut ed, "gvd");
    assert_eq!(text(&ed), "a\nd");
}

#[test]
fn visual_block() {
    let mut ed = editor("abcd\nefgh\nijkl");
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
}

#[test]
fn insert_extras_and_astro_keys() {
    let mut ed = editor("");
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
    let mut ed = editor("a");
    keys(&mut ed, ":colorscheme dracula\n");
    assert_eq!(ed.colorscheme, "dracula");
    keys(&mut ed, " ub");
    assert_eq!(ed.colorscheme, "astrolight");
    keys(&mut ed, " ft");
    keys(&mut ed, "nord");
    assert_eq!(ed.colorscheme, "nord");
    ed.handle(Key::Esc);
    assert_eq!(ed.colorscheme, "astrolight");
    keys(&mut ed, ":colorscheme nope\n");
    assert!(ed.message_text().contains("E185"));

    ed.handle(Key::F(1));
    assert_eq!(ed.mode, Mode::Cheatsheet);
    keys(&mut ed, "macro");
    let cards = ed.cheatsheet_cards();
    assert!(cards.iter().all(|c| c.rows.iter().all(|(k, d)| (k.clone() + d).to_lowercase().contains("macro") || c.title.to_lowercase().contains("macro"))));
    assert!(!cards.is_empty());
    ed.handle(Key::Esc);

    let path = std::env::temp_dir().join(format!("lim-rename-{}.txt", std::process::id()));
    let target = path.with_extension("md");
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
    let mut ed = editor("a\nb\nc\nd\ne");
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
    let mut ed = editor("restock result\n");
    keys(&mut ed, "Gores");
    let items = ed.completion.as_ref().map(|c| c.items.clone());
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
    let mut ed = editor("a");
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
fn command_line_completion() {
    let mut ed = editor("a");
    keys(&mut ed, ":colo");
    assert!(ed.cmd_completion.is_some());
    ed.handle(Key::Tab);
    assert_eq!(ed.cmdline, "colorscheme");
    keys(&mut ed, " drac");
    ed.handle(Key::Tab);
    assert_eq!(ed.cmdline, "colorscheme dracula");
    keys(&mut ed, "\n");
    assert_eq!(ed.colorscheme, "dracula");
    keys(&mut ed, ":colorscheme catppuccin-");
    ed.handle(Key::Tab);
    ed.handle(Key::Tab);
    ed.handle(Key::BackTab);
    assert_eq!(ed.cmdline, "colorscheme catppuccin-mocha");
    ed.handle(Key::Esc);
    assert!(ed.cmd_completion.is_none());
}
