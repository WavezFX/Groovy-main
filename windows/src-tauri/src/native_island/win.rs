// The native island's window on Windows (prototype, `--native-island`).
//
// One top-most, non-activating layered popup at the top of the screen, fed
// premultiplied pixels with UpdateLayeredWindow. Pixels with alpha 0 are
// click-through by themselves; the folded pill paints an alpha-1 band around
// it, so the hover area is a little bigger than what shows.
//
// Frames: a 16 ms timer runs only while the pill opens, folds or blinks; a
// blink timer only while the cursor is on it; one timer a minute for the
// clock. Folded and idle, nothing else runs. Lives on the main thread (the
// app's event loop pumps its messages), and needs no Tauri type, so it can be
// type-checked on its own.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, GetDC, GetDeviceCaps, GetTextExtentPoint32W, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, TextOutW, AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DIB_RGB_COLORS, HBITMAP, HDC, HFONT, HGDIOBJ, LOGPIXELSX, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, KillTimer, RegisterClassW, SetTimer, SetWindowPos, ShowWindow, UpdateLayeredWindow,
    HWND_TOPMOST, MA_NOACTIVATE, SM_CXSCREEN, SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_LBUTTONUP, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::draw::{self, Anim, Canvas, Mask, Palette, Scene};

/// What the user did to the pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Click,
    RightClick,
}

pub struct Config {
    pub look: String,
    pub name: String,
    /// Where along the top, 0 left … 1 right (the island's own setting).
    pub fraction: f64,
    /// Local hour, minute and second.
    pub time: fn() -> (u32, u32, u32),
    pub on_input: Box<dyn Fn(Input)>,
}

const WM_MOUSELEAVE: u32 = 0x02A3;
const FRAME: usize = 1;
const BLINK: usize = 2;
const CLOCK: usize = 3;
const NOTICE: usize = 4;
const BLINK_EVERY_MS: u32 = 3400;

struct Gdi {
    dc: HDC,
    bitmap: HBITMAP,
    bits: *mut u8,
    text_dc: HDC,
    text_bitmap: HBITMAP,
    text_bits: *mut u8,
    text_size: (u32, u32),
    fonts: [HFONT; 2],
}

struct State {
    hwnd: HWND,
    gdi: Gdi,
    canvas: Canvas,
    scale: f32,
    pos: (i32, i32),
    palette: Palette,
    name: String,
    time: fn() -> (u32, u32, u32),
    on_input: Rc<dyn Fn(Input)>,
    anim: Anim,
    origin: Instant,
    hovered: bool,
    shown: bool,
    frame_timer: bool,
    /// The last text drawn into the mask, and which font.
    text: Option<(String, usize, Mask)>,
    /// A notice on the pill: its text and tint, until its timer ends.
    notice: Option<(String, draw::Rgba)>,
    /// Frames drawn since start (logged, and read by tests on the PC).
    frames: u64,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn dib(w: u32, h: u32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

unsafe fn section(w: u32, h: u32) -> Option<(HDC, HBITMAP, *mut u8)> {
    let dc = CreateCompatibleDC(None);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let Ok(bitmap) = CreateDIBSection(Some(dc), &dib(w, h), DIB_RGB_COLORS, &mut bits, None, 0) else {
        let _ = DeleteDC(dc);
        return None;
    };
    if bits.is_null() {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        return None;
    }
    SelectObject(dc, HGDIOBJ(bitmap.0));
    Some((dc, bitmap, bits as *mut u8))
}

unsafe fn font(px: f32) -> HFONT {
    CreateFontW(
        -(px.round() as i32),
        0,
        0,
        0,
        600,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        ANTIALIASED_QUALITY,
        0,
        w!("Segoe UI Variable Text"),
    )
}

/// Makes the window, hidden. Main thread only. False when Windows said no.
pub fn create(cfg: Config) -> bool {
    if STATE.with(|s| s.borrow().is_some()) {
        return true;
    }
    unsafe {
        static CLASS: std::sync::Once = std::sync::Once::new();
        CLASS.call_once(|| {
            let class = WNDCLASSW { lpfnWndProc: Some(proc), lpszClassName: w!("GroovyNativeIsland"), ..Default::default() };
            RegisterClassW(&class);
        });
        let screen = GetDC(None);
        let dpi = GetDeviceCaps(Some(screen), LOGPIXELSX);
        ReleaseDC(None, screen);
        let scale = if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 };
        let (w, h) = ((draw::WIN_W * scale).round() as u32, (draw::WIN_H * scale).round() as u32);
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = ((screen_w - w as i32).max(0) as f64 * cfg.fraction.clamp(0.0, 1.0)).round() as i32;
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("GroovyNativeIsland"),
            w!("Groovy"),
            WS_POPUP,
            x,
            0,
            w as i32,
            h as i32,
            None,
            None,
            None,
            None,
        ) else {
            return false;
        };
        let Some((dc, bitmap, bits)) = section(w, h) else {
            let _ = DestroyWindow(hwnd);
            return false;
        };
        let text_size = ((250.0 * scale) as u32, (26.0 * scale) as u32);
        let Some((text_dc, text_bitmap, text_bits)) = section(text_size.0, text_size.1) else {
            let _ = DestroyWindow(hwnd);
            return false;
        };
        let fonts = [font(draw::layout(0.0, scale).font_px), font(draw::layout(1.0, scale).font_px)];
        SetBkMode(text_dc, TRANSPARENT);
        SetTextColor(text_dc, COLORREF(0x00FF_FFFF));
        let state = State {
            hwnd,
            gdi: Gdi { dc, bitmap, bits, text_dc, text_bitmap, text_bits, text_size, fonts },
            canvas: Canvas::new(w, h),
            scale,
            pos: (x, 0),
            palette: draw::palette(&cfg.look),
            name: cfg.name,
            time: cfg.time,
            on_input: Rc::from(cfg.on_input),
            anim: Anim::default(),
            origin: Instant::now(),
            hovered: false,
            shown: false,
            frame_timer: false,
            text: None,
            notice: None,
            frames: 0,
        };
        STATE.with(|s| *s.borrow_mut() = Some(state));
    }
    true
}

/// Shows (true) or hides the pill. Hidden, no timer runs at all.
pub fn show(on: bool) {
    with(|s| unsafe {
        if s.shown == on {
            return;
        }
        s.shown = on;
        if on {
            s.hovered = false;
            s.anim = Anim::default();
            render(s);
            let _ = SetWindowPos(s.hwnd, Some(HWND_TOPMOST), s.pos.0, s.pos.1, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
            let _ = ShowWindow(s.hwnd, SW_SHOWNOACTIVATE);
            arm_clock(s);
        } else {
            let _ = ShowWindow(s.hwnd, SW_HIDE);
            s.notice = None;
            for id in [FRAME, BLINK, CLOCK, NOTICE] {
                let _ = KillTimer(Some(s.hwnd), id);
            }
            s.frame_timer = false;
        }
    });
}

/// He changed his look or his name: one new frame.
pub fn restyle(look: &str, name: &str) {
    with(|s| {
        s.palette = draw::palette(look);
        s.name = name.to_string();
        s.text = None;
        if s.shown {
            unsafe { render(s) };
        }
    });
}

/// A notice on the pill for `seconds`: it opens, shows the text and a dot in
/// the tint, blinks once, then folds back (unless the cursor is on it). A
/// click opens the web island, where the notice and its button wait.
pub fn notice(text: &str, tint: &str, seconds: u64) {
    with(|s| unsafe {
        if !s.shown {
            return;
        }
        s.notice = Some((draw::notice_text(text), draw::hex(tint).unwrap_or(s.palette.eye)));
        let t = now(s);
        s.anim.set_open(t, true);
        s.anim.blink(t + 0.2);
        SetTimer(Some(s.hwnd), NOTICE, (seconds.clamp(3, 60) * 1000) as u32, None);
        start_frames(s);
    });
}

/// How many frames have been drawn since the window was made.
pub fn frames() -> u64 {
    STATE.with(|s| s.borrow().as_ref().map_or(0, |s| s.frames))
}

fn with(f: impl FnOnce(&mut State)) {
    STATE.with(|s| {
        if let Ok(mut b) = s.try_borrow_mut() {
            if let Some(state) = b.as_mut() {
                f(state);
            }
        }
    });
}

fn now(s: &State) -> f64 {
    s.origin.elapsed().as_secs_f64()
}

/// The clock's next redraw, just after the minute turns.
unsafe fn arm_clock(s: &State) {
    let (_, _, sec) = (s.time)();
    let ms = (60 - sec.min(59)) * 1000 + 50;
    SetTimer(Some(s.hwnd), CLOCK, ms, None);
}

unsafe fn start_frames(s: &mut State) {
    if !s.frame_timer {
        s.frame_timer = true;
        SetTimer(Some(s.hwnd), FRAME, 16, None);
    }
}

/// Text into the coverage mask, only when it changed.
unsafe fn text_mask(s: &mut State, text: &str, which: usize) {
    if s.text.as_ref().is_some_and(|(t, f, _)| t == text && *f == which) {
        return;
    }
    let (w, h) = s.gdi.text_size;
    std::ptr::write_bytes(s.gdi.text_bits, 0, (w * h * 4) as usize);
    let wide: Vec<u16> = text.encode_utf16().collect();
    let old = SelectObject(s.gdi.text_dc, HGDIOBJ(s.gdi.fonts[which].0));
    let mut size = SIZE::default();
    let _ = GetTextExtentPoint32W(s.gdi.text_dc, &wide, &mut size);
    let top = ((h as i32 - size.cy) / 2).max(0);
    let _ = TextOutW(s.gdi.text_dc, 0, top, &wide);
    SelectObject(s.gdi.text_dc, old);
    let used = (size.cx.max(0) as u32 + 2).min(w);
    let mut a = vec![0u8; (used * h) as usize];
    for y in 0..h {
        for x in 0..used {
            // GDI draws grey anti-aliased white on black: green is the coverage.
            a[(y * used + x) as usize] = *s.gdi.text_bits.add(((y * w + x) * 4 + 1) as usize);
        }
    }
    s.text = Some((text.to_string(), which, Mask { w: used, h, a }));
}

unsafe fn render(s: &mut State) {
    let t = now(s);
    let open = s.anim.open(t);
    let (hour, minute, _) = (s.time)();
    let wide = s.anim.target() > 0.5;
    let label = match &s.notice {
        Some((text, _)) => text.clone(),
        None => draw::label(hour, minute, &s.name, wide),
    };
    text_mask(s, &label, if open > 0.5 { 1 } else { 0 });
    let mask = s.text.as_ref().map(|(_, _, m)| m);
    let scene = Scene { palette: s.palette, open, blink: s.anim.shut(t), scale: s.scale, text: mask, dot: s.notice.as_ref().map(|n| n.1) };
    draw::draw(&mut s.canvas, &scene);
    std::ptr::copy_nonoverlapping(s.canvas.px.as_ptr(), s.gdi.bits, s.canvas.px.len());
    let size = SIZE { cx: s.canvas.w as i32, cy: s.canvas.h as i32 };
    let origin = POINT { x: 0, y: 0 };
    let at = POINT { x: s.pos.0, y: s.pos.1 };
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let _ = UpdateLayeredWindow(s.hwnd, None, Some(&at), Some(&size), Some(s.gdi.dc), Some(&origin), COLORREF(0), Some(&blend), ULW_ALPHA);
    s.frames += 1;
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mut input = None;
    let mut handled = true;
    match msg {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => with(|s| {
            if !s.hovered {
                s.hovered = true;
                let mut track = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                let _ = TrackMouseEvent(&mut track);
                let t = now(s);
                s.anim.set_open(t, true);
                s.anim.blink(t + 0.12);
                SetTimer(Some(hwnd), BLINK, BLINK_EVERY_MS, None);
                start_frames(s);
            }
        }),
        WM_MOUSELEAVE => with(|s| {
            s.hovered = false;
            let t = now(s);
            if s.notice.is_none() {
                s.anim.set_open(t, false);
            }
            let _ = KillTimer(Some(hwnd), BLINK);
            start_frames(s);
        }),
        WM_LBUTTONUP => input = Some(Input::Click),
        WM_RBUTTONUP => input = Some(Input::RightClick),
        WM_TIMER => with(|s| match wparam.0 {
            FRAME => {
                render(s);
                if !s.anim.moving(now(s)) {
                    let _ = KillTimer(Some(hwnd), FRAME);
                    s.frame_timer = false;
                    // One last frame at rest, so the end state is exact.
                    render(s);
                }
            }
            BLINK => {
                let t = now(s);
                s.anim.blink(t);
                start_frames(s);
            }
            NOTICE => {
                let _ = KillTimer(Some(hwnd), NOTICE);
                s.notice = None;
                if !s.hovered {
                    s.anim.set_open(now(s), false);
                }
                start_frames(s);
            }
            CLOCK => {
                render(s);
                arm_clock(s);
            }
            _ => {}
        }),
        _ => handled = false,
    }
    if let Some(i) = input {
        // Called with the state released: the host may show or hide the pill.
        let callback = STATE.with(|s| s.try_borrow().ok().and_then(|b| b.as_ref().map(|s| s.on_input.clone())));
        if let Some(f) = callback {
            f(i);
        }
        return LRESULT(0);
    }
    if handled {
        LRESULT(0)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

