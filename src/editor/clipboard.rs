//! The system clipboard (`clipboard=unnamedplus`, as AstroNvim sets it): wl-copy/wl-paste,
//! xclip, xsel or pbcopy/pbpaste, whichever exists; otherwise copying falls back to OSC 52.

use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

fn tools() -> (&'static [&'static [&'static str]], &'static [&'static [&'static str]]) {
    if cfg!(target_os = "macos") {
        return (&[&["pbcopy"]], &[&["pbpaste"]]);
    }
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        return (&[&["wl-copy"]], &[&["wl-paste", "--no-newline"]]);
    }
    (
        &[&["xclip", "-selection", "clipboard"], &["xsel", "--clipboard", "--input"]],
        &[&["xclip", "-selection", "clipboard", "-o"], &["xsel", "--clipboard", "--output"]],
    )
}

/// True when a clipboard tool took the text.
pub fn copy(text: &str) -> bool {
    if cfg!(test) {
        return false;
    }
    tools().0.iter().any(|argv| {
        let Some((program, args)) = argv.split_first() else { return false };
        let child = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        let Ok(mut child) = child else { return false };
        let written = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        // These tools fork to keep serving the selection, so waiting returns promptly.
        child.wait().is_ok_and(|s| s.success()) && written
    })
}

pub fn paste() -> Option<String> {
    if cfg!(test) {
        return None;
    }
    tools().1.iter().find_map(|argv| {
        let (program, args) = argv.split_first()?;
        let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8(out.stdout).ok()).flatten()
    })
}

/// OSC 52 sequence that asks the terminal itself to set the clipboard (works over SSH).
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }
}
