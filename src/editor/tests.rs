//! Key-sequence tests: type like a user, check the text.

use super::*;

fn editor(text: &str) -> Editor {
    let mut ed = Editor::new(&[]);
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
