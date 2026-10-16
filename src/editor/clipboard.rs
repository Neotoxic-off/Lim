//! The system clipboard (`clipboard=unnamedplus`, as `AstroNvim` sets it): wl-copy/wl-paste, xclip, xsel or
//! pbcopy/pbpaste, whichever exists, and the Win32 clipboard on Windows; otherwise copying falls back to OSC 52.

/// True when the system clipboard took the text.
pub fn copy(text: &str) -> bool {
    !cfg!(test) && system::copy(text)
}

pub fn paste() -> Option<String> {
    if cfg!(test) { None } else { system::paste() }
}

#[cfg(unix)]
mod system {
    use std::env;
    use std::io::{self, Write};
    use std::process::{Child, Command, Output, Stdio};

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

    pub fn copy(text: &str) -> bool {
        tools().0.iter().any(|argv| {
            let Some((program, args)): Option<(&&str, &[&str])> = argv.split_first() else { return false };
            let child: io::Result<Child> = Command::new(program)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child): io::Result<Child> = child else { return false };
            let written: bool = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
            // These tools fork to keep serving the selection, so waiting returns promptly.
            child.wait().is_ok_and(|s| s.success()) && written
        })
    }

    pub fn paste() -> Option<String> {
        tools().1.iter().find_map(|argv| {
            let (program, args): (&&str, &[&str]) = argv.split_first()?;
            let out: Output =
                Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
            out.status.success().then(|| String::from_utf8(out.stdout).ok()).flatten()
        })
    }
}

/// `CF_UNICODETEXT` through user32/kernel32 FFI; Windows text uses CRLF line ends.
#[cfg(windows)]
mod system {
    use std::ffi::c_void;
    use std::ptr;

    type Handle = *mut c_void;

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x2;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(owner: Handle) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> Handle;
        fn SetClipboardData(format: u32, memory: Handle) -> Handle;
    }

    unsafe extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
        fn GlobalFree(memory: Handle) -> Handle;
        fn GlobalLock(memory: Handle) -> *mut c_void;
        fn GlobalUnlock(memory: Handle) -> i32;
    }

    /// Runs `f` with the clipboard open, closing it afterwards.
    fn with_clipboard<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
        if unsafe { OpenClipboard(ptr::null_mut()) } == 0 {
            return None;
        }
        let result: Option<T> = f();
        unsafe { CloseClipboard() };
        result
    }

    pub fn copy(text: &str) -> bool {
        let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain([0]).collect();
        with_clipboard(|| unsafe {
            if EmptyClipboard() == 0 {
                return None;
            }
            let memory: Handle = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>());
            let target: *mut u16 = GlobalLock(memory).cast::<u16>();
            if target.is_null() {
                GlobalFree(memory);
                return None;
            }
            ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            GlobalUnlock(memory);
            // On success the clipboard owns the memory; on failure it is still ours to free.
            if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
                GlobalFree(memory);
                return None;
            }
            Some(())
        })
        .is_some()
    }

    pub fn paste() -> Option<String> {
        with_clipboard(|| unsafe {
            let memory: Handle = GetClipboardData(CF_UNICODETEXT);
            let source: *const u16 = GlobalLock(memory).cast::<u16>().cast_const();
            if source.is_null() {
                return None;
            }
            // CF_UNICODETEXT is NUL-terminated.
            let mut len: usize = 0;
            while *source.add(len) != 0 {
                len += 1;
            }
            let text: String = String::from_utf16_lossy(std::slice::from_raw_parts(source, len));
            GlobalUnlock(memory);
            Some(text.replace("\r\n", "\n"))
        })
    }
}

/// OSC 52 sequence that asks the terminal itself to set the clipboard (works over SSH).
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out: String = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n: u32 = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
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
