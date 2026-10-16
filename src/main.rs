//! lim: a lightweight, dependency-free modal editor modelled on an AstroNvim setup.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod buffer;
mod config;
mod editor;
mod error;
mod render;
mod syntax;
mod term;

use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::process::ExitCode;
use std::sync::Mutex;

use editor::Editor;
use render::{Screen, Theme};
use term::{ColorDepth, Event, Input, Terminal};

/// Where the panic hook leaves its message for the recovery path.
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

fn main() -> ExitCode {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: lim [file...]");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("lim {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--print-config") {
        print!("{}", config::EXAMPLE);
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--config-path") {
        println!("{}", config::path().map(|p| p.display().to_string()).unwrap_or_default());
        return ExitCode::SUCCESS;
    }
    let (config, mut warnings) = config::load();
    let (theme, theme_warnings) = Theme::build(&config);
    warnings.extend(theme_warnings);
    render::install_theme(theme);
    let depth = match config.options.truecolor {
        Some(true) => ColorDepth::TrueColor,
        Some(false) => ColorDepth::Ansi256,
        None => term::color_depth(),
    };
    let mut editor = Editor::new(&args);
    editor.started = started;
    editor.configure(&config);
    if !warnings.is_empty() {
        editor.set_message(editor::Message::Error(format!("config: {}", warnings.join("; "))));
    }
    match run(&mut editor, depth) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lim: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(editor: &mut Editor, depth: ColorDepth) -> io::Result<()> {
    let terminal = Terminal::enter()?;
    // A bug must never kill the session: record it quietly, recover, keep editing.
    panic::set_hook(Box::new(|info| {
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let place = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = format!("{what}{place}");
        }
    }));
    let mut input = Input::default();
    let mut screen = Screen::new(depth);
    let mut out = io::stdout().lock();
    while !editor.quit {
        // Skip frames while keys are still queued (pastes, key repeat).
        if !input.has_pending() {
            let frame = match panic::catch_unwind(AssertUnwindSafe(|| screen.draw(editor, term::size()))) {
                Ok(frame) => frame,
                Err(_) => {
                    recover(editor, &mut screen);
                    // Drawing itself failed: show the error plainly so `:w`/`:q` stay usable.
                    format!("\x1b[0m\x1b[2J\x1b[H{}", editor.message_text().replace(char::is_control, "?"))
                }
            };
            out.write_all(frame.as_bytes())?;
            out.flush()?;
        }
        let event = input.read(editor.deadline())?;
        let handled = panic::catch_unwind(AssertUnwindSafe(|| match event {
            Event::Key(key) => editor.handle(key),
            Event::Timeout => editor.tick(),
            Event::Resize => {}
        }));
        if handled.is_err() {
            recover(editor, &mut screen);
        }
        if std::mem::take(&mut editor.shell_requested) {
            if let Err(e) = terminal.run_shell() {
                editor.set_message(editor::Message::Error(format!("shell: {e}")));
            }
            screen.invalidate();
        }
    }
    Ok(())
}

fn recover(editor: &mut Editor, screen: &mut Screen) {
    let detail = LAST_PANIC.lock().map(|m| m.clone()).unwrap_or_default();
    editor.recover(&detail);
    screen.invalidate();
}
