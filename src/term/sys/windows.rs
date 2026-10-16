//! Raw kernel32 bindings for console modes, window size, waiting and reading input.
//! With `ENABLE_VIRTUAL_TERMINAL_INPUT` the console sends the same escape sequences as a Unix terminal, so the
//! key decoder is shared; input is read as UTF-16 records and handed on as UTF-8 bytes.

use std::ffi::c_void;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

type Handle = *mut c_void;

/// `(DWORD)-10` and `(DWORD)-11`.
const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;

const ENABLE_PROCESSED_INPUT: u32 = 0x1;
const ENABLE_LINE_INPUT: u32 = 0x2;
const ENABLE_ECHO_INPUT: u32 = 0x4;
const ENABLE_WINDOW_INPUT: u32 = 0x8;
const ENABLE_MOUSE_INPUT: u32 = 0x10;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x200;

const ENABLE_PROCESSED_OUTPUT: u32 = 0x1;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x4;
const DISABLE_NEWLINE_AUTO_RETURN: u32 = 0x8;

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x102;
const INFINITE: u32 = u32::MAX;

const KEY_EVENT: u16 = 0x1;
const WINDOW_BUFFER_SIZE_EVENT: u16 = 0x4;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord {
    x: i16,
    y: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SmallRect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}

#[repr(C)]
#[derive(Default)]
struct ScreenBufferInfo {
    size: Coord,
    cursor: Coord,
    attributes: u16,
    window: SmallRect,
    max_window: Coord,
}

/// `KEY_EVENT_RECORD`, the largest member of the `INPUT_RECORD` union (16 bytes); other events reuse its bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyEvent {
    key_down: i32,
    repeat: u16,
    virtual_key: u16,
    scan_code: u16,
    unicode: u16,
    control_state: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InputRecord {
    kind: u16,
    event: KeyEvent,
}

unsafe extern "system" {
    fn GetStdHandle(which: u32) -> Handle;
    fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
    fn SetConsoleMode(console: Handle, mode: u32) -> i32;
    fn GetConsoleScreenBufferInfo(console: Handle, info: *mut ScreenBufferInfo) -> i32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn GetNumberOfConsoleInputEvents(console: Handle, count: *mut u32) -> i32;
    fn ReadConsoleInputW(console: Handle, records: *mut InputRecord, len: u32, read: *mut u32) -> i32;
}

/// The input and output console modes, saved on entry and restored on exit.
#[derive(Clone, Copy)]
pub struct Mode {
    input: u32,
    output: u32,
}

fn stdin() -> Handle {
    unsafe { GetStdHandle(STD_INPUT_HANDLE) }
}

fn stdout() -> Handle {
    unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }
}

fn check(ok: i32) -> io::Result<()> {
    if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

/// Fails when stdin or stdout is not a console (redirected).
pub fn get_mode() -> io::Result<Mode> {
    let (mut input, mut output): (u32, u32) = (0, 0);
    check(unsafe { GetConsoleMode(stdin(), &raw mut input) })?;
    check(unsafe { GetConsoleMode(stdout(), &raw mut output) })?;
    Ok(Mode { input, output })
}

pub fn set_mode(mode: &Mode) -> io::Result<()> {
    check(unsafe { SetConsoleMode(stdin(), mode.input) })?;
    check(unsafe { SetConsoleMode(stdout(), mode.output) })
}

/// Keys as VT sequences, Ctrl-C as a key, resize events; output interprets VT sequences.
pub fn raw_mode(mode: Mode) -> Mode {
    Mode {
        input: mode.input & !(ENABLE_PROCESSED_INPUT | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_MOUSE_INPUT)
            | ENABLE_WINDOW_INPUT
            | ENABLE_VIRTUAL_TERMINAL_INPUT,
        output: mode.output
            | ENABLE_PROCESSED_OUTPUT
            | ENABLE_VIRTUAL_TERMINAL_PROCESSING
            | DISABLE_NEWLINE_AUTO_RETURN,
    }
}

/// The visible window, not the scrollback buffer.
pub fn window_size() -> Option<(u16, u16)> {
    let mut info: ScreenBufferInfo = ScreenBufferInfo::default();
    check(unsafe { GetConsoleScreenBufferInfo(stdout(), &raw mut info) }).ok()?;
    let cols: u16 = u16::try_from(i32::from(info.window.right) - i32::from(info.window.left) + 1).ok()?;
    let rows: u16 = u16::try_from(i32::from(info.window.bottom) - i32::from(info.window.top) + 1).ok()?;
    (cols > 0 && rows > 0).then_some((cols, rows))
}

/// Ok(false) on timeout. The handle is also signalled by events that are not keys (focus, resize), so a wait can
/// end with nothing to read; `read_stdin` then reports `Interrupted`.
pub fn wait_readable(timeout_ms: Option<u32>) -> io::Result<bool> {
    match unsafe { WaitForSingleObject(stdin(), timeout_ms.unwrap_or(INFINITE)) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(io::Error::last_os_error()),
    }
}

/// A high surrogate whose low half has not been read yet (0: none).
static PENDING_SURROGATE: AtomicU32 = AtomicU32::new(0);

/// Reads the queued console events without blocking; `Interrupted` when none of them was typed text.
pub fn read_stdin(buf: &mut [u8]) -> io::Result<usize> {
    let mut queued: u32 = 0;
    check(unsafe { GetNumberOfConsoleInputEvents(stdin(), &raw mut queued) })?;
    // Each UTF-16 unit becomes at most 3 UTF-8 bytes (a surrogate pair, 2 units, becomes 4).
    let room: usize = buf.len() / 3;
    let mut records: Vec<InputRecord> = vec![InputRecord::default(); usize::try_from(queued).unwrap_or(0).min(room)];
    if records.is_empty() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let mut read: u32 = 0;
    let len: u32 = u32::try_from(records.len()).unwrap_or(u32::MAX);
    check(unsafe { ReadConsoleInputW(stdin(), records.as_mut_ptr(), len, &raw mut read) })?;
    records.truncate(usize::try_from(read).unwrap_or(0));

    let mut units: Vec<u16> = Vec::with_capacity(records.len() + 1);
    if let Ok(high) = u16::try_from(PENDING_SURROGATE.swap(0, Ordering::Relaxed))
        && high != 0
    {
        units.push(high);
    }
    for record in &records {
        match record.kind {
            KEY_EVENT if record.event.key_down != 0 && record.event.unicode != 0 => units.push(record.event.unicode),
            WINDOW_BUFFER_SIZE_EVENT => RESIZED.store(true, Ordering::Relaxed),
            _ => {}
        }
    }
    if let Some(&last) = units.last()
        && (0xD800..0xDC00).contains(&last)
    {
        units.pop();
        PENDING_SURROGATE.store(u32::from(last), Ordering::Relaxed);
    }
    let text: String = char::decode_utf16(units).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect();
    if text.is_empty() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    let n: usize = text.len().min(buf.len());
    buf[..n].copy_from_slice(&text.as_bytes()[..n]);
    Ok(n)
}

static RESIZED: AtomicBool = AtomicBool::new(false);

/// Resizes arrive as `WINDOW_BUFFER_SIZE_EVENT` records (`ENABLE_WINDOW_INPUT`), so there is nothing to install.
pub fn watch_resize() {}

pub fn take_resized() -> bool {
    RESIZED.swap(false, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_match_the_windows_abi() {
        assert_eq!(size_of::<InputRecord>(), 20);
        assert_eq!(size_of::<ScreenBufferInfo>(), 22);
    }

    #[test]
    fn raw_mode_turns_on_vt_and_off_line_editing() {
        let mode: Mode =
            raw_mode(Mode { input: ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT, output: 0 });
        assert_eq!(mode.input, ENABLE_WINDOW_INPUT | ENABLE_VIRTUAL_TERMINAL_INPUT);
        assert_ne!(mode.output & ENABLE_VIRTUAL_TERMINAL_PROCESSING, 0);
    }
}
