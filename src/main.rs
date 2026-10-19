//! lim: Neovim fully set up as `AstroNvim`, built in: no Lua, no plugin manager, no dependencies.

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
#[cfg(test)]
mod fuzz;
#[cfg(test)]
mod perf;
mod render;
mod syntax;
mod term;
mod update;

use std::any::Any;
use std::env;
use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{self, Path};
use std::process::ExitCode;
use std::sync::Mutex;

use editor::Editor;
use render::Screen;
use term::{Event, Input, Terminal};

/// Where the panic hook leaves its message for the recovery path.
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: lim [dir] [file...] | --version | --print-config | --config-path");
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
    let (config, mut warnings): (config::Config, Vec<String>) = config::load();
    let colorscheme: String = render::initial_colorscheme(&config);
    warnings.extend(render::apply_colorscheme(&colorscheme, &config));
    // `lim some/dir` works inside that folder, as `cd some/dir && nvim .`; files keep their meaning.
    let (dirs, mut files): (Vec<String>, Vec<String>) = args.into_iter().partition(|a| Path::new(a).is_dir());
    if let Some(dir) = dirs.first() {
        files =
            files.iter().map(|f| path::absolute(f).map_or_else(|_| f.clone(), |p| p.display().to_string())).collect();
        if let Err(e) = env::set_current_dir(dir) {
            warnings.push(format!("{dir}: {e}"));
        }
    }
    let mut editor: Editor = Editor::new(&files);
    editor.configure(&config);
    editor.colorscheme = colorscheme;
    if !warnings.is_empty() {
        editor.set_message(editor::Message::Error(format!("config: {}", warnings.join("; "))));
    }
    let result: Result<(), io::Error> = run(&mut editor);
    editor.autosave_session();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lim: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(editor: &mut Editor) -> io::Result<()> {
    let terminal: Terminal = Terminal::enter()?;
    // A bug must never kill the session: record it quietly, recover, keep editing.
    panic::set_hook(Box::new(|info| {
        let what: String = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let place: String = info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = format!("{what}{place}");
        }
    }));
    let mut input: Input = Input::default();
    let mut screen: Screen = Screen::new(term::color_depth());
    let mut out: io::StdoutLock<'_> = io::stdout().lock();
    while !editor.quit {
        // Skip frames while keys are still queued (pastes, key repeat).
        if !input.has_pending() {
            let frame: String =
                if let Ok(frame) = panic::catch_unwind(AssertUnwindSafe(|| screen.draw(editor, term::size()))) {
                    frame
                } else {
                    recover(editor, &mut screen);
                    // Drawing itself failed: show the error plainly so `:w`/`:q` stay usable.
                    format!("\x1b[0m\x1b[2J\x1b[H{}", editor.message_text().replace(char::is_control, "?"))
                };
            out.write_all(frame.as_bytes())?;
            out.flush()?;
        }
        let event: Event = input.read(editor.deadline())?;
        let handled: Result<(), Box<dyn Any + Send>> = panic::catch_unwind(AssertUnwindSafe(|| match event {
            Event::Key(key) => editor.handle(key),
            Event::Timeout => editor.tick(),
            Event::Resize => {}
        }));
        if handled.is_err() {
            recover(editor, &mut screen);
        }
        if let Some(command) = editor.shell_requested.take() {
            if let Err(e) = terminal.run(&command) {
                editor.set_message(editor::Message::Error(format!(
                    "{}: {e}",
                    command.first().map_or("shell", String::as_str)
                )));
            }
            editor.reload_changed_buffers();
            screen.invalidate();
        }
        if let Some(sequence) = editor.osc52.take() {
            out.write_all(sequence.as_bytes())?;
            out.flush()?;
        }
    }
    Ok(())
}

fn recover(editor: &mut Editor, screen: &mut Screen) {
    let detail: String = LAST_PANIC.lock().map(|m| m.clone()).unwrap_or_default();
    editor.recover(&detail);
    screen.invalidate();
}
