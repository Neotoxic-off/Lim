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
use render::Screen;
use term::{Event, Input, Terminal};

/// Where the panic hook leaves its message for the recovery path.
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: lim [file...]");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("lim {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let mut editor = Editor::new(&args);
    match run(&mut editor) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lim: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(editor: &mut Editor) -> io::Result<()> {
    let _terminal = Terminal::enter()?;
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
    let mut screen = Screen::new(term::color_depth());
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
        if let Event::Key(key) = input.read()?
            && panic::catch_unwind(AssertUnwindSafe(|| editor.handle(key))).is_err()
        {
            recover(editor, &mut screen);
        }
    }
    Ok(())
}

fn recover(editor: &mut Editor, screen: &mut Screen) {
    let detail = LAST_PANIC.lock().map(|m| m.clone()).unwrap_or_default();
    editor.recover(&detail);
    screen.invalidate();
}
