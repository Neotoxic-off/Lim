//! lim-gui: lim in its own window, without a terminal's tabs, padding or font settings.
//!
//! A small terminal emulator that runs the `lim` installed next to it: `alacritty_terminal` parses its output from a
//! pty (`ConPTY` on Windows), `softbuffer` shows the cells drawn with an embedded Nerd Font. The shell, `:!` and the
//! `<Leader>t…` tools run in the same pty, so they work as in a terminal.

#![cfg_attr(windows, windows_subsystem = "windows")]
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

use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::{env, fs};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event as TermEvent, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop as PtyLoop, EventLoopSender, Msg};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::tty::{self, Options as PtyOptions, Shell};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use fontdue::{Font, FontSettings, Metrics};
use softbuffer::{Buffer, Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// `JetBrains Mono NL` with the Nerd Font icons (SIL OFL 1.1, the license is in the font's metadata).
const FONT: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMonoNLNerdFont-Regular.ttf");
/// In logical pixels; the window's scale factor multiplies it.
const FONT_SIZE: f32 = 15.0;
/// What shows before lim paints its colorscheme (astrodark's).
const FOREGROUND: u32 = 0x00ad_b0bb;
const BACKGROUND: u32 = 0x001a_1d23;

fn main() -> Result<(), String> {
    let event_loop: EventLoop<TermEvent> = EventLoop::with_user_event().build().map_err(|e| e.to_string())?;
    let main: Font = Font::from_bytes(FONT, FontSettings::default())?;
    let mut app: App = App {
        proxy: Proxy(event_loop.create_proxy()),
        fonts: Some((main, fallback_fonts())),
        view: None,
        modifiers: ModifiersState::empty(),
        mouse: (0, 0),
        left_down: false,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

/// Windows' symbol and emoji fonts, for the chars the Nerd Font lacks (drawn in one color).
/// shortcut: no fallback on Linux or macOS, add one (fontconfig paths) when lim-gui is used there.
fn fallback_fonts() -> Vec<Font> {
    let Some(dir): Option<PathBuf> = env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts")) else {
        return Vec::new();
    };
    ["seguisym.ttf", "seguiemj.ttf"]
        .iter()
        .filter_map(|name| Font::from_bytes(fs::read(dir.join(name)).ok()?, FontSettings::default()).ok())
        .collect()
}

/// Sends the terminal's events to the window's event loop.
#[derive(Clone)]
struct Proxy(EventLoopProxy<TermEvent>);

impl EventListener for Proxy {
    fn send_event(&self, event: TermEvent) {
        // Fails only once the window is closed, when nothing is left to update.
        drop(self.0.send_event(event));
    }
}

struct App {
    proxy: Proxy,
    /// Taken when the window opens: the embedded font and the fallbacks.
    fonts: Option<(Font, Vec<Font>)>,
    view: Option<View>,
    modifiers: ModifiersState,
    /// The cell under the mouse (column, line).
    mouse: (usize, usize),
    left_down: bool,
}

struct View {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    term: Arc<FairMutex<Term<Proxy>>>,
    pty: EventLoopSender,
    glyphs: Glyphs,
    size: WindowSize,
}

impl View {
    fn send(&self, bytes: Vec<u8>) {
        // Fails only when lim has exited, and then the window closes.
        drop(self.pty.send(Msg::Input(Cow::Owned(bytes))));
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = self.glyphs.grid(size);
        self.term.lock().resize(TermSize::new(self.size.num_cols.into(), self.size.num_lines.into()));
        drop(self.pty.send(Msg::Resize(self.size)));
    }

    fn draw(&mut self) {
        let PhysicalSize { width, height }: PhysicalSize<u32> = self.window.inner_size();
        let (Some(w), Some(h)): (Option<NonZeroU32>, Option<NonZeroU32>) =
            (NonZeroU32::new(width), NonZeroU32::new(height))
        else {
            return;
        };
        if self.surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer): Result<Buffer<'_, Rc<Window>, Rc<Window>>, _> = self.surface.buffer_mut() else { return };
        let Ok(width): Result<usize, _> = usize::try_from(width) else { return };
        let mut canvas: Canvas<'_> = Canvas { pixels: &mut buffer, width };
        let term = self.term.lock();
        let content = term.renderable_content();
        let colors: &Colors = content.colors;
        let cursor: (usize, usize) =
            (content.cursor.point.column.0, usize::try_from(content.cursor.point.line.0).unwrap_or(0));
        let shape: CursorShape = content.cursor.shape;
        canvas.pixels.fill(rgb(Color::Named(NamedColor::Background), colors));
        let (cw, ch, ascent): (usize, usize, usize) = (self.glyphs.cell_w, self.glyphs.cell_h, self.glyphs.ascent);
        // Backgrounds first, then glyphs: a wide icon may reach into the next cell.
        let mut texts: Vec<(usize, usize, char, u32, Flags)> = Vec::new();
        for indexed in content.display_iter {
            let cell = indexed.cell;
            // The history is never scrolled into view (lim draws on the alternate screen), so lines are >= 0.
            let Ok(line): Result<usize, _> = usize::try_from(indexed.point.line.0) else { continue };
            let col: usize = indexed.point.column.0;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg): (u32, u32) = (rgb(cell.fg, colors), rgb(cell.bg, colors));
            if cell.flags.contains(Flags::INVERSE) != (shape == CursorShape::Block && (col, line) == cursor) {
                (fg, bg) = (bg, fg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = blend(fg, bg, 96);
            }
            let cells: usize = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            canvas.fill(col * cw, line * ch, cells * cw, ch, bg);
            texts.push((col * cw, line * ch, cell.c, fg, cell.flags));
        }
        for (x, y, c, fg, flags) in texts {
            let cells: usize = if flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            if c != ' ' && !flags.contains(Flags::HIDDEN) {
                let (metrics, bitmap): &(Metrics, Vec<u8>) = self.glyphs.get(c);
                canvas.glyph(x, y + ascent, metrics, bitmap, fg);
            }
            if flags.intersects(Flags::ALL_UNDERLINES) {
                canvas.fill(x, y + ascent + 1, cells * cw, 1, fg);
            }
            if flags.contains(Flags::STRIKEOUT) {
                canvas.fill(x, y + ch / 2, cells * cw, 1, fg);
            }
        }
        let color: u32 = rgb(Color::Named(NamedColor::Cursor), colors);
        let (x, y): (usize, usize) = (cursor.0 * cw, cursor.1 * ch);
        match shape {
            CursorShape::Beam => canvas.fill(x, y, 2, ch, color),
            CursorShape::Underline => canvas.fill(x, y + ch - 2, cw, 2, color),
            CursorShape::HollowBlock => {
                canvas.fill(x, y, cw, 1, color);
                canvas.fill(x, y + ch - 1, cw, 1, color);
                canvas.fill(x, y, 1, ch, color);
                canvas.fill(x + cw - 1, y, 1, ch, color);
            }
            CursorShape::Block | CursorShape::Hidden => {}
        }
        drop(term);
        drop(buffer.present());
    }
}

/// The window's pixels, `0x00RRGGBB`, row after row.
struct Canvas<'a> {
    pixels: &'a mut [u32],
    width: usize,
}

impl Canvas<'_> {
    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let right: usize = (x + w).min(self.width);
        for row in self.pixels.chunks_exact_mut(self.width).skip(y).take(h) {
            if let Some(span) = row.get_mut(x.min(right)..right) {
                span.fill(color);
            }
        }
    }

    /// Blends a glyph's coverage into the pixels, its baseline at `baseline`.
    fn glyph(&mut self, x: usize, baseline: usize, metrics: &Metrics, bitmap: &[u8], color: u32) {
        let Ok(left): Result<i64, _> = i64::try_from(x).map(|x| x + i64::from(metrics.xmin)) else { return };
        let Ok(height): Result<i64, _> = i64::try_from(metrics.height) else { return };
        let Ok(top): Result<i64, _> = i64::try_from(baseline).map(|b| b - height - i64::from(metrics.ymin)) else {
            return;
        };
        for (i, &alpha) in bitmap.iter().enumerate().filter(|(_, a)| **a > 0) {
            let (gx, gy): (usize, usize) = (i % metrics.width.max(1), i / metrics.width.max(1));
            let (Ok(px), Ok(py)): (Result<usize, _>, Result<usize, _>) = (
                usize::try_from(left + i64::try_from(gx).unwrap_or(0)),
                usize::try_from(top + i64::try_from(gy).unwrap_or(0)),
            ) else {
                continue;
            };
            if px < self.width
                && let Some(pixel) = self.pixels.get_mut(py * self.width + px)
            {
                *pixel = blend(color, *pixel, alpha);
            }
        }
    }
}

/// The font at the window's scale, its cell size and the glyphs drawn so far.
struct Glyphs {
    main: Font,
    fallback: Vec<Font>,
    px: f32,
    cell_w: usize,
    cell_h: usize,
    ascent: usize,
    cache: HashMap<char, (Metrics, Vec<u8>)>,
}

impl Glyphs {
    fn new(main: Font, fallback: Vec<Font>, scale: f64) -> Self {
        let mut glyphs: Self = Self { main, fallback, px: 0.0, cell_w: 1, cell_h: 1, ascent: 1, cache: HashMap::new() };
        glyphs.scale(scale);
        glyphs
    }

    #[expect(clippy::cast_possible_truncation, reason = "a scale factor and font metrics are small positive numbers")]
    #[expect(clippy::cast_sign_loss, reason = "clamped to at least 1 first")]
    fn scale(&mut self, scale: f64) {
        self.px = FONT_SIZE * scale as f32;
        let (ascent, descent, gap): (f32, f32, f32) = self
            .main
            .horizontal_line_metrics(self.px)
            .map_or((self.px, 0.0, 0.0), |m| (m.ascent, m.descent, m.line_gap));
        self.cell_w = self.main.metrics('M', self.px).advance_width.round().max(1.0) as usize;
        self.cell_h = (ascent - descent + gap).ceil().max(1.0) as usize;
        self.ascent = ascent.round().max(1.0) as usize;
        self.cache.clear();
    }

    fn get(&mut self, c: char) -> &(Metrics, Vec<u8>) {
        let (main, fallback, px): (&Font, &[Font], f32) = (&self.main, &self.fallback, self.px);
        self.cache.entry(c).or_insert_with(|| {
            let font: &Font = if main.lookup_glyph_index(c) != 0 {
                main
            } else {
                fallback.iter().find(|f| f.lookup_glyph_index(c) != 0).unwrap_or(main)
            };
            font.rasterize(c, px)
        })
    }

    /// How many cells fit in the window.
    fn grid(&self, size: PhysicalSize<u32>) -> WindowSize {
        let fit = |pixels: u32, cell: usize| {
            u16::try_from(usize::try_from(pixels).unwrap_or(0) / cell).unwrap_or(u16::MAX).max(1)
        };
        WindowSize {
            num_cols: fit(size.width, self.cell_w),
            num_lines: fit(size.height, self.cell_h),
            cell_width: u16::try_from(self.cell_w).unwrap_or(u16::MAX),
            cell_height: u16::try_from(self.cell_h).unwrap_or(u16::MAX),
        }
    }
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<View, String> {
        let attributes =
            Window::default_attributes().with_title("lim").with_inner_size(LogicalSize::new(1100.0, 720.0));
        let window: Rc<Window> = Rc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        let context: Context<Rc<Window>> = Context::new(Rc::clone(&window)).map_err(|e| e.to_string())?;
        let surface: Surface<Rc<Window>, Rc<Window>> =
            Surface::new(&context, Rc::clone(&window)).map_err(|e| e.to_string())?;
        let (main, fallback): (Font, Vec<Font>) = self.fonts.take().ok_or("the window is already open")?;
        let glyphs: Glyphs = Glyphs::new(main, fallback, window.scale_factor());
        let size: WindowSize = glyphs.grid(window.inner_size());
        let config: Config = Config { scrolling_history: 0, ..Config::default() };
        let dimensions: TermSize = TermSize::new(size.num_cols.into(), size.num_lines.into());
        let term: Arc<FairMutex<Term<Proxy>>> =
            Arc::new(FairMutex::new(Term::new(config, &dimensions, self.proxy.clone())));
        let options: PtyOptions = PtyOptions {
            shell: Some(Shell::new(lim_path(), env::args().skip(1).collect())),
            env: HashMap::from([
                ("COLORTERM".into(), "truecolor".into()),
                ("TERM".into(), "xterm-256color".into()),
                // lim keeps the Nerd Font icons: this window draws them even when no Nerd Font is installed.
                ("LIM_GUI".into(), "1".into()),
            ]),
            ..PtyOptions::default()
        };
        let pty = tty::new(&options, size, 0).map_err(|e| format!("cannot start lim: {e}"))?;
        let pty_loop =
            PtyLoop::new(Arc::clone(&term), self.proxy.clone(), pty, false, false).map_err(|e| e.to_string())?;
        let sender: EventLoopSender = pty_loop.channel();
        drop(pty_loop.spawn());
        Ok(View { window, surface, term, pty: sender, glyphs, size })
    }
}

/// An SGR mouse report for the cell `at`, when lim asked for them.
fn report(view: &View, at: (usize, usize), button: u8, press: bool) {
    let mode: TermMode = *view.term.lock().mode();
    if mode.contains(TermMode::SGR_MOUSE) && mode.intersects(TermMode::MOUSE_MODE) {
        let end: char = if press { 'M' } else { 'm' };
        view.send(format!("\x1b[<{button};{};{}{end}", at.0 + 1, at.1 + 1).into_bytes());
    }
}

/// The lim next to this program (`cargo install` puts both in the same folder), else the one on the `PATH`.
fn lim_path() -> String {
    let name: &str = if cfg!(windows) { "lim.exe" } else { "lim" };
    env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(name))
        .filter(|path| path.is_file())
        .map_or_else(|| name.to_owned(), |path| path.to_string_lossy().into_owned())
}

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.view.is_some() {
            return;
        }
        match self.open(event_loop) {
            Ok(view) => self.view = Some(view),
            Err(e) => {
                eprintln!("lim-gui: {e}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        let Some(view): Option<&View> = self.view.as_ref() else { return };
        match event {
            TermEvent::Wakeup => view.window.request_redraw(),
            TermEvent::PtyWrite(text) => view.send(text.into_bytes()),
            TermEvent::ColorRequest(index, reply) => {
                let colors: Colors = *view.term.lock().colors();
                let color: u32 = colors[index].map_or_else(
                    || match u8::try_from(index) {
                        Ok(i) => xterm(i),
                        Err(_) if index == NamedColor::Background as usize => BACKGROUND,
                        Err(_) => FOREGROUND,
                    },
                    pack,
                );
                let [_, r, g, b]: [u8; 4] = color.to_be_bytes();
                view.send(reply(Rgb { r, g, b }).into_bytes());
            }
            TermEvent::TextAreaSizeRequest(reply) => view.send(reply(view.size).into_bytes()),
            TermEvent::ChildExit(_) | TermEvent::Exit => event_loop.exit(),
            _ => {}
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(view): Option<&mut View> = self.view.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => {
                drop(view.pty.send(Msg::Shutdown));
                event_loop.exit();
            }
            WindowEvent::Resized(size) => view.resize(size),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                view.glyphs.scale(scale_factor);
                let size: PhysicalSize<u32> = view.window.inner_size();
                view.resize(size);
            }
            WindowEvent::RedrawRequested => view.draw(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let app_cursor: bool = view.term.lock().mode().contains(TermMode::APP_CURSOR);
                if let Some(bytes) = encode(&event.logical_key, event.text.as_deref(), self.modifiers, app_cursor) {
                    view.send(bytes);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let at: (usize, usize) =
                    (cell_at(position.x, view.glyphs.cell_w), cell_at(position.y, view.glyphs.cell_h));
                if at != self.mouse {
                    self.mouse = at;
                    if self.left_down {
                        report(view, self.mouse, 32, true);
                    }
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.left_down = state == ElementState::Pressed;
                report(view, self.mouse, 0, self.left_down);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let up: bool = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.0,
                };
                report(view, self.mouse, if up { 64 } else { 65 }, true);
            }
            _ => {}
        }
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0 and up; a window is small")]
fn cell_at(pixels: f64, cell: usize) -> usize {
    pixels.max(0.0) as usize / cell.max(1)
}

/// The bytes a terminal sends for a key press: xterm's sequences, which lim reads on every platform.
fn encode(key: &Key, text: Option<&str>, mods: ModifiersState, app_cursor: bool) -> Option<Vec<u8>> {
    let (shift, ctrl, alt): (bool, bool, bool) = (mods.shift_key(), mods.control_key(), mods.alt_key());
    // xterm's modifier parameter: 1 + shift + 2·alt + 4·ctrl.
    let m: u8 = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let csi = |end: char| match (m, app_cursor) {
        (1, true) => format!("\x1bO{end}"),
        (1, false) => format!("\x1b[{end}"),
        _ => format!("\x1b[1;{m}{end}"),
    };
    let ss3 = |end: char| if m == 1 { format!("\x1bO{end}") } else { format!("\x1b[1;{m}{end}") };
    let tilde = |n: u8| if m == 1 { format!("\x1b[{n}~") } else { format!("\x1b[{n};{m}~") };
    let text: String = match key {
        Key::Named(named) => match named {
            NamedKey::ArrowUp => return Some(csi('A').into_bytes()),
            NamedKey::ArrowDown => return Some(csi('B').into_bytes()),
            NamedKey::ArrowRight => return Some(csi('C').into_bytes()),
            NamedKey::ArrowLeft => return Some(csi('D').into_bytes()),
            NamedKey::Home => return Some(csi('H').into_bytes()),
            NamedKey::End => return Some(csi('F').into_bytes()),
            NamedKey::Insert => return Some(tilde(2).into_bytes()),
            NamedKey::Delete => return Some(tilde(3).into_bytes()),
            NamedKey::PageUp => return Some(tilde(5).into_bytes()),
            NamedKey::PageDown => return Some(tilde(6).into_bytes()),
            NamedKey::F1 => return Some(ss3('P').into_bytes()),
            NamedKey::F2 => return Some(ss3('Q').into_bytes()),
            NamedKey::F3 => return Some(ss3('R').into_bytes()),
            NamedKey::F4 => return Some(ss3('S').into_bytes()),
            NamedKey::F5 => return Some(tilde(15).into_bytes()),
            NamedKey::F6 => return Some(tilde(17).into_bytes()),
            NamedKey::F7 => return Some(tilde(18).into_bytes()),
            NamedKey::F8 => return Some(tilde(19).into_bytes()),
            NamedKey::F9 => return Some(tilde(20).into_bytes()),
            NamedKey::F10 => return Some(tilde(21).into_bytes()),
            NamedKey::F11 => return Some(tilde(23).into_bytes()),
            NamedKey::F12 => return Some(tilde(24).into_bytes()),
            NamedKey::Tab if shift => return Some(b"\x1b[Z".to_vec()),
            NamedKey::Tab => "\t".into(),
            NamedKey::Enter => "\r".into(),
            NamedKey::Backspace if ctrl => "\x08".into(),
            NamedKey::Backspace => "\x7f".into(),
            NamedKey::Escape => "\x1b".into(),
            NamedKey::Space if ctrl => "\0".into(),
            NamedKey::Space => " ".into(),
            _ => return None,
        },
        // AltGr is Ctrl+Alt on Windows: its char (`@` on AZERTY) comes as text.
        Key::Character(_) if ctrl && alt && text.is_some_and(|t| !t.chars().any(char::is_control)) => {
            return text.map(|t| t.as_bytes().to_vec());
        }
        Key::Character(s) if ctrl => {
            let c: char = s.chars().next()?.to_ascii_lowercase();
            let code: u8 = match c {
                'a'..='z' => u8::try_from(c).ok()? & 0x1f,
                '@' | '2' | ' ' => 0,
                '[' | '3' => 0x1b,
                '\\' | '4' => 0x1c,
                ']' | '5' => 0x1d,
                '^' | '6' => 0x1e,
                '_' | '-' | '7' => 0x1f,
                _ => return Some(s.as_bytes().to_vec()),
            };
            char::from(code).to_string()
        }
        Key::Character(s) => text.map_or_else(|| s.to_string(), ToString::to_string),
        _ => return None,
    };
    Some(if alt { format!("\x1b{text}") } else { text }.into_bytes())
}

/// A cell color: lim's 24-bit colors as they are, the rest from the terminal's palette or xterm's defaults.
fn rgb(color: Color, colors: &Colors) -> u32 {
    match color {
        Color::Spec(c) => pack(c),
        Color::Indexed(i) => colors[usize::from(i)].map_or_else(|| xterm(i), pack),
        Color::Named(named) => colors[named].map_or_else(
            || match named {
                NamedColor::Background => BACKGROUND,
                NamedColor::Cursor | NamedColor::Foreground => FOREGROUND,
                // The 16 named colors are the first palette entries, a fieldless enum's discriminant.
                other => u8::try_from(other as usize).ok().filter(|i| *i < 16).map_or(FOREGROUND, xterm),
            },
            pack,
        ),
    }
}

fn pack(c: Rgb) -> u32 {
    u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b)
}

/// xterm's 256 colors: 16 ANSI, a 6×6×6 cube, 24 grays.
fn xterm(i: u8) -> u32 {
    const ANSI: [u32; 16] = [
        0x0000_0000,
        0x00cd_0000,
        0x0000_cd00,
        0x00cd_cd00,
        0x0000_00ee,
        0x00cd_00cd,
        0x0000_cdcd,
        0x00e5_e5e5,
        0x007f_7f7f,
        0x00ff_0000,
        0x0000_ff00,
        0x00ff_ff00,
        0x005c_5cff,
        0x00ff_00ff,
        0x0000_ffff,
        0x00ff_ffff,
    ];
    let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
    match i {
        0..=15 => ANSI[usize::from(i)],
        16..=231 => {
            let n: u8 = i - 16;
            pack(Rgb { r: level(n / 36), g: level(n / 6 % 6), b: level(n % 6) })
        }
        _ => {
            let gray: u8 = 8 + 10 * (i - 232);
            pack(Rgb { r: gray, g: gray, b: gray })
        }
    }
}

/// `over` drawn on `under` with coverage `alpha` (0–255).
fn blend(over: u32, under: u32, alpha: u8) -> u32 {
    let a: u32 = u32::from(alpha);
    [16, 8, 0].iter().fold(0, |out, shift| {
        let (o, u): (u32, u32) = ((over >> shift) & 0xff, (under >> shift) & 0xff);
        out | ((o * a + u * (255 - a)) / 255) << shift
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_follow_xterm() {
        assert_eq!(xterm(1), 0x00cd_0000);
        assert_eq!(xterm(16), 0);
        assert_eq!(xterm(231), 0x00ff_ffff);
        assert_eq!(xterm(232), 0x0008_0808);
        assert_eq!(xterm(255), 0x00ee_eeee);
        assert_eq!(blend(0x00ff_ffff, 0, 255), 0x00ff_ffff);
        assert_eq!(blend(0x00ff_ffff, 0, 0), 0);
    }

    #[test]
    fn keys_encode_as_xterm() {
        let none: ModifiersState = ModifiersState::empty();
        let char = |s: &str| Key::Character(s.into());
        let enc = |key: &Key, text: Option<&str>, mods: ModifiersState| encode(key, text, mods, false).unwrap();
        assert_eq!(enc(&char("é"), Some("é"), none), "é".as_bytes());
        assert_eq!(enc(&char("w"), Some("w"), ModifiersState::CONTROL), b"\x17");
        assert_eq!(enc(&char("x"), Some("x"), ModifiersState::ALT), b"\x1bx");
        // AltGr+0 on AZERTY arrives as Ctrl+Alt with `@` as text.
        assert_eq!(enc(&char("à"), Some("@"), ModifiersState::CONTROL | ModifiersState::ALT), b"@");
        assert_eq!(enc(&Key::Named(NamedKey::ArrowUp), None, none), b"\x1b[A");
        assert_eq!(enc(&Key::Named(NamedKey::ArrowUp), None, ModifiersState::CONTROL), b"\x1b[1;5A");
        assert_eq!(encode(&Key::Named(NamedKey::ArrowUp), None, none, true).unwrap(), b"\x1bOA");
        assert_eq!(enc(&Key::Named(NamedKey::Tab), None, ModifiersState::SHIFT), b"\x1b[Z");
        assert_eq!(enc(&Key::Named(NamedKey::F5), None, none), b"\x1b[15~");
        assert_eq!(enc(&Key::Named(NamedKey::Escape), None, none), b"\x1b");
        assert_eq!(encode(&Key::Named(NamedKey::Shift), None, ModifiersState::SHIFT, false), None);
    }

    #[test]
    fn the_embedded_font_has_the_nerd_icons() {
        let font: Font = Font::from_bytes(FONT, FontSettings::default()).unwrap();
        assert_ne!(font.lookup_glyph_index('\u{e7a8}'), 0); // Rust devicon
        assert_ne!(font.lookup_glyph_index('a'), 0);
    }
}
