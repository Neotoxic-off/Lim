//! Raw libc bindings for termios, window size, select, read and SIGWINCH.
//! Layouts are the 64-bit little-endian Linux (glibc/musl) and macOS ABIs.

use std::io;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("lim supports Linux and macOS");

#[cfg(target_os = "linux")]
mod os {
    pub type Flag = u32;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub iflag: Flag,
        pub oflag: Flag,
        pub cflag: Flag,
        pub lflag: Flag,
        line: u8,
        pub cc: [u8; 32],
        ispeed: u32,
        ospeed: u32,
    }

    pub const BRKINT: Flag = 0o2;
    pub const INPCK: Flag = 0o20;
    pub const ISTRIP: Flag = 0o40;
    pub const ICRNL: Flag = 0o400;
    pub const IXON: Flag = 0o2000;
    pub const OPOST: Flag = 0o1;
    pub const CS8: Flag = 0o60;
    pub const ISIG: Flag = 0o1;
    pub const ICANON: Flag = 0o2;
    pub const ECHO: Flag = 0o10;
    pub const IEXTEN: Flag = 0o100000;
    pub const VTIME: usize = 5;
    pub const VMIN: usize = 6;
    pub const TIOCGWINSZ: u64 = 0x5413;
}

#[cfg(target_os = "macos")]
mod os {
    pub type Flag = u64;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub iflag: Flag,
        pub oflag: Flag,
        pub cflag: Flag,
        pub lflag: Flag,
        pub cc: [u8; 20],
        ispeed: u64,
        ospeed: u64,
    }

    pub const BRKINT: Flag = 0x2;
    pub const INPCK: Flag = 0x10;
    pub const ISTRIP: Flag = 0x20;
    pub const ICRNL: Flag = 0x100;
    pub const IXON: Flag = 0x200;
    pub const OPOST: Flag = 0x1;
    pub const CS8: Flag = 0x300;
    pub const ISIG: Flag = 0x80;
    pub const ICANON: Flag = 0x100;
    pub const ECHO: Flag = 0x8;
    pub const IEXTEN: Flag = 0x400;
    pub const VMIN: usize = 16;
    pub const VTIME: usize = 17;
    pub const TIOCGWINSZ: u64 = 0x4008_7468;
}

pub use os::Termios;
use os::*;

const STDIN: i32 = 0;
const STDOUT: i32 = 1;
const TCSAFLUSH: i32 = 2;
const SIGWINCH: i32 = 28;

#[repr(C)]
#[derive(Default)]
struct WinSize {
    rows: u16,
    cols: u16,
    xpixel: u16,
    ypixel: u16,
}

/// 1024-bit `fd_set`; fd 0 is bit 0 of the first word on both ABIs.
#[repr(C)]
struct FdSet([u64; 16]);

/// macOS has a 32-bit `tv_usec` plus padding; little-endian puts the value in the right half.
#[repr(C)]
struct TimeVal {
    sec: i64,
    usec: i64,
}

unsafe extern "C" {
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, action: i32, termios: *const Termios) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    // `poll` cannot wait on ttys on macOS, `select` can everywhere.
    fn select(nfds: i32, read: *mut FdSet, write: *mut FdSet, except: *mut FdSet, timeout: *mut TimeVal) -> i32;
}

pub fn get_mode() -> io::Result<Termios> {
    let mut t = MaybeUninit::<Termios>::uninit();
    if unsafe { tcgetattr(STDIN, t.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { t.assume_init() })
}

pub fn set_mode(t: &Termios) -> io::Result<()> {
    if unsafe { tcsetattr(STDIN, TCSAFLUSH, t) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn raw_mode(mut t: Termios) -> Termios {
    t.iflag &= !(BRKINT | INPCK | ISTRIP | ICRNL | IXON);
    t.oflag &= !OPOST;
    t.cflag |= CS8;
    t.lflag &= !(ISIG | ICANON | ECHO | IEXTEN);
    t.cc[VMIN] = 1;
    t.cc[VTIME] = 0;
    t
}

pub fn window_size() -> Option<(u16, u16)> {
    let mut ws = WinSize::default();
    let ok = unsafe { ioctl(STDOUT, TIOCGWINSZ, &mut ws as *mut WinSize) } == 0;
    (ok && ws.cols > 0).then_some((ws.cols, ws.rows))
}

/// Ok(false) on timeout or when a signal (resize) interrupted the wait.
pub fn wait_readable(timeout_ms: Option<u32>) -> io::Result<bool> {
    let mut set = FdSet([0; 16]);
    set.0[0] = 1 << STDIN;
    let mut tv = timeout_ms.map(|ms| TimeVal { sec: i64::from(ms / 1000), usec: i64::from(ms % 1000) * 1000 });
    let tv_ptr = tv.as_mut().map_or(ptr::null_mut(), |t| t as *mut TimeVal);
    match unsafe { select(STDIN + 1, &mut set, ptr::null_mut(), ptr::null_mut(), tv_ptr) } {
        n if n > 0 => Ok(true),
        0 => Ok(false),
        _ => interrupted_is(false),
    }
}

pub fn read_stdin(buf: &mut [u8]) -> io::Result<usize> {
    match unsafe { read(STDIN, buf.as_mut_ptr(), buf.len()) } {
        n if n >= 0 => Ok(n as usize),
        _ => Err(io::Error::last_os_error()),
    }
}

fn interrupted_is<T>(value: T) -> io::Result<T> {
    let e = io::Error::last_os_error();
    if e.kind() == io::ErrorKind::Interrupted { Ok(value) } else { Err(e) }
}

static RESIZED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_winch(_: i32) {
    RESIZED.store(true, Ordering::Relaxed);
}

pub fn watch_resize() {
    unsafe { signal(SIGWINCH, on_winch) };
}

pub fn take_resized() -> bool {
    RESIZED.swap(false, Ordering::Relaxed)
}
