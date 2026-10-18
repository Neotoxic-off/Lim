//! Performance budgets: the hot paths timed on realistic sizes, so a change that slows lim down fails
//! `cargo test`. `cargo test perf -- --nocapture` prints the table to compare before and after an update
//! (`--release` for numbers close to what users get). Budgets are about 3-5x the debug times measured on the
//! dev machine: loose enough for a busy CI box, tight enough to catch a real regression (rescanning the whole
//! buffer for completion on every key made typing 7x slower, and failed here).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use crate::buffer::Buffer;
use crate::editor::Editor;
use crate::fuzz::phrase_keys;
use crate::render::Screen;
use crate::term::{ColorDepth, Key, Size};

/// 10 000 lines of Rust: comments, strings, numbers, keywords, a TODO, nested blocks.
fn big_rust() -> String {
    let block: &str = "/// Doc comment for item {n}.\nfn item_{n}(value: &str) -> Option<usize> {\n    let parsed: usize = value.trim().parse().ok()?; // TODO: errors\n    if parsed > 42 { return Some(parsed * 2); }\n    let text: &str = \"a string with {braces} and \\\"quotes\\\"\";\n    println!(\"{} {}\", text, parsed);\n    None\n}\n\n";
    (0..1111).map(|n| block.replace("{n}", &n.to_string())).collect()
}

fn big_markdown() -> String {
    let block: &str = "## Section\n\nSome **bold** text, `code`, a [link](http://x) and more words to wrap around the panel edge.\n\n- item one\n- [x] done\n  1. nested\n\n> quoted line\n\n```rs\nfn x() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n";
    block.repeat(200)
}

fn editor_with(name: &str, text: &str) -> Editor {
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    ed.buffers[0] = Buffer::from_text(Some(name.into()), text);
    ed
}

fn keys(ed: &mut Editor, typed: &str) {
    for key in phrase_keys(typed) {
        ed.handle(key);
    }
}

const SCREEN: Size = Size { cols: 160, rows: 48 };

/// (name, budget, run): each case builds its own state outside the timed part where it can.
type Case = (&'static str, Duration, Box<dyn Fn() -> Duration>);

fn timed(f: impl FnOnce()) -> Duration {
    let start: Instant = Instant::now();
    f();
    start.elapsed()
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
#[expect(clippy::too_many_lines, reason = "one entry per timed case")]
fn perf() {
    let cases: Vec<Case> = vec![
        (
            "startup: editor + first frame",
            ms(50),
            Box::new(|| {
                timed(|| {
                    let mut ed: Editor = Editor::new(&[]);
                    Screen::new(ColorDepth::TrueColor).draw(&mut ed, SCREEN);
                })
            }),
        ),
        (
            "hex: dump + read back 256 KB binary",
            ms(100),
            Box::new(|| {
                let bytes: Vec<u8> = (0..1 << 18).map(|i: u32| u8::try_from(i * 7 % 251).unwrap_or(0)).collect();
                timed(|| {
                    let lines: Vec<String> = crate::buffer::hex::dump(&bytes, 0);
                    assert_eq!(crate::buffer::hex::undump(&lines).unwrap().len(), bytes.len());
                })
            }),
        ),
        (
            "open 10k-line file + draw",
            ms(100),
            Box::new(|| {
                let text: String = big_rust();
                timed(|| {
                    let mut ed: Editor = editor_with("big.rs", &text);
                    Screen::new(ColorDepth::TrueColor).draw(&mut ed, SCREEN);
                })
            }),
        ),
        (
            "scroll 10k lines, 200 frames",
            ms(2000),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
                screen.draw(&mut ed, SCREEN);
                timed(|| {
                    for _ in 0..200 {
                        ed.handle(Key::Ctrl('d'));
                        screen.draw(&mut ed, SCREEN);
                    }
                })
            }),
        ),
        (
            "type 500 chars, frame per key",
            ms(4000),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
                keys(&mut ed, "5000Go");
                timed(|| {
                    for c in "let value: usize = compute(42); ".chars().cycle().take(500) {
                        ed.handle(Key::Char(c));
                        screen.draw(&mut ed, SCREEN);
                    }
                })
            }),
        ),
        (
            "type 100 chars in a 1 MB line, frame per key",
            ms(1500),
            Box::new(|| {
                let mut ed: Editor = editor_with("min.js", &"var a=\"x\";f(a,1);".repeat(60_000));
                let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
                keys(&mut ed, "i");
                timed(|| {
                    for c in "x(1);".chars().cycle().take(100) {
                        ed.handle(Key::Char(c));
                        screen.draw(&mut ed, SCREEN);
                    }
                })
            }),
        ),
        (
            "gitsigns diff 20k lines: 10% edited, then all unrelated",
            ms(100),
            Box::new(|| {
                let old: Vec<String> = (0..20_000).map(|n| format!("line {n}")).collect();
                let edited: Vec<String> = old
                    .iter()
                    .enumerate()
                    .map(|(n, l)| if n % 10 == 0 { format!("{l}!") } else { l.clone() })
                    .collect();
                let unrelated: Vec<String> = (0..20_000).map(|n| format!("other {n}")).collect();
                timed(|| {
                    crate::buffer::git::diff(&old, &edited);
                    crate::buffer::git::diff(&old, &unrelated);
                })
            }),
        ),
        (
            "regex search x50 over 10k lines",
            ms(100),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                keys(&mut ed, "/\\v\\w+_\\d+\\(\n");
                timed(|| keys(&mut ed, &"n".repeat(50)))
            }),
        ),
        (
            ":%s with groups on 10k lines",
            ms(1200),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                timed(|| keys(&mut ed, ":%s/\\v(\\w+): (\\w+)/\\2 \\1/g\n"))
            }),
        ),
        (
            ":g/TODO/d on 10k lines",
            ms(500),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                timed(|| keys(&mut ed, ":g/TODO/d\n"))
            }),
        ),
        (
            ":g/item/norm Ax on 10k lines",
            ms(500),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                timed(|| keys(&mut ed, ":g/item/norm Ax\n"))
            }),
        ),
        (
            "undo + redo 300 steps",
            ms(50),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                for _ in 0..300 {
                    keys(&mut ed, "ddj");
                }
                timed(|| {
                    keys(&mut ed, &"u".repeat(300));
                    for _ in 0..300 {
                        ed.handle(Key::Ctrl('r'));
                    }
                })
            }),
        ),
        (
            "markdown preview, 3.6k lines, 50 frames",
            ms(3000),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.md", &big_markdown());
                let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
                keys(&mut ed, " um");
                timed(|| {
                    for _ in 0..50 {
                        keys(&mut ed, "ix\x1b");
                        screen.draw(&mut ed, SCREEN);
                    }
                })
            }),
        ),
        (
            "lines picker, filter 10k lines",
            ms(1000),
            Box::new(|| {
                let mut ed: Editor = editor_with("big.rs", &big_rust());
                let mut screen: Screen = Screen::new(ColorDepth::TrueColor);
                timed(|| {
                    keys(&mut ed, " fl");
                    for c in "parsedsome".chars() {
                        ed.handle(Key::Char(c));
                        screen.draw(&mut ed, SCREEN);
                    }
                })
            }),
        ),
    ];
    let mut slow: Vec<String> = Vec::new();
    eprintln!("\n{:<42} {:>10} {:>10}", "perf case", "time", "budget");
    for (name, budget, run) in &cases {
        let took: Duration = run();
        let mark: &str = if took > *budget { "  SLOW" } else { "" };
        eprintln!("{name:<42} {:>8.1}ms {:>8}ms{mark}", took.as_secs_f64() * 1000.0, budget.as_millis());
        if took > *budget {
            slow.push(format!("{name}: {took:?} > {budget:?}"));
        }
    }
    assert!(slow.is_empty(), "over budget: {slow:?}");
}

/// Prints the timings as `perf` does and fails on any over its budget.
fn check_budgets(rows: &[(&str, Duration, Duration)]) {
    eprintln!("\n{:<42} {:>10} {:>10}", "big file case", "time", "budget");
    for (name, took, budget) in rows {
        let mark: &str = if took > budget { "  SLOW" } else { "" };
        eprintln!("{name:<42} {:>8.1}ms {:>8}ms{mark}", took.as_secs_f64() * 1000.0, budget.as_millis());
    }
    let slow: Vec<&str> = rows.iter().filter(|(_, took, budget)| took > budget).map(|(name, _, _)| *name).collect();
    assert!(slow.is_empty(), "over budget: {slow:?}");
}

/// A 100 MB text file and a 64 MB binary (the hex limit), opened, browsed, edited and written back: every step
/// stays usable and the bytes on disk are right. Writes ~170 MB to the temp dir and needs a release build:
/// `cargo test --release big_files -- --ignored --nocapture`.
#[test]
#[ignore = "writes ~170 MB to the temp dir; run with --release"]
fn big_files() {
    let dir: PathBuf = std::env::temp_dir().join(format!("lim-big-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut rows: Vec<(&str, Duration, Duration)> = Vec::new();
    let mut screen: Screen = Screen::new(ColorDepth::TrueColor);

    // Text: 300 copies of the 10k-line Rust sample, 3M lines, and one marker line at the end for the search.
    let text_path: PathBuf = dir.join("big.rs");
    let text: String = big_rust().repeat(300) + "// the only end marker\n";
    let (size, line_count): (usize, usize) = (text.len(), text.lines().count());
    assert!(size > 100_000_000, "{size}");
    fs::write(&text_path, &text).unwrap();
    drop(text);
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    let open: String = format!(":e {}\n", text_path.display());
    rows.push((
        "text 100 MB: open + draw",
        timed(|| {
            keys(&mut ed, &open);
            screen.draw(&mut ed, SCREEN);
        }),
        ms(3000),
    ));
    assert_eq!(ed.buf().line_count(), line_count);
    // The first far jump scans every line for comments and strings (exact colors), then checkpoints make it cheap.
    rows.push(("text 100 MB: G + draw (exact syntax state)", timed(|| draw_after(&mut ed, &mut screen, "G")), ms(300)));
    rows.push(("text 100 MB: gg, G again", timed(|| draw_after(&mut ed, &mut screen, "ggG")), ms(50)));
    rows.push((
        "text 100 MB: search to the end",
        timed(|| draw_after(&mut ed, &mut screen, "gg/end marker\n")),
        ms(3000),
    ));
    assert_eq!(ed.buf().cursor.line, line_count - 1, "the marker is on the last line");
    rows.push((
        "text 100 MB: type at the top + draw",
        timed(|| draw_after(&mut ed, &mut screen, "ggihello \x1b")),
        ms(50),
    ));
    rows.push(("text 100 MB: undo + redo", timed(|| draw_after(&mut ed, &mut screen, "u\x12")), ms(100)));
    rows.push(("text 100 MB: :w", timed(|| keys(&mut ed, ":w\n")), ms(3000)));
    assert_eq!(fs::metadata(&text_path).unwrap().len(), u64::try_from(size + "hello ".len()).unwrap());
    drop(ed);

    // Binary: exactly the 64 MB hex limit, 4 KB blocks of non-UTF-8 noise with two strings each.
    let bin_path: PathBuf = dir.join("big.bin");
    let mut block: Vec<u8> =
        (0..4096_u32).map(|i| 0x80 | u8::try_from(i.wrapping_mul(2_654_435_761) >> 25).unwrap()).collect();
    block[100..112].copy_from_slice(b"hello world!");
    block[2000..2008].copy_from_slice(b"Kernel32");
    let bytes: Vec<u8> = block.repeat((64 << 20) / 4096);
    fs::write(&bin_path, &bytes).unwrap();
    let mut ed: Editor = Editor::new(&[]);
    ed.dashboard = false;
    let open: String = format!(":e {}\n", bin_path.display());
    rows.push((
        "hex 64 MB: open + draw",
        timed(|| {
            keys(&mut ed, &open);
            screen.draw(&mut ed, SCREEN);
        }),
        ms(5000),
    ));
    assert!(ed.buf().hex);
    assert_eq!(ed.buf().line_count(), bytes.len() / 16);
    rows.push(("hex 64 MB: G + draw", timed(|| draw_after(&mut ed, &mut screen, "G")), ms(50)));
    rows.push(("hex 64 MB: :detect", timed(|| keys(&mut ed, ":detect\n")), ms(3000)));
    assert!(ed.message_text().starts_with("data · 67108864 bytes"), "{}", ed.message_text());
    rows.push(("hex 64 MB: :strings", timed(|| keys(&mut ed, ":strings\n")), ms(5000)));
    assert_eq!(ed.buf().line_count(), 2 * (64 << 20) / 4096);
    assert_eq!(ed.buf().line(1), "000007d0: Kernel32");
    keys(&mut ed, "\x1e");
    assert!(ed.buf().hex, "back on the binary");
    rows.push(("hex 64 MB: edit a byte + draw", timed(|| draw_after(&mut ed, &mut screen, "gg010lr0lr0")), ms(50)));
    rows.push(("hex 64 MB: :xor one line", timed(|| keys(&mut ed, ":2xor 0xff\n")), ms(5000)));
    rows.push(("hex 64 MB: :w", timed(|| keys(&mut ed, ":w\n")), ms(5000)));
    let written: Vec<u8> = fs::read(&bin_path).unwrap();
    assert_eq!(written.len(), bytes.len());
    assert_eq!(written[0], 0);
    assert!(written[16..32].iter().zip(&bytes[16..32]).all(|(w, b)| *w == b ^ 0xff));
    assert_eq!(written[32..], bytes[32..], "nothing else moved");
    drop(ed);
    let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    check_budgets(&rows);
}

fn draw_after(ed: &mut Editor, screen: &mut Screen, typed: &str) {
    keys(ed, typed);
    screen.draw(ed, SCREEN);
}

/// The same operations on the same file in Neovim (`-u NONE`: no config, no plugins, its fastest) and timed inside
/// it, best of 5 each.
const NVIM_BENCH: &str = r"
local file = os.getenv('LIM_BENCH_FILE')
local out = {}
local function bench(name, setup, run)
  local best = math.huge
  for _ = 1, 5 do
    vim.cmd('silent edit! ' .. vim.fn.fnameescape(file))
    setup()
    local t = vim.uv.hrtime()
    run()
    best = math.min(best, (vim.uv.hrtime() - t) / 1e6)
  end
  out[#out + 1] = string.format('%s=%.4f', name, best)
end
local none = function() end
bench('open', none, function() vim.cmd('silent edit! ' .. vim.fn.fnameescape(file)) end)
bench('substitute', none, function() vim.cmd([[silent! %s/\v(\w+): (\w+)/\2 \1/g]]) end)
bench('global_delete', none, function() vim.cmd('silent! g/TODO/d') end)
bench('global_norm', none, function() vim.cmd('silent! g/item/norm Ax') end)
bench('search', function() vim.fn.setreg('/', [[\v\w+_\d+\(]]) end, function()
  for _ = 1, 50 do vim.cmd('silent! normal! n') end
end)
bench('typing', function() vim.cmd('normal! 5000G') end, function()
  vim.cmd('normal! o' .. string.rep('let value: usize = compute(42); ', 16):sub(1, 500))
end)
bench('undo_redo', function()
  for _ = 1, 300 do vim.cmd('normal! ddj') end
end, function()
  for _ = 1, 300 do vim.cmd('silent! undo') end
  for _ = 1, 300 do vim.cmd('silent! redo') end
end)
bench('sort', none, function() vim.cmd('silent sort') end)
io.stdout:write(table.concat(out, '\n') .. '\n')
";

/// lim against Neovim on every operation both can do; lim must win each one. Needs `nvim` (or `LIM_NVIM`) and a
/// release build: `cargo test --release vs_neovim -- --ignored --nocapture`.
#[test]
#[ignore = "needs nvim and --release"]
fn vs_neovim() {
    let nvim: String = std::env::var("LIM_NVIM").unwrap_or_else(|_| "nvim".into());
    let dir: PathBuf = std::env::temp_dir().join(format!("lim-vs-nvim-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let (file, script): (PathBuf, PathBuf) = (dir.join("big.rs"), dir.join("bench.lua"));
    fs::write(&file, big_rust()).unwrap();
    fs::write(&script, NVIM_BENCH).unwrap();
    let out: Output = Command::new(&nvim)
        .args(["--headless", "-u", "NONE", "-i", "NONE", "-n", "-c"])
        .arg(format!("luafile {}", script.display()))
        .args(["-c", "qa!"])
        .env("LIM_BENCH_FILE", &file)
        .output()
        .unwrap_or_else(|e| panic!("{nvim}: {e} (set LIM_NVIM to its path)"));
    let report: String = String::from_utf8_lossy(&out.stdout).into_owned();
    let neovim = |name: &str| -> f64 {
        report
            .lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix('=')?.parse().ok())
            .unwrap_or_else(|| panic!("no {name} in nvim output: {report}{}", String::from_utf8_lossy(&out.stderr)))
    };
    // The same steps in lim, keys as typed, no frames (Neovim runs headless too); best of 5.
    let best = |setup: &dyn Fn(&mut Editor), typed: &str| -> f64 {
        (0..5)
            .map(|_| {
                let mut ed: Editor = Editor::new(&[]);
                ed.dashboard = false;
                ed.buffers[0] = Buffer::open(&file).unwrap();
                setup(&mut ed);
                timed(|| keys(&mut ed, typed)).as_secs_f64() * 1000.0
            })
            .fold(f64::INFINITY, f64::min)
    };
    let none = |_: &mut Editor| {};
    let open: f64 = (0..5)
        .map(|_| {
            timed(|| {
                let mut ed: Editor = Editor::new(&[]);
                ed.buffers[0] = Buffer::open(&file).unwrap();
            })
            .as_secs_f64()
                * 1000.0
        })
        .fold(f64::INFINITY, f64::min);
    let typing: String =
        format!("o{}", "let value: usize = compute(42); ".repeat(16).chars().take(500).collect::<String>());
    let rows: Vec<(&str, f64)> = vec![
        ("open", open),
        ("substitute", best(&none, ":%s/\\v(\\w+): (\\w+)/\\2 \\1/g\n")),
        ("global_delete", best(&none, ":g/TODO/d\n")),
        ("global_norm", best(&none, ":g/item/norm Ax\n")),
        ("search", best(&|ed| keys(ed, "/\\v\\w+_\\d+\\(\n"), &"n".repeat(50))),
        ("typing", best(&|ed| keys(ed, "5000G"), &typing)),
        ("undo_redo", best(&|ed| keys(ed, &"ddj".repeat(300)), &format!("{}{}", "u".repeat(300), "\x12".repeat(300)))),
        ("sort", best(&none, ":sort\n")),
    ];
    let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    eprintln!("\n{:<16} {:>10} {:>10} {:>8}", "operation", "lim", "neovim", "faster");
    let mut slower: Vec<String> = Vec::new();
    for (name, lim) in rows {
        let nv: f64 = neovim(name);
        eprintln!("{name:<16} {lim:>8.2}ms {nv:>8.2}ms {:>7.1}x", nv / lim.max(0.001));
        if lim >= nv {
            slower.push(format!("{name}: lim {lim:.2}ms, neovim {nv:.2}ms"));
        }
    }
    assert!(slower.is_empty(), "slower than Neovim: {slower:?}");
}
