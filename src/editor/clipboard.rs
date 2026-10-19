//! The system clipboard (`clipboard=unnamedplus`, as `AstroNvim` sets it) through arboard: Wayland, X11, macOS and
//! Windows; otherwise copying falls back to OSC 52.

use std::sync::{Mutex, MutexGuard, OnceLock};

use arboard::Clipboard;
use data_encoding::BASE64;

/// One clipboard handle for the session: on X11 it serves what was copied, which a dropped handle would stop doing.
fn with<T>(f: impl FnOnce(&mut Clipboard) -> Option<T>) -> Option<T> {
    static CLIPBOARD: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();
    let mut slot: MutexGuard<'_, Option<Clipboard>> =
        CLIPBOARD.get_or_init(|| Mutex::new(Clipboard::new().ok())).lock().ok()?;
    f(slot.as_mut()?)
}

/// True when the system clipboard took the text.
pub fn copy(text: &str) -> bool {
    // Windows text uses CRLF line ends.
    let text: String = if cfg!(windows) { text.replace('\n', "\r\n") } else { text.to_owned() };
    !cfg!(test) && with(|c| c.set_text(text).ok()).is_some()
}

pub fn paste() -> Option<String> {
    if cfg!(test) { None } else { with(|c| c.get_text().ok()).map(|t| t.replace("\r\n", "\n")) }
}

/// OSC 52 sequence that asks the terminal itself to set the clipboard (works over SSH).
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", BASE64.encode(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_is_base64() {
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }
}
