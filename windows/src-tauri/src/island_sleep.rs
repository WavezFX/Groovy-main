// The island sleeps at idle (round 4F item 1).
//
// An awake WebView2 costs about 100 MB by itself. When the island has been
// still for a while and nothing is going on, the page asks to sleep
// (`island_sleep`). Windows only:
//
//   1. the island's webview is photographed, on black and on white (WebView2
//      photographs without transparency; the two give it back), behind a
//      screenshot that covers it meanwhile, and that picture is shown by a
//      small native window in the island's exact place, so nothing changes on
//      screen;
//   2. the webview is hidden and suspended (ICoreWebView2_3::TrySuspend), and
//      the working sets of Groovy's processes are handed back to Windows.
//
// It wakes when the cursor comes to the island, or when Rust has something for
// the page: every event the island listens to is watched here. While asleep
// the events are kept (the suspended page can't take them) and delivered, in
// order, once the page is back. Cosmetic ones are dropped, state updates wait
// for the next wake without causing one.
//
// Elsewhere (Linux) the island never sleeps: `island_sleep` answers false.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Listener};

use crate::island::WINDOW_LABEL;

/// Every event the island's page listens to (tests/sleep.test.mjs keeps this
/// list and the page's `onEvent` calls the same).
pub const ISLAND_EVENTS: &[&str] = &[
    "chat-delta",
    "click-outside",
    "cursor",
    "github-alerts",
    "growth-changed",
    "hook",
    "integration",
    "island-glass",
    "island-glass-boost",
    "island-notice",
    "live-bands",
    "live-state",
    "mail-inboxes",
    "mail-notification",
    "memory-pressure",
    "ollama-setup",
    "open-wardrobe",
    "phone-link-decided",
    "phone-link-getdiff",
    "pointer-inside",
    "recap-check",
    "reminder",
    "runtime-release",
    "runtime-state",
    "screen-changed",
    "settings-changed",
    "shortcut",
    "sounds-changed",
    "spotify",
    "spotify-artwork",
    "tray",
    "voice-ask",
    "voice-call-start",
    "voice-say",
    "voice-wake",
];

/// What an event does to a sleeping island.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// Only matters while the page is drawing: forgotten.
    Drop,
    /// A state update: the newest one waits for the next wake.
    Keep,
    /// Something to show or do: wakes the island, then is delivered.
    Wake,
}

pub fn held(event: &str) -> Held {
    match event {
        "cursor" | "pointer-inside" | "live-bands" | "click-outside" => Held::Drop,
        "growth-changed" | "memory-pressure" | "runtime-state" | "runtime-release" | "island-glass" | "island-glass-boost"
        | "sounds-changed" | "mail-inboxes" | "integration" | "spotify-artwork" => Held::Keep,
        _ => Held::Wake,
    }
}

/// The events kept for a sleeping page, in the order to deliver them.
#[derive(Default)]
pub struct Queue {
    items: Vec<(String, String)>,
}

impl Queue {
    /// Takes `event`; true when the island must wake for it.
    pub fn push(&mut self, event: &str, payload: &str) -> bool {
        match held(event) {
            Held::Drop => false,
            Held::Keep => {
                self.items.retain(|(name, _)| name != event);
                self.items.push((event.to_string(), payload.to_string()));
                false
            }
            Held::Wake => {
                // A stream of answers can't grow without bound while he wakes.
                if self.items.len() < 512 {
                    self.items.push((event.to_string(), payload.to_string()));
                }
                true
            }
        }
    }

    pub fn take(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.items)
    }
}

/// Why the island may not sleep right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Blockers {
    pub switched_off: bool,
    /// Started less than a minute ago.
    pub young: bool,
    pub call: bool,
    /// The runtime is in a conversation (wake word, dictation, a reply due).
    pub conversation: bool,
    pub cursor_near: bool,
}

pub fn may_sleep(b: Blockers) -> bool {
    !(b.switched_off || b.young || b.call || b.conversation || b.cursor_near)
}

/// The island as its webview draws it, ready for a layered window.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Picture {
    /// The drawn part only: premultiplied BGRA, top row first.
    pub bgra: Vec<u8>,
    /// The drawn part's box inside the photograph: x, y, w, h.
    pub bounds: (u32, u32, u32, u32),
}

/// WebView2 photographs a page without its transparency, so the island is
/// photographed twice, once on black and once on white (RGB, same size). A
/// pixel that is the same on both is opaque; one that follows the background
/// is clear: alpha = 255 - (white - black), and the black photograph is
/// already the colour multiplied by it.
/// `Ok(None)`: nothing is drawn (the island is folded away).
/// `Err`: the photographs can't stand in for the island.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn picture(on_black: &[u8], on_white: &[u8], width: u32, height: u32) -> Result<Option<Picture>, &'static str> {
    let len = (width as usize) * (height as usize) * 3;
    if width == 0 || height == 0 || on_black.len() != len || on_white.len() != len {
        return Err("the photographs are the wrong size");
    }
    let alpha = |i: usize| -> u8 {
        let d: i32 = (0..3).map(|c| (on_white[i + c] as i32 - on_black[i + c] as i32).max(0)).sum();
        (255 - (d + 1) / 3).clamp(0, 255) as u8
    };
    // Anything fainter than this is the photograph's own noise.
    const FAINT: u8 = 3;
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0u32, 0u32);
    for y in 0..height {
        for x in 0..width {
            if alpha(((y * width + x) * 3) as usize) > FAINT {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return Ok(None);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    // The island is a strip at the top of a wider clear window: opaque from
    // edge to edge means the backgrounds never showed through.
    if w == width && h == height {
        return Err("the photographs show no transparency");
    }
    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * width + x) * 3) as usize;
            let a = alpha(i);
            if a <= FAINT {
                bgra.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                bgra.extend_from_slice(&[on_black[i + 2].min(a), on_black[i + 1].min(a), on_black[i].min(a), a]);
            }
        }
    }
    Ok(Some(Picture { bgra, bounds: (x0, y0, w, h) }))
}

/// Whether the cursor is on the sleeping island (or within `margin` of it).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn cursor_on(cursor: (f64, f64), rect: (f64, f64, f64, f64), margin: f64) -> bool {
    let (x, y, w, h) = rect;
    cursor.0 >= x - margin && cursor.0 <= x + w + margin && cursor.1 >= y - margin && cursor.1 <= y + h + margin
}

const AWAKE: u8 = 0;
const FALLING: u8 = 1;
const ASLEEP: u8 = 2;
const WAKING: u8 = 3;

static PHASE: AtomicU8 = AtomicU8::new(AWAKE);
static QUEUE: Mutex<Queue> = Mutex::new(Queue { items: Vec::new() });
static STARTED: Mutex<Option<Instant>> = Mutex::new(None);
/// Called once at start: watches the island's events for a sleeping page.
pub fn init(app: &AppHandle) {
    *STARTED.lock().unwrap() = Some(Instant::now());
    for name in ISLAND_EVENTS {
        let handle = app.clone();
        app.listen_any(*name, move |event| {
            if PHASE.load(Ordering::Relaxed) == AWAKE {
                return;
            }
            let wake = QUEUE.lock().unwrap().push(name, event.payload());
            if wake {
                wake_up(&handle, name);
            }
        });
    }
}

#[cfg(feature = "ci")]
fn sleep_test() -> bool {
    std::env::var_os("GROOVY_SLEEP_TEST").is_some()
}

#[cfg(not(feature = "ci"))]
fn sleep_test() -> bool {
    false
}

fn blockers(app: &AppHandle) -> Blockers {
    // A `ci` build sleeps only for the sleep test (GROOVY_SLEEP_TEST), and then
    // without the first minute's wait: every other test sees an island that never sleeps.
    let (test_build, sleep_test) = (cfg!(feature = "ci"), sleep_test());
    let young = !test_build && STARTED.lock().unwrap().is_none_or(|t| t.elapsed() < Duration::from_secs(60));
    Blockers {
        switched_off: std::env::var_os("GROOVY_NO_SLEEP").is_some() || !os::SUPPORTED || (test_build && !sleep_test),
        young,
        call: crate::calls::active(),
        conversation: crate::resources::state::state() == crate::resources::state::RunState::Conversation,
        cursor_near: os::cursor_near(app),
    }
}

/// The page has been still for a while and asks to sleep. False: not now.
#[tauri::command]
pub fn island_sleep(app: AppHandle) -> bool {
    let b = blockers(&app);
    if !may_sleep(b) {
        return false;
    }
    if PHASE.compare_exchange(AWAKE, FALLING, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return false;
    }
    os::fall_asleep(&app);
    true
}

/// Asleep for good: called by the OS side once the webview is suspended.
#[cfg_attr(not(windows), allow(dead_code))]
fn fell_asleep(app: &AppHandle) {
    // Something may have arrived while he was falling asleep.
    let pending = QUEUE.lock().unwrap().items.iter().any(|(name, _)| held(name) == Held::Wake);
    PHASE.store(ASLEEP, Ordering::SeqCst);
    crate::log::line("island: asleep (webview suspended, working sets handed back)".to_string());
    if pending {
        wake_up(app, "an event that came while falling asleep");
    }
}

/// The page could not be photographed or suspended: carry on awake.
fn stay_awake(app: &AppHandle, why: &str) {
    crate::log::line(format!("island: not sleeping ({why})"));
    PHASE.store(WAKING, Ordering::SeqCst);
    woke(app, Instant::now());
}

/// Wakes the island (the cursor, an event, anything in Rust that needs the page).
pub fn wake_up(app: &AppHandle, why: &str) {
    if PHASE.compare_exchange(ASLEEP, WAKING, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    crate::log::line(format!("island: waking for {why}"));
    os::wake(app, Instant::now());
}

/// The page is drawing again: deliver what was kept, in order.
fn woke(app: &AppHandle, since: Instant) {
    let items = QUEUE.lock().unwrap().take();
    PHASE.store(AWAKE, Ordering::SeqCst);
    crate::log::line(format!("island: awake after {} ms", since.elapsed().as_millis()));
    for (name, payload) in items {
        let value: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        let _ = app.emit_to(WINDOW_LABEL, &name, value);
    }
    let _ = app.emit_to(WINDOW_LABEL, "island-woke", since.elapsed().as_millis() as u64);
}

#[cfg(windows)]
mod os {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use tauri::{AppHandle, Manager};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2Controller2, ICoreWebView2_3, COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, COREWEBVIEW2_COLOR,
    };
    use webview2_com::{CapturePreviewCompletedHandler, TrySuspendCompletedHandler};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow, UpdateLayeredWindow, SW_SHOWNOACTIVATE, ULW_ALPHA,
        WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };
    use windows::core::w;
    use wv2_core::Interface;
    use wv2_windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use wv2_windows::Win32::System::Com::{STREAM_SEEK_SET, STATFLAG_NONAME, STATSTG};

    use super::{fell_asleep, picture, stay_awake, woke, ASLEEP, PHASE};

    pub const SUPPORTED: bool = true;
    /// How close the cursor may be for the island to fall asleep or to wake (physical px).
    const MARGIN: f64 = 12.0;

    /// The stand-in window (its handle) and the sleeping island's place on screen.
    static STANDIN: Mutex<Option<isize>> = Mutex::new(None);
    /// The screenshot that covers the island while it is photographed.
    static COVER: Mutex<Option<isize>> = Mutex::new(None);
    /// The first photograph (on black): RGB, width, height.
    static ON_BLACK: Mutex<Option<(Vec<u8>, u32, u32)>> = Mutex::new(None);
    static PLACE: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

    fn island_place(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
        let win = crate::island::window(app)?;
        let pos = win.outer_position().ok()?;
        let size = win.outer_size().ok()?;
        Some((pos.x as f64, pos.y as f64, size.width as f64, size.height as f64))
    }

    pub fn cursor_near(app: &AppHandle) -> bool {
        let Some(cursor) = crate::platform::cursor_physical() else { return false };
        let Some(win) = crate::island::window(app) else { return false };
        let scale = win.scale_factor().unwrap_or(1.0);
        let gate = &app.state::<crate::Shared>().gate;
        let Some(place) = island_place(app) else { return false };
        // The island's own shape while it shows; the whole strip while folded.
        let r = *gate.rect.lock().unwrap();
        let rect = if gate.collapsed.load(Ordering::Relaxed) || r.w <= 0.0 {
            place
        } else {
            (place.0 + r.x * scale, place.1 + r.y * scale, r.w * scale, r.h * scale)
        };
        super::cursor_on(cursor, rect, MARGIN)
    }

    unsafe extern "system" fn standin_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    /// Shows premultiplied BGRA pixels at (`x`, `y`) on the screen in a
    /// click-through, top-most layered window. Main thread only.
    unsafe fn show_layered(bgra: &[u8], w: u32, h: u32, x: i32, y: i32) -> Option<isize> {
        if w == 0 || h == 0 || bgra.len() != (w * h * 4) as usize {
            return None;
        }
        static CLASS: std::sync::Once = std::sync::Once::new();
        CLASS.call_once(|| {
            let class = WNDCLASSW { lpfnWndProc: Some(standin_proc), lpszClassName: w!("GroovyIslandAsleep"), ..Default::default() };
            RegisterClassW(&class);
        });
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("GroovyIslandAsleep"),
            w!(""),
            WS_POPUP,
            x,
            y,
            w as i32,
            h as i32,
            None,
            None,
            None,
            None,
        )
        .ok()?;
        let dc = CreateCompatibleDC(None);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &dib(w, h), DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            let _ = DestroyWindow(hwnd);
            return None;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = DestroyWindow(hwnd);
            return None;
        }
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        let old = SelectObject(dc, HGDIOBJ(bitmap.0));
        let size = SIZE { cx: w as i32, cy: h as i32 };
        let origin = POINT { x: 0, y: 0 };
        let at = POINT { x, y };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let drawn = UpdateLayeredWindow(hwnd, None, Some(&at), Some(&size), Some(dc), Some(&origin), COLORREF(0), Some(&blend), ULW_ALPHA);
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        if drawn.is_err() {
            let _ = DestroyWindow(hwnd);
            return None;
        }
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Some(hwnd.0 as isize)
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

    /// What is on the screen in this rectangle right now, opaque, as BGRA.
    unsafe fn screen_pixels(x: i32, y: i32, w: u32, h: u32) -> Option<Vec<u8>> {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(dc), &dib(w, h), DIB_RGB_COLORS, &mut bits, None, 0).ok();
        let mut out = None;
        if let Some(bitmap) = bitmap {
            if !bits.is_null() {
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                if BitBlt(dc, 0, 0, w as i32, h as i32, Some(screen), x, y, SRCCOPY).is_ok() {
                    let mut px = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
                    for p in px.chunks_exact_mut(4) {
                        p[3] = 255;
                    }
                    out = Some(px);
                }
                SelectObject(dc, old);
            }
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        out
    }

    fn drop_window(slot: &Mutex<Option<isize>>) {
        if let Some(h) = slot.lock().unwrap().take() {
            unsafe {
                let _ = DestroyWindow(HWND(h as *mut std::ffi::c_void));
            }
        }
    }

    fn drop_standin() {
        drop_window(&STANDIN);
    }

    /// The PNG the webview wrote into `stream`, decoded to RGB.
    unsafe fn read_png(stream: &wv2_windows::Win32::System::Com::IStream) -> Result<(Vec<u8>, u32, u32), String> {
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME).map_err(|e| e.to_string())?;
        let len = stat.cbSize as usize;
        if len == 0 || len > 64 * 1024 * 1024 {
            return Err(format!("a photograph of {len} bytes"));
        }
        stream.Seek(0, STREAM_SEEK_SET, None).map_err(|e| e.to_string())?;
        let mut bytes = vec![0u8; len];
        let mut read = 0u32;
        let hr = stream.Read(bytes.as_mut_ptr() as *mut std::ffi::c_void, len as u32, Some(&mut read));
        if hr.is_err() || read as usize != len {
            return Err("the photograph could not be read".into());
        }
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; reader.output_buffer_size().ok_or("a photograph too large to decode")?];
        let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(frame.buffer_size());
        let rgb = match frame.color_type {
            png::ColorType::Rgb => buf,
            png::ColorType::Rgba => buf.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
            other => return Err(format!("a photograph in {other:?}")),
        };
        Ok((rgb, frame.width, frame.height))
    }

    /// The webview's background, behind whatever the page leaves clear.
    unsafe fn set_background(controller: &ICoreWebView2Controller, a: u8, v: u8) -> bool {
        controller.cast::<ICoreWebView2Controller2>().is_ok_and(|c| c.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: a, R: v, G: v, B: v }).is_ok())
    }

    /// Gives up falling asleep: the page is as it was, and awake.
    fn give_up(app: &AppHandle, why: &str) {
        *ON_BLACK.lock().unwrap() = None;
        let why = why.to_string();
        let handle = app.clone();
        let restored = crate::island::window(app).map(|win| {
            win.with_webview(move |webview| unsafe {
                set_background(&webview.controller(), 0, 0);
                drop_window(&COVER);
                stay_awake(&handle, &why);
            })
        });
        if !matches!(restored, Some(Ok(()))) {
            drop_window(&COVER);
            stay_awake(app, "the webview is gone");
        }
    }

    /// Falling asleep, in steps on the main thread:
    ///   1. a screenshot of the island covers it, so nothing below shows;
    ///   2. the webview is photographed on a black background, then on white
    ///      (WebView2 photographs without transparency: the two give it back);
    ///   3. the picture made from them takes the cover's place, and the
    ///      webview is hidden and suspended.
    pub fn fall_asleep(app: &AppHandle) {
        let Some(win) = crate::island::window(app) else { return stay_awake(app, "no island window") };
        let Some(place) = island_place(app) else { return stay_awake(app, "no window place") };
        let handle = app.clone();
        let posted = win.with_webview(move |webview| unsafe {
            let app = handle;
            let controller = webview.controller();
            if controller.cast::<ICoreWebView2Controller2>().is_err() || controller.CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_3>()).is_err() {
                return stay_awake(&app, "this WebView2 cannot suspend");
            }
            let (x, y, w, h) = (place.0 as i32, place.1 as i32, place.2 as u32, place.3 as u32);
            let Some(cover) = screen_pixels(x, y, w, h).and_then(|px| show_layered(&px, w, h, x, y)) else {
                return stay_awake(&app, "the screen could not be copied");
            };
            *COVER.lock().unwrap() = Some(cover);
            photograph(&app, place, false);
        });
        if posted.is_err() {
            stay_awake(app, "the webview is gone");
        }
    }

    /// One photograph, on black (`white` false) or on white, a moment after
    /// the background changed so the page has drawn on it.
    fn photograph(app: &AppHandle, place: (f64, f64, f64, f64), white: bool) {
        let Some(win) = crate::island::window(app) else { return give_up(app, "no island window") };
        let ready = win.with_webview(move |webview| unsafe {
            set_background(&webview.controller(), 255, if white { 255 } else { 0 });
        });
        if ready.is_err() {
            return give_up(app, "the webview is gone");
        }
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(90));
            let Some(win) = crate::island::window(&app) else { return give_up(&app, "no island window") };
            let handle = app.clone();
            let posted = win.with_webview(move |webview| unsafe {
                let app = handle;
                let Ok(core) = webview.controller().CoreWebView2() else { return give_up(&app, "no webview") };
                let Ok(stream) = CreateStreamOnHGlobal(Default::default(), true) else { return give_up(&app, "no memory stream") };
                let (after, stream_after) = (app.clone(), stream.clone());
                let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                    let app = after;
                    if let Err(err) = result {
                        give_up(&app, &format!("the photograph failed: {err}"));
                        return Ok(());
                    }
                    match read_png(&stream_after) {
                        Err(err) => give_up(&app, &err),
                        Ok(shot) if !white => {
                            *ON_BLACK.lock().unwrap() = Some(shot);
                            photograph(&app, place, true);
                        }
                        Ok(shot) => finish(&app, place, shot),
                    }
                    Ok(())
                }));
                if let Err(err) = core.CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, &stream, &handler) {
                    give_up(&app, &format!("the photograph could not start: {err}"));
                }
            });
            if posted.is_err() {
                give_up(&app, "the webview is gone");
            }
        });
    }

    /// Both photographs are in: the stand-in replaces the cover, the webview goes to sleep.
    fn finish(app: &AppHandle, place: (f64, f64, f64, f64), on_white: (Vec<u8>, u32, u32)) {
        let Some((black, w, h)) = ON_BLACK.lock().unwrap().take() else { return give_up(app, "the first photograph is missing") };
        let (white, w2, h2) = on_white;
        if (w, h) != (w2, h2) {
            return give_up(app, "the two photographs differ in size");
        }
        // The photographs may be at another scale than the window.
        if ((place.2 / w as f64) - 1.0).abs() > 0.01 || ((place.3 / h as f64) - 1.0).abs() > 0.01 {
            return give_up(app, "the photographs are not at the window's scale");
        }
        let pic = match picture(&black, &white, w, h) {
            Ok(pic) => pic,
            Err(err) => return give_up(app, err),
        };
        let Some(win) = crate::island::window(app) else { return give_up(app, "no island window") };
        let handle = app.clone();
        let posted = win.with_webview(move |webview| unsafe {
            let app = handle;
            let controller = webview.controller();
            let rect = match &pic {
                // Folded away: nothing to show, the cursor wakes it over its strip.
                None => place,
                Some(pic) => {
                    let (bx, by, bw, bh) = pic.bounds;
                    let (x, y) = (place.0 + bx as f64, place.1 + by as f64);
                    match show_layered(&pic.bgra, bw, bh, x as i32, y as i32) {
                        Some(hwnd) => *STANDIN.lock().unwrap() = Some(hwnd),
                        None => return give_up(&app, "the stand-in window could not be made"),
                    }
                    (x, y, bw as f64, bh as f64)
                }
            };
            *PLACE.lock().unwrap() = Some(rect);
            if controller.SetIsVisible(false).is_err() {
                drop_standin();
                return give_up(&app, "the webview could not be hidden");
            }
            // Hidden: back to its clear background for the wake, and the cover can go.
            set_background(&controller, 0, 0);
            drop_window(&COVER);
            let Ok(core3) = controller.CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_3>()) else { return asleep_now(&app) };
            let done = app.clone();
            let suspended = TrySuspendCompletedHandler::create(Box::new(move |result, ok| {
                if result.is_err() || !ok {
                    crate::log::line("island: hidden but not suspended (WebView2 said no)".to_string());
                }
                asleep_now(&done);
                Ok(())
            }));
            if core3.TrySuspend(&suspended).is_err() {
                asleep_now(&app);
            }
        });
        if posted.is_err() {
            give_up(app, "the webview is gone");
        }
    }

    /// Suspended: park the 60 Hz cursor poll, hand memory back, watch the cursor.
    fn asleep_now(app: &AppHandle) {
        let gate = app.state::<crate::Shared>().gate.clone();
        gate.set_active(false);
        fell_asleep(app);
        let app = app.clone();
        std::thread::Builder::new()
            .name("groovy-island-asleep".into())
            .spawn(move || {
                // Let the suspended renderer settle, then give the pages back.
                // Not while another Groovy window (Settings) is being used:
                // its pages are handed back once it loses the focus.
                std::thread::sleep(Duration::from_millis(1500));
                let mut handed_back = false;
                let mut ticks: u32 = 0;
                while PHASE.load(Ordering::Relaxed) == ASLEEP {
                    if ticks % 20 == 0 {
                        let in_use = app.webview_windows().values().any(|w| w.label() != crate::island::WINDOW_LABEL && w.is_focused().unwrap_or(false));
                        if in_use {
                            handed_back = false;
                        } else if !handed_back {
                            handed_back = true;
                            let before = crate::resources::ceiling::total_mb();
                            crate::resources::ceiling::trim_tree();
                            std::thread::sleep(Duration::from_millis(300));
                            crate::log::line(format!(
                                "island: asleep, {before:.0} MB suspended, {:.0} MB after handing memory back",
                                crate::resources::ceiling::total_mb()
                            ));
                        }
                    }
                    ticks = ticks.wrapping_add(1);
                    std::thread::sleep(Duration::from_millis(100));
                    let Some(cursor) = crate::platform::cursor_physical() else { continue };
                    let rect = *PLACE.lock().unwrap();
                    if rect.is_some_and(|r| super::cursor_on(cursor, r, MARGIN)) {
                        super::wake_up(&app, "the cursor");
                    }
                }
            })
            .ok();
    }

    pub fn wake(app: &AppHandle, since: Instant) {
        let Some(win) = crate::island::window(app) else { return woke(app, since) };
        let handle = app.clone();
        let posted = win.with_webview(move |webview| unsafe {
            let controller = webview.controller();
            if let Ok(core3) = controller.CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_3>()) {
                let _ = core3.Resume();
            }
            let _ = controller.SetIsVisible(true);
            let app = handle;
            // The page needs a frame or two before the stand-in can go.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(120));
                let main = app.clone();
                let _ = app.run_on_main_thread(move || {
                    drop_standin();
                    let shared = main.state::<crate::Shared>();
                    if !shared.gate.collapsed.load(Ordering::Relaxed) {
                        shared.gate.set_active(true);
                    }
                    woke(&main, since);
                });
            });
        });
        if posted.is_err() {
            drop_standin();
            woke(app, since);
        }
    }
}

#[cfg(not(windows))]
mod os {
    use std::time::Instant;

    use tauri::AppHandle;

    pub const SUPPORTED: bool = false;

    pub fn cursor_near(_app: &AppHandle) -> bool {
        false
    }

    pub fn fall_asleep(app: &AppHandle) {
        super::stay_awake(app, "not on this system");
    }

    pub fn wake(app: &AppHandle, since: Instant) {
        super::woke(app, since);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_sleeps_through_a_call_a_conversation_or_a_near_cursor() {
        assert!(may_sleep(Blockers::default()));
        for b in [
            Blockers { switched_off: true, ..Default::default() },
            Blockers { young: true, ..Default::default() },
            Blockers { call: true, ..Default::default() },
            Blockers { conversation: true, ..Default::default() },
            Blockers { cursor_near: true, ..Default::default() },
        ] {
            assert!(!may_sleep(b), "{b:?}");
        }
    }

    #[test]
    fn events_for_a_sleeping_page_are_kept_in_order_and_only_some_wake_it() {
        let mut q = Queue::default();
        assert!(!q.push("cursor", "{\"x\":1}"), "the cursor's place is of no use to a sleeping page");
        assert!(!q.push("growth-changed", "1"));
        assert!(!q.push("growth-changed", "2"), "a state update waits");
        assert!(q.push("island-notice", "\"hello\""), "a notice wakes him");
        assert!(q.push("hook", "{}"));
        assert_eq!(
            q.take(),
            vec![("growth-changed".to_string(), "2".to_string()), ("island-notice".to_string(), "\"hello\"".to_string()), ("hook".to_string(), "{}".to_string())],
            "the newest state update, then what woke him, in order; the cursor dropped"
        );
        assert!(q.take().is_empty());
    }

    #[test]
    fn an_event_nobody_classified_wakes_the_island() {
        assert_eq!(held("something-new"), Held::Wake);
        for name in ISLAND_EVENTS {
            let _ = held(name);
        }
        assert_eq!(held("live-bands"), Held::Drop);
        assert_eq!(held("runtime-state"), Held::Keep);
        assert_eq!(held("voice-say"), Held::Wake);
    }

    /// The island drawn in `rgb` at `alpha` (0-255) inside `drawn`, photographed on a `background`.
    fn photo(w: u32, h: u32, drawn: (u32, u32, u32, u32), rgb: [u8; 3], alpha: u32, background: u8) -> Vec<u8> {
        let mut px = vec![background; (w * h * 3) as usize];
        for y in drawn.1..drawn.1 + drawn.3 {
            for x in drawn.0..drawn.0 + drawn.2 {
                for c in 0..3 {
                    px[((y * w + x) * 3) as usize + c] = ((rgb[c] as u32 * alpha + background as u32 * (255 - alpha) + 127) / 255) as u8;
                }
            }
        }
        px
    }

    #[test]
    fn the_stand_in_comes_from_a_photograph_on_black_and_one_on_white() {
        let drawn = (5, 0, 8, 4);
        let (black, white) = (photo(20, 10, drawn, [200, 100, 50], 128, 0), photo(20, 10, drawn, [200, 100, 50], 128, 255));
        let pic = picture(&black, &white, 20, 10).unwrap().expect("something is drawn");
        assert_eq!(pic.bounds, drawn);
        assert_eq!(pic.bgra.len(), 8 * 4 * 4);
        // BGRA, each colour already multiplied by its alpha (half of 50, 100, 200).
        assert_eq!(&pic.bgra[..4], &[25, 50, 100, 128]);
        // A fully opaque island comes out opaque, in its own colours.
        let (black, white) = (photo(20, 10, drawn, [10, 20, 30], 255, 0), photo(20, 10, drawn, [10, 20, 30], 255, 255));
        let pic = picture(&black, &white, 20, 10).unwrap().unwrap();
        assert_eq!(&pic.bgra[..4], &[30, 20, 10, 255]);
    }

    #[test]
    fn a_folded_island_needs_no_stand_in_and_photographs_without_transparency_are_refused() {
        let (black, white) = (vec![0u8; 20 * 10 * 3], vec![255u8; 20 * 10 * 3]);
        assert!(picture(&black, &white, 20, 10).unwrap().is_none(), "nothing drawn: the backgrounds show everywhere");
        let same = photo(20, 10, (0, 0, 20, 10), [9, 9, 9], 255, 0);
        assert!(picture(&same, &same, 20, 10).is_err(), "the same on black and on white: a black box would be painted");
        assert!(picture(&[0u8; 7], &[0u8; 7], 20, 10).is_err());
    }

    #[test]
    fn the_cursor_wakes_him_on_the_island_and_just_beside_it() {
        let rect = (100.0, 0.0, 200.0, 40.0);
        assert!(cursor_on((150.0, 20.0), rect, 12.0));
        assert!(cursor_on((95.0, 45.0), rect, 12.0));
        assert!(!cursor_on((50.0, 20.0), rect, 12.0));
        assert!(!cursor_on((150.0, 80.0), rect, 12.0));
    }
}
