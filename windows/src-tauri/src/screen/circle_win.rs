//! Round 4C item 5, step 5: circle and ask, Windows. A native layered window
//! dims the monitor under the pointer; the user draws round something with the
//! mouse (Escape or a right click cancels). On release the dimming goes and only
//! their yellow stroke stays, click-through, so the picture taken next has the
//! circle on it; it fades a few seconds later. No WebView; made on use only.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{mpsc, Mutex, Once};
use std::time::Duration;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetMonitorInfoW, InvalidateRect, MonitorFromPoint, Polyline, SelectObject, UpdateWindow, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_SOLID,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, ShowWindow, TranslateMessage, GWL_EXSTYLE, IDC_CROSS, LWA_ALPHA, LWA_COLORKEY, MSG, SW_SHOW, WM_APP, WM_DESTROY, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const CLASS: PCWSTR = w!("GroovyCircleAsk");
const KEY: u32 = 0x00010001;
const DIM: u32 = 0x00101010;
const YELLOW: u32 = 0x0000D7FF;
const FADE: usize = 1;
const ESC: usize = 2;
const FADE_NOW: u32 = WM_APP + 1;

static CURRENT: AtomicIsize = AtomicIsize::new(0);
static DRAWING: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);
static PATH: Mutex<Vec<POINT>> = Mutex::new(Vec::new());
/// The monitor's top-left on the desktop (the window's client origin).
static ORIGIN: Mutex<(i32, i32)> = Mutex::new((0, 0));
static ANSWER: Mutex<Option<mpsc::Sender<Option<(i32, i32, i32, i32)>>>> = Mutex::new(None);

fn answer(a: Option<(i32, i32, i32, i32)>) {
    if let Some(tx) = ANSWER.lock().unwrap().take() {
        let _ = tx.send(a);
    }
}

/// Waits (up to a minute) for the user to circle something; the circle's
/// bounds in desktop pixels, or None when cancelled.
pub fn run() -> Option<(i32, i32, i32, i32)> {
    if CURRENT.load(Ordering::SeqCst) != 0 {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    *ANSWER.lock().unwrap() = Some(tx);
    PATH.lock().unwrap().clear();
    DONE.store(false, Ordering::SeqCst);
    std::thread::spawn(|| unsafe { window() });
    let got = rx.recv_timeout(Duration::from_secs(60)).unwrap_or(None);
    if got.is_none() {
        close();
    }
    got
}

/// The picture is taken: the stroke fades in a few seconds.
pub fn fade() {
    let h = CURRENT.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), FADE_NOW, WPARAM(0), LPARAM(0));
        }
    }
}

fn close() {
    let h = CURRENT.swap(0, Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), FADE_NOW, WPARAM(1), LPARAM(0));
        }
    }
}

unsafe fn window() {
    static REGISTER: Once = Once::new();
    let Ok(module) = GetModuleHandleW(None) else { return answer(None) };
    REGISTER.call_once(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&class);
    });
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST), &mut mi).as_bool() {
        return answer(None);
    }
    let r = mi.rcMonitor;
    *ORIGIN.lock().unwrap() = (r.left, r.top);
    let Ok(hwnd) = CreateWindowExW(WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW, CLASS, w!(""), WS_POPUP, r.left, r.top, r.right - r.left, r.bottom - r.top, None, None, Some(module.into()), None) else {
        return answer(None);
    };
    CURRENT.store(hwnd.0 as isize, Ordering::SeqCst);
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 70, LWA_ALPHA);
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    SetTimer(Some(hwnd), ESC, 40, None);
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let _ = CURRENT.compare_exchange(hwnd.0 as isize, 0, Ordering::SeqCst, Ordering::SeqCst);
    answer(None);
}

fn at(lparam: LPARAM) -> POINT {
    POINT { x: (lparam.0 & 0xFFFF) as u16 as i16 as i32, y: ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32 }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN if !DONE.load(Ordering::SeqCst) => {
            SetCapture(hwnd);
            DRAWING.store(true, Ordering::SeqCst);
            let mut p = PATH.lock().unwrap();
            p.clear();
            p.push(at(lparam));
            LRESULT(0)
        }
        WM_MOUSEMOVE if DRAWING.load(Ordering::SeqCst) => {
            let mut p = PATH.lock().unwrap();
            if p.len() < 4000 {
                p.push(at(lparam));
            }
            drop(p);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_LBUTTONUP if DRAWING.load(Ordering::SeqCst) => {
            let _ = ReleaseCapture();
            DRAWING.store(false, Ordering::SeqCst);
            let bounds = {
                let p = PATH.lock().unwrap();
                let (o, mut b) = (*ORIGIN.lock().unwrap(), (i32::MAX, i32::MAX, i32::MIN, i32::MIN));
                for q in p.iter() {
                    b = (b.0.min(q.x + o.0), b.1.min(q.y + o.1), b.2.max(q.x + o.0), b.3.max(q.y + o.1));
                }
                (b.2 - b.0 >= 12 || b.3 - b.1 >= 12).then_some(b)
            };
            let Some(bounds) = bounds else {
                // A click, not a circle: try again.
                PATH.lock().unwrap().clear();
                let _ = InvalidateRect(Some(hwnd), None, false);
                return LRESULT(0);
            };
            // Only the stroke stays, and the mouse goes back to the screen.
            DONE.store(true, Ordering::SeqCst);
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TRANSPARENT.0 as isize);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(KEY), 255, LWA_COLORKEY);
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
            answer(Some(bounds));
            LRESULT(0)
        }
        WM_RBUTTONDOWN if !DONE.load(Ordering::SeqCst) => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ESC => {
            if !DONE.load(Ordering::SeqCst) && GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000 != 0 {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == FADE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        FADE_NOW => {
            if wparam.0 == 1 {
                let _ = DestroyWindow(hwnd);
            } else {
                SetTimer(Some(hwnd), FADE, 4000, None);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let back = CreateSolidBrush(COLORREF(if DONE.load(Ordering::SeqCst) { KEY } else { DIM }));
            FillRect(dc, &ps.rcPaint, back);
            let _ = DeleteObject(back.into());
            let p = PATH.lock().unwrap();
            if p.len() > 1 {
                let pen = CreatePen(PS_SOLID, 5, COLORREF(YELLOW));
                let old = SelectObject(dc, pen.into());
                let _ = Polyline(dc, &p);
                SelectObject(dc, old);
                let _ = DeleteObject(pen.into());
            }
            drop(p);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
