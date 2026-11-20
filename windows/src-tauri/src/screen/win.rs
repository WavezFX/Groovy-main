//! Round 4C item 2: the Windows capture, GDI `BitBlt` from the screen's DC.
//!
//! The monitor under the pointer, or the active window cut from it. Groovy's own
//! windows are excluded from the capture (`WDA_EXCLUDEFROMCAPTURE`) only while it
//! is taken, so the user's own screenshots still show him. The full bitmap is
//! freed before this returns; only the small JPEG leaves.

use super::shrink::{self, Shot};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
    DIB_RGB_COLORS, MONITORINFO, MONITOR_DEFAULTTONEAREST, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowRect, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE};

/// Why a capture failed: `Locked` when Windows wouldn't give the screen (a locked
/// or secure desktop), otherwise the words.
pub enum Failed {
    Locked,
    Other(String),
}

/// The area to capture in desktop pixels: the monitor under the pointer, or the
/// active window's part of it (when `window` and the active window isn't ours).
fn area(window: bool, own: &[isize]) -> Result<(RECT, POINT), Failed> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).map_err(|_| Failed::Locked)?;
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return Err(Failed::Other("Windows didn't say where the monitor is".into()));
        }
        let mut r = mi.rcMonitor;
        if window {
            let fg = GetForegroundWindow();
            let mut wr = RECT::default();
            if !fg.is_invalid() && !own.contains(&(fg.0 as isize)) && GetWindowRect(fg, &mut wr).is_ok() {
                let cut = RECT { left: wr.left.max(r.left), top: wr.top.max(r.top), right: wr.right.min(r.right), bottom: wr.bottom.min(r.bottom) };
                if cut.right - cut.left > 32 && cut.bottom - cut.top > 32 {
                    r = cut;
                }
            }
        }
        Ok((r, pt))
    }
}

fn exclude(own: &[isize], on: bool) {
    for &h in own {
        unsafe {
            let _ = SetWindowDisplayAffinity(HWND(h as *mut _), if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }
}

/// One capture, made small. `own` are Groovy's window handles.
pub fn capture(window: bool, own: &[isize]) -> Result<Shot, Failed> {
    let (r, pt) = area(window, own)?;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return Err(Failed::Other("the screen area is empty".into()));
    }
    exclude(own, true);
    let result = unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let out = match CreateDIBSection(Some(screen), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(hbm) if !bits.is_null() => {
                let old = SelectObject(mem, hbm.into());
                let out = match BitBlt(mem, 0, 0, w, h, Some(screen), r.left, r.top, SRCCOPY | CAPTUREBLT) {
                    Ok(()) => {
                        let pixels = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize);
                        shrink::shot_from_bgra(pixels, w as u32, h as u32, w as usize * 4, (r.left, r.top), Some((pt.x, pt.y))).map_err(Failed::Other)
                    }
                    Err(_) => Err(Failed::Locked),
                };
                SelectObject(mem, old);
                let _ = DeleteObject(hbm.into());
                out
            }
            _ => Err(Failed::Other("Windows had no memory for the picture".into())),
        };
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        out
    };
    exclude(own, false);
    result
}
