//! Round 4C item 5: marks on the screen, Windows. A native, click-through,
//! always-on-top layered window over the area that was looked at, painted with
//! GDI (no WebView, so drawing adds almost nothing to memory). It is made on
//! first use, on its own thread, and destroyed when the marks clear: after 6 s,
//! on "clear that", or at the next request when asked to keep them (60 s at most).

use super::marks::{Mark, Shape};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, Once};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, FillRect, GetStockObject, LineTo, MoveToEx, Rectangle, SelectObject, SetBkMode, SetTextColor, TextOutW,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, FW_BOLD, HGDIOBJ, NONANTIALIASED_QUALITY, NULL_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage, RegisterClassW, SetLayeredWindowAttributes, SetTimer, ShowWindow, TranslateMessage,
    LWA_ALPHA, LWA_COLORKEY, MSG, SW_SHOWNOACTIVATE, WM_CLOSE, WM_DESTROY, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};

/// The see-through colour: never used by a mark.
const KEY: u32 = 0x00010001;
const CLASS: PCWSTR = w!("GroovyScreenMarks");

static CURRENT: AtomicIsize = AtomicIsize::new(0);
/// What the window paints: the marks and the window's top-left on the desktop.
static PAINT: Mutex<Option<(Vec<Mark>, (i32, i32))>> = Mutex::new(None);

/// Takes the marks away now (and the window with them).
pub fn clear() {
    let h = CURRENT.swap(0, Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Draws `marks` over `area` (left, top, width, height in desktop pixels).
pub fn show(marks: Vec<Mark>, area: (i32, i32, i32, i32), keep: bool) {
    clear();
    *PAINT.lock().unwrap() = Some((marks, (area.0, area.1)));
    std::thread::spawn(move || unsafe { run(area, if keep { 60_000 } else { 6_000 }) });
}

unsafe fn run(area: (i32, i32, i32, i32), ms: u32) {
    static REGISTER: Once = Once::new();
    let Ok(module) = GetModuleHandleW(None) else { return };
    REGISTER.call_once(|| {
        let class = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: module.into(), lpszClassName: CLASS, ..Default::default() };
        RegisterClassW(&class);
    });
    let Ok(hwnd) = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        CLASS,
        w!(""),
        WS_POPUP,
        area.0,
        area.1,
        area.2,
        area.3,
        None,
        None,
        Some(module.into()),
        None,
    ) else {
        return;
    };
    CURRENT.store(hwnd.0 as isize, Ordering::SeqCst);
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(KEY), 235, LWA_COLORKEY | LWA_ALPHA);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    SetTimer(Some(hwnd), 1, ms, None);
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let _ = CURRENT.compare_exchange(hwnd.0 as isize, 0, Ordering::SeqCst, Ordering::SeqCst);
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_TIMER | WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut ps);
    let key = CreateSolidBrush(COLORREF(KEY));
    FillRect(dc, &ps.rcPaint, key);
    let _ = DeleteObject(key.into());
    let guard = PAINT.lock().unwrap();
    if let Some((marks, (ox, oy))) = guard.as_ref() {
        SetBkMode(dc, TRANSPARENT);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        for m in marks {
            let colour = COLORREF(m.colour.colorref());
            let (x, y, x2, y2) = (m.x - ox, m.y - oy, m.x2 - ox, m.y2 - oy);
            let width = if m.shape == Shape::Highlight { 8 } else { 4 };
            let pen = CreatePen(PS_SOLID, width, colour);
            let old_pen = SelectObject(dc, pen.into());
            match m.shape {
                Shape::Circle => {
                    let _ = Ellipse(dc, x - 6, y - 6, x2 + 6, y2 + 6);
                }
                Shape::Box | Shape::Highlight => {
                    let _ = Rectangle(dc, x - 4, y - 4, x2 + 4, y2 + 4);
                }
                Shape::Underline => {
                    let _ = MoveToEx(dc, x, y2 + 3, None);
                    let _ = LineTo(dc, x2, y2 + 3);
                }
                Shape::Arrow => {
                    let _ = MoveToEx(dc, x, y, None);
                    let _ = LineTo(dc, x2, y2);
                    // The head: two short strokes back from the tip.
                    let (dx, dy) = ((x2 - x) as f64, (y2 - y) as f64);
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let (ux, uy) = (dx / len, dy / len);
                    for side in [-1.0f64, 1.0] {
                        let (hx, hy) = (x2 as f64 - 18.0 * ux + side * 10.0 * -uy, y2 as f64 - 18.0 * uy + side * 10.0 * ux);
                        let _ = MoveToEx(dc, x2, y2, None);
                        let _ = LineTo(dc, hx as i32, hy as i32);
                    }
                }
                Shape::Write => {}
            }
            SelectObject(dc, old_pen);
            let _ = DeleteObject(pen.into());
            if !m.text.is_empty() {
                let size = if m.shape == Shape::Write { (y2 - y).max(12) } else { 20 };
                let font = CreateFontW(size, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, NONANTIALIASED_QUALITY, 0, w!("Segoe UI"));
                let old_font = SelectObject(dc, HGDIOBJ(font.0));
                SetTextColor(dc, colour);
                let (tx, ty) = match m.shape {
                    Shape::Write => (x, y),
                    Shape::Arrow => (x + 6, y + 6),
                    _ => (x, y2 + 10),
                };
                let _ = TextOutW(dc, tx, ty, &wide(&m.text));
                SelectObject(dc, old_font);
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
        SelectObject(dc, old_brush);
    }
    drop(guard);
    let _ = EndPaint(hwnd, &ps);
    let _: RECT = ps.rcPaint;
}
