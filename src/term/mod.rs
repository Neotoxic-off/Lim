//! Terminal: raw mode, size, color depth and key input, through libc / kernel32 FFI only.

mod input;
#[cfg(unix)]
#[path = "sys/unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "sys/windows.rs"]
mod sys;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
compile_error!("lim supports Linux, macOS and Windows");

use std::{
    ffi::OsString,
    io::{self, Write},
    process::ExitStatus,
};

pub use input::{Event, Input, Key, Mouse, MouseKind};
pub use sys::{https_get, random, utc_offset};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
}

/// Raw mode on the alternate screen; restored on drop, including during a panic unwind.
pub struct Terminal {
    original: sys::Mode,
}

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        let original: sys::Mode = sys::get_mode()?;
        sys::set_mode(&sys::raw_mode(original))?;
        sys::watch_resize();
        let mut out: io::Stdout = io::stdout();
        out.write_all(b"\x1b[?1049h")?;
        out.flush()?;
        Ok(Self { original })
    }
}

impl Terminal {
    /// Hands the terminal to `command` (`$SHELL`, or `%COMSPEC%` on Windows, when empty) and takes it back when it
    /// exits.
    pub fn run(&self, command: &[String]) -> io::Result<()> {
        let mut out: io::Stdout = io::stdout();
        out.write_all(b"\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1002l\x1b[?1006l\x1b[?1049l")?;
        out.flush()?;
        sys::set_mode(&self.original)?;
        let (shell_var, fallback): (&str, &str) =
            if cfg!(windows) { ("COMSPEC", "cmd.exe") } else { ("SHELL", "/bin/sh") };
        let shell: OsString = std::env::var_os(shell_var).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.into());
        let status: Result<ExitStatus, io::Error> = match command.split_first() {
            Some((program, args)) => std::process::Command::new(program).args(args).status(),
            None => std::process::Command::new(shell).status(),
        };
        sys::set_mode(&sys::raw_mode(self.original))?;
        out.write_all(b"\x1b[?1049h")?;
        out.flush()?;
        status.map(|_| ())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let mut out: io::Stdout = io::stdout();
        let _: Result<(), io::Error> = out.write_all(b"\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1002l\x1b[?1006l\x1b[?1049l");
        let _: Result<(), io::Error> = out.flush();
        let _: Result<(), io::Error> = sys::set_mode(&self.original);
    }
}

pub fn size() -> Size {
    sys::window_size().map_or(Size { cols: 80, rows: 24 }, |(cols, rows)| Size { cols: cols.into(), rows: rows.into() })
}

pub fn color_depth() -> ColorDepth {
    let colorterm: String = std::env::var("COLORTERM").unwrap_or_default();
    let term: String = std::env::var("TERM").unwrap_or_default();
    // Windows Terminal and conhost (since Windows 10 1703) render 24-bit color.
    let truecolor: bool = cfg!(windows)
        || colorterm.contains("truecolor")
        || colorterm.contains("24bit")
        || term.ends_with("-direct")
        || ["kitty", "alacritty", "wezterm", "ghostty", "foot"].iter().any(|t| term.contains(t));
    if truecolor { ColorDepth::TrueColor } else { ColorDepth::Ansi256 }
}
