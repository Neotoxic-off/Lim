//! Terminal: raw mode, size, color depth and key input, through libc FFI only.

mod input;
mod sys;

use std::io::{self, Write};

pub use input::{Event, Input, Key};

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
    original: sys::Termios,
}

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        let original = sys::get_mode()?;
        sys::set_mode(&sys::raw_mode(original))?;
        sys::watch_resize();
        let mut out = io::stdout();
        out.write_all(b"\x1b[?1049h")?;
        out.flush()?;
        Ok(Self { original })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[0m\x1b[0 q\x1b[?25h\x1b[?1049l");
        let _ = out.flush();
        let _ = sys::set_mode(&self.original);
    }
}

pub fn size() -> Size {
    sys::window_size().map_or(Size { cols: 80, rows: 24 }, |(cols, rows)| Size {
        cols: cols.into(),
        rows: rows.into(),
    })
}

pub fn color_depth() -> ColorDepth {
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    let truecolor = colorterm.contains("truecolor")
        || colorterm.contains("24bit")
        || term.ends_with("-direct")
        || ["kitty", "alacritty", "wezterm", "ghostty", "foot"].iter().any(|t| term.contains(t));
    if truecolor { ColorDepth::TrueColor } else { ColorDepth::Ansi256 }
}
