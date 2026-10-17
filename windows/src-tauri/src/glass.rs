// The glass island (FIXES_ROUND3 item 2c, round 3B item 1).
//
// Windows blurs what is behind the island with the legacy "accent" blur
// (SetWindowCompositionAttribute, ACCENT_ENABLE_ACRYLICBLURBEHIND on Windows
// 11, the plain blur on Windows 10), behind a transparent webview. Unlike the
// DWM system backdrop (Tauri's Effect::Acrylic, DWMWA_SYSTEMBACKDROP_TYPE),
// which fills the whole window rectangle whatever the region (the owner's
// "grey square"), the accent blur stays inside the window region.
//
// The region (square top, rounded bottom corners, the CSS radius at the
// monitor's scale) is set in every style, Solid included, and again on every
// size change (set_island_rect, island::fit), so nothing outside the island is
// ever drawn. Where the accent can't be set, the page draws a CSS glass (a
// darker tint, no blur) on the transparent window: "css".
//
// Solid (the black island) when the user chose it, when Windows' own
// "Transparency effects" switch is off, or while battery saver is on: the
// watcher looks again every 30 seconds.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::island::WINDOW_LABEL;

/// What the island looks like: the black island, Windows' blur, or the page's own glass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Solid = 0,
    Blur = 1,
    Css = 2,
}

impl Look {
    pub fn as_str(self) -> &'static str {
        match self {
            Look::Solid => "solid",
            Look::Blur => "blur",
            Look::Css => "css",
        }
    }
    fn from_u8(v: u8) -> Look {
        match v {
            1 => Look::Blur,
            2 => Look::Css,
            _ => Look::Solid,
        }
    }
}

static LOOK: AtomicU8 = AtomicU8::new(0);
/// The island's last shape: window-logical x, y, width, height and the
/// bottom corners' radius.
static SHAPE: Mutex<(f64, f64, f64, f64, f64)> = Mutex::new((0.0, 0.0, 0.0, 0.0, 0.0));

/// Glass is drawn when it is chosen and Windows allows it right now.
pub fn wanted(style: &str, transparency: bool, battery_saver: bool) -> bool {
    style != "solid" && transparency && !battery_saver
}

/// The look for a wanted glass and whether Windows took the accent blur.
pub fn decide(glass: bool, accent_set: bool) -> Look {
    match (glass, accent_set) {
        (false, _) => Look::Solid,
        (true, true) => Look::Blur,
        (true, false) => Look::Css,
    }
}

/// The window region, in physical pixels: the island's rectangle with its
/// bottom corners rounded. A round rectangle that starts above the window
/// leaves the top corners square. None for an island with no size (hidden):
/// an empty region, nothing drawn.
pub fn region(shape: (f64, f64, f64, f64, f64), scale: f64) -> Option<(i32, i32, i32, i32, i32)> {
    let (x, y, w, h, r) = shape;
    if w < 1.0 || h < 1.0 || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let left = (x * scale).round() as i32;
    let right = ((x + w) * scale).round() as i32;
    let bottom = ((y + h) * scale).round() as i32;
    let radius = (r.clamp(0.0, h.min(w / 2.0)) * scale).round() as i32;
    Some((left, (y * scale).round() as i32 - 2 * radius, right + 1, bottom + 1, 2 * radius))
}

/// The left edge of an island `w` wide centred in a window `window_w` wide.
pub fn centred_left(window_w: f64, w: f64) -> f64 {
    ((window_w - w) / 2.0).max(0.0)
}

pub fn look() -> Look {
    Look::from_u8(LOOK.load(Ordering::Relaxed))
}

/// Turns the blur on or off as the settings and Windows say, and tells the
/// island page (which then draws a light tint, or its own glass, instead of black).
pub fn apply(app: &AppHandle) {
    let style = app
        .try_state::<crate::Shared>()
        .map(|s| s.settings.lock().unwrap().island_style.clone())
        .unwrap_or_else(|| "glass".into());
    let glass = cfg!(windows) && wanted(&style, os::transparency_on(), os::battery_saver_on());
    let Some(win) = app.get_webview_window(WINDOW_LABEL) else { return };
    let before = look();
    let now = match (glass, before) {
        // Already blurred (or on the page's glass): nothing to ask Windows again.
        (true, Look::Blur | Look::Css) => before,
        // Round 3C item 1: Windows 11's host backdrop, clipped to the island's
        // shape, before anything else; then the accent blur if it stays in the
        // shape; else the page's tint, opaque enough to read.
        (true, Look::Solid) if crate::backdrop::active() => {
            crate::backdrop::show(app, true);
            Look::Blur
        }
        (true, Look::Solid) => decide(true, !BLUR_LEAKS.load(Ordering::Relaxed) && os::set_accent(&win, true)),
        (false, _) => {
            if before == Look::Blur {
                if crate::backdrop::active() {
                    crate::backdrop::show(app, false);
                } else {
                    os::set_accent(&win, false);
                }
            }
            Look::Solid
        }
    };
    LOOK.store(now as u8, Ordering::Relaxed);
    reshape(&win);
    if now != before {
        crate::log::line(format!("island style: {}", now.as_str()));
        let _ = app.emit_to(WINDOW_LABEL, "island-glass", now.as_str());
        if now == Look::Blur && !crate::backdrop::active() {
            check_blur_later(app.clone());
        }
    }
}

/// Some Windows builds and sessions (CI's runner among them) draw the accent
/// "blur" as a flat, opaque tint over the whole window rectangle, whatever the
/// region: the grey square again, and nothing shows through. Shortly after the
/// blur goes on, one pixel just outside the island's rounded corner is read
/// from the screen: if it is the tint while the desktop beside the window is
/// not, the blur is turned off for this run and the page draws its own glass
/// (round 3B items 1 and 1b).
fn check_blur_later(app: AppHandle) {
    let _ = std::thread::Builder::new().name("groovy-glass-check".into()).spawn(move || {
        // Read once the island is open (folded away, its window is a thin strip),
        // then keep looking for a few seconds: on CI's runner the tint sometimes
        // only fills the corners a few seconds after the island first shows, so
        // a single early reading missed it (build of fee1ba8).
        let mut opened = None;
        // A dark window behind the corner is only ruled out twice: the blur is
        // never flickered off and on every second.
        let mut tries = 2;
        for _ in 0..60 {
            std::thread::sleep(Duration::from_millis(1000));
            if look() != Look::Blur {
                return;
            }
            let Some(win) = app.get_webview_window(WINDOW_LABEL) else { return };
            let Some((corner, beside)) = os::corner_pixels(&win) else { continue };
            let since = *opened.get_or_insert_with(std::time::Instant::now);
            if tries > 0 && blur_leaks(corner, &beside) && {
                tries -= 1;
                confirm_leak(&win, &beside)
            } {
                os::set_accent(&win, false);
                BLUR_LEAKS.store(true, Ordering::Relaxed);
                LOOK.store(Look::Css as u8, Ordering::Relaxed);
                reshape(&win);
                crate::log::line(format!("island style: css (the blur filled the corners: rgb{corner:?})"));
                let _ = app.emit_to(WINDOW_LABEL, "island-glass", Look::Css.as_str());
                return;
            }
            if since.elapsed() >= WATCH_CORNERS {
                return;
            }
        }
    });
}

/// How long after the island opens its corners are still checked.
const WATCH_CORNERS: Duration = Duration::from_secs(12);

/// Something dark behind the corner (a dark browser tab) looks like the tint:
/// the blur is turned off for a moment, and only if the corner then changes
/// was it the blur (round 3C item 1: a false alarm here left the owner with
/// clear glass). Windows can take more than a frame to repaint without the
/// accent, so the corner is read a few times before deciding. The accent is
/// left off when the leak is confirmed, back on otherwise.
fn confirm_leak(win: &tauri::WebviewWindow, beside: &[(u8, u8, u8)]) -> bool {
    os::set_accent(win, false);
    for _ in 0..6 {
        std::thread::sleep(Duration::from_millis(200));
        if os::corner_pixels(win).is_some_and(|(c, _)| !blur_leaks(c, beside)) {
            return true;
        }
    }
    os::set_accent(win, true);
    false
}

/// Set once the accent blur was seen filling the corners: this run keeps the page's glass.
static BLUR_LEAKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The accent's tint, as it shows when Windows draws it flat.
const TINT_RGB: (u8, u8, u8) = (0x12, 0x14, 0x18);

/// The pixel outside the rounded corner is the flat tint while the desktop
/// next to the window isn't: the blur is drawn outside the island.
pub fn blur_leaks(corner: (u8, u8, u8), beside: &[(u8, u8, u8)]) -> bool {
    let near = |a: (u8, u8, u8), b: (u8, u8, u8)| a.0.abs_diff(b.0) <= 6 && a.1.abs_diff(b.1) <= 6 && a.2.abs_diff(b.2) <= 6;
    near(corner, TINT_RGB) && !beside.is_empty() && beside.iter().all(|&b| !near(b, TINT_RGB))
}

/// The island moved or changed size: the region follows it, in this same call.
pub fn shape(app: &AppHandle, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    *SHAPE.lock().unwrap() = (x, y, w, h, radius);
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        reshape(&win);
    }
}

/// The window was resized: the region is centred on it again.
pub fn refit(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        reshape(&win);
    }
}

fn reshape(win: &tauri::WebviewWindow) {
    let scale = win.scale_factor().unwrap_or(1.0);
    let mut shape = *SHAPE.lock().unwrap();
    if shape.2 < 1.0 {
        // Nothing measured yet: leave the window as it is.
        return;
    }
    // The island sits at the top centre of the window, whose size follows
    // it (island::fit): centre it on the window as it is now, not as the
    // page last measured it.
    if let Ok(size) = win.inner_size() {
        shape.0 = centred_left(size.width as f64 / scale, shape.2);
    }
    let r = region(shape, scale);
    os::set_region(win, r);
    // The frosted backdrop is clipped to the same shape (round 3C item 1).
    if crate::backdrop::active() {
        let (x, y, w, h, radius) = shape;
        let px = |v: f64| (v * scale) as f32;
        crate::backdrop::shape(win.app_handle(), (px(x), px(y), px(w), px(h), px(radius.clamp(0.0, h.min(w / 2.0)))));
    }
    // Windows works out where the accent blur goes when it is set: set after
    // a new region, or it keeps filling the old one (CI's corner test saw the
    // tint in the corners, round 3B item 1).
    if look() == Look::Blur {
        os::set_accent(win, true);
    }
}

/// How often the backdrop's brightness is looked at, at most (round 3B item 1b).
const SAMPLE_EVERY: Duration = Duration::from_secs(60);

/// How much a bright backdrop raises the glass tint, 0–1, in steps of 0.05:
/// nothing up to a mid-bright backdrop, all of it on a white one.
pub fn boost_for(luminance: f64) -> f64 {
    if !luminance.is_finite() {
        return 0.0;
    }
    let b = ((luminance - 0.6) / 0.35).clamp(0.0, 1.0);
    (b * 20.0).round() / 20.0
}

/// The average brightness (0–1) of sampled pixels; None with nothing sampled.
pub fn luminance(pixels: &[(u8, u8, u8)]) -> Option<f64> {
    if pixels.is_empty() {
        return None;
    }
    let sum: f64 = pixels.iter().map(|&(r, g, b)| 0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64).sum();
    Some(sum / pixels.len() as f64 / 255.0)
}

/// Looks again every 30 seconds: transparency switched off, battery saver
/// on. While the island is glass, the backdrop's brightness is sampled once a
/// minute, or as soon as the wallpaper changes, and the page raises its tint
/// only as far as a bright backdrop needs (round 3B item 1b).
pub fn spawn_watch(app: AppHandle) {
    apply(&app);
    if !cfg!(windows) {
        return;
    }
    let _ = std::thread::Builder::new().name("groovy-glass".into()).spawn(move || {
        let mut sampled: Option<std::time::Instant> = None;
        let mut wallpaper = String::new();
        let mut boost = 0.0;
        loop {
            if look() != Look::Solid {
                let now_wallpaper = os::wallpaper();
                let due = sampled.is_none_or(|at| at.elapsed() >= SAMPLE_EVERY) || now_wallpaper != wallpaper;
                if due {
                    wallpaper = now_wallpaper;
                    sampled = Some(std::time::Instant::now());
                    let pixels = app.get_webview_window(WINDOW_LABEL).map(|w| os::backdrop(&w)).unwrap_or_default();
                    let next = luminance(&pixels).map(boost_for).unwrap_or(boost);
                    if (next - boost).abs() >= 0.05 {
                        boost = next;
                        crate::log::line(format!("glass tint boost: {boost:.2}"));
                        let _ = app.emit_to(WINDOW_LABEL, "island-glass-boost", boost);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(30));
            apply(&app);
        }
    });
}

/// "solid", "blur" or "css" (the page's own glass).
#[tauri::command]
pub fn island_glass() -> &'static str {
    look().as_str()
}

#[cfg(windows)]
mod os {
    use tauri::WebviewWindow;
    use windows::core::{s, w, BOOL};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, CreateRoundRectRgn, DeleteObject, GetDC, GetPixel, GetSysColor, ReleaseDC, SetWindowRgn, COLOR_DESKTOP, HGDIOBJ};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};

    fn hwnd(win: &WebviewWindow) -> Option<HWND> {
        let raw = win.hwnd().ok()?.0 as isize;
        (raw != 0).then(|| HWND(raw as *mut _))
    }

    fn dword(key: windows::Win32::System::Registry::HKEY, path: windows::core::PCWSTR, name: windows::core::PCWSTR) -> Option<u32> {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let r = unsafe { RegGetValueW(key, path, name, RRF_RT_REG_DWORD, None, Some(&mut value as *mut u32 as *mut _), Some(&mut size)) };
        r.is_ok().then_some(value)
    }

    /// Settings → Personalisation → Colours → Transparency effects (on when unset).
    pub fn transparency_on() -> bool {
        dword(HKEY_CURRENT_USER, w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"), w!("EnableTransparency")) != Some(0)
    }

    pub fn battery_saver_on() -> bool {
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.SystemStatusFlag == 1 }
    }

    /// Windows 11 is build 22000 and later.
    fn windows_11() -> bool {
        let mut buf = [0u16; 16];
        let mut size = (buf.len() * 2) as u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
                w!("CurrentBuildNumber"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                Some(&mut size),
            )
        };
        if r.is_err() {
            return false;
        }
        let text = String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1).min(buf.len())]);
        text.trim().parse::<u32>().is_ok_and(|b| b >= 22000)
    }

    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        /// AABBGGRR: the tint the blur is mixed with.
        gradient: u32,
        animation: u32,
    }

    #[repr(C)]
    struct CompositionData {
        attribute: u32,
        data: *mut core::ffi::c_void,
        size: usize,
    }

    const WCA_ACCENT_POLICY: u32 = 19;
    const ACCENT_DISABLED: u32 = 0;
    const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
    const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;
    /// The page's own tint (rgba(18,20,24,…), core/glass.ts) does the rest:
    /// barely any here, so the blurred desktop shows through (round 3B item 1b).
    const TINT: u32 = 0x10_18_14_12;

    type SetWca = unsafe extern "system" fn(HWND, *mut CompositionData) -> BOOL;

    /// The accent blur on or off. False when Windows didn't take it (then the page draws its own glass).
    pub fn set_accent(win: &WebviewWindow, on: bool) -> bool {
        let Some(hwnd) = hwnd(win) else { return false };
        unsafe {
            let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return false };
            let Some(proc) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return false };
            let set: SetWca = std::mem::transmute(proc);
            let state = match (on, windows_11()) {
                (false, _) => ACCENT_DISABLED,
                (true, true) => ACCENT_ENABLE_ACRYLICBLURBEHIND,
                (true, false) => ACCENT_ENABLE_BLURBEHIND,
            };
            let mut policy = AccentPolicy { state, flags: 2, gradient: TINT, animation: 0 };
            let mut data = CompositionData {
                attribute: WCA_ACCENT_POLICY,
                data: &mut policy as *mut AccentPolicy as *mut _,
                size: std::mem::size_of::<AccentPolicy>(),
            };
            set(hwnd, &mut data).as_bool()
        }
    }

    /// The wallpaper's file and the desktop colour: a change means sample again.
    pub fn wallpaper() -> String {
        let mut buf = [0u16; 520];
        let ok = unsafe {
            SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr() as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
        }
        .is_ok();
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let path = if ok { String::from_utf16_lossy(&buf[..end]) } else { String::new() };
        format!("{path}|{}", unsafe { GetSysColor(COLOR_DESKTOP) })
    }

    /// The screen just outside the island's bottom-left rounded corner (inside
    /// the window, outside the region), and beside the window there.
    pub fn corner_pixels(win: &WebviewWindow) -> Option<((u8, u8, u8), Vec<(u8, u8, u8)>)> {
        let hwnd = hwnd(win)?;
        let mut r = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
        if r.right - r.left < 120 || r.bottom - r.top < 60 {
            return None;
        }
        unsafe {
            let dc = GetDC(None);
            if dc.is_invalid() {
                return None;
            }
            let read = |x: i32, y: i32| {
                let c = GetPixel(dc, x, y).0;
                (c != 0xFFFF_FFFF).then(|| ((c & 0xFF) as u8, ((c >> 8) & 0xFF) as u8, ((c >> 16) & 0xFF) as u8))
            };
            let corner = read(r.left + 1, r.bottom - 2);
            let beside: Vec<_> = [(r.left - 4, r.bottom - 2), (r.left + 1, r.bottom + 4), (r.right + 3, r.bottom - 2)]
                .into_iter()
                .filter_map(|(x, y)| read(x, y))
                .collect();
            ReleaseDC(None, dc);
            Some((corner?, beside))
        }
    }

    /// Pixels just around the island window: what shows through its glass.
    /// Twelve GetPixel calls, once a minute at most.
    pub fn backdrop(win: &WebviewWindow) -> Vec<(u8, u8, u8)> {
        let Some(hwnd) = hwnd(win) else { return Vec::new() };
        let mut r = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() || r.right <= r.left {
            return Vec::new();
        }
        let mut points: Vec<(i32, i32)> = (0..8).map(|i| (r.left + (r.right - r.left) * (2 * i + 1) / 16, r.bottom + 16)).collect();
        let mid = r.top + (r.bottom - r.top) / 2;
        points.extend([(r.left - 16, mid), (r.right + 16, mid), (r.left - 16, r.bottom), (r.right + 16, r.bottom)]);
        unsafe {
            let dc = GetDC(None);
            if dc.is_invalid() {
                return Vec::new();
            }
            let pixels = points
                .into_iter()
                .map(|(x, y)| GetPixel(dc, x, y).0)
                .filter(|&c| c != 0xFFFF_FFFF)
                .map(|c| ((c & 0xFF) as u8, ((c >> 8) & 0xFF) as u8, ((c >> 16) & 0xFF) as u8))
                .collect();
            ReleaseDC(None, dc);
            pixels
        }
    }

    pub fn set_region(win: &WebviewWindow, region: Option<(i32, i32, i32, i32, i32)>) {
        let Some(hwnd) = hwnd(win) else { return };
        unsafe {
            let rgn = match region {
                Some((l, t, r, b, d)) => CreateRoundRectRgn(l, t, r, b, d, d),
                None => CreateRectRgn(0, 0, 0, 0),
            };
            if rgn.is_invalid() {
                return;
            }
            // Windows owns the region once it is set; only a refused one is ours to free.
            if SetWindowRgn(hwnd, Some(rgn), true) == 0 {
                let _ = DeleteObject(HGDIOBJ(rgn.0));
            }
        }
    }
}

#[cfg(not(windows))]
mod os {
    use tauri::WebviewWindow;
    pub fn transparency_on() -> bool {
        false
    }
    pub fn battery_saver_on() -> bool {
        false
    }
    pub fn set_accent(_win: &WebviewWindow, _on: bool) -> bool {
        false
    }
    pub fn set_region(_win: &WebviewWindow, _region: Option<(i32, i32, i32, i32, i32)>) {}
    pub fn wallpaper() -> String {
        String::new()
    }
    pub fn backdrop(_win: &WebviewWindow) -> Vec<(u8, u8, u8)> {
        Vec::new()
    }
    pub fn corner_pixels(_win: &WebviewWindow) -> Option<((u8, u8, u8), Vec<(u8, u8, u8)>)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blur_that_fills_the_corners_is_caught() {
        let tint = (0x12, 0x14, 0x18);
        assert!(blur_leaks(tint, &[(255, 0, 255), (255, 0, 255)]));
        assert!(blur_leaks((0x14, 0x15, 0x1a), &[(40, 120, 200)]));
        // The desktop shows in the corner: fine.
        assert!(!blur_leaks((255, 0, 255), &[(255, 0, 255)]));
        // A dark wallpaper the colour of the tint: can't tell, keep the blur.
        assert!(!blur_leaks(tint, &[tint]));
        assert!(!blur_leaks(tint, &[]));
    }

    #[test]
    fn only_a_bright_backdrop_raises_the_tint() {
        assert_eq!(boost_for(0.0), 0.0);
        assert_eq!(boost_for(0.5), 0.0);
        assert_eq!(boost_for(1.0), 1.0);
        let mid = boost_for(0.78);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(boost_for(f64::NAN), 0.0);
        assert_eq!(luminance(&[]), None);
        assert!((luminance(&[(255, 255, 255)]).unwrap() - 1.0).abs() < 1e-9);
        let magenta = luminance(&[(255, 0, 255)]).unwrap();
        assert!(magenta < 0.3, "{magenta}");
    }

    #[test]
    fn glass_needs_the_choice_windows_transparency_and_no_battery_saver() {
        assert!(wanted("glass", true, false));
        assert!(!wanted("solid", true, false));
        assert!(!wanted("glass", false, false));
        assert!(!wanted("glass", true, true));
        // An unknown value (a newer version's) is glass, the default.
        assert!(wanted("frosted", true, false));
    }

    #[test]
    fn the_region_is_the_island_with_its_bottom_corners_rounded() {
        // A 640×200 island centred in the 720 panel, radius 22, at 150 %.
        let r = region((40.0, 0.0, 640.0, 200.0, 22.0), 1.5).unwrap();
        assert_eq!(r, (60, -66, 1021, 301, 66));
        // Top corners: the round rectangle starts a whole radius above the window.
        assert!(r.1 + r.4 / 2 <= 0);
    }

    #[test]
    fn the_blur_if_windows_takes_it_else_the_pages_own_glass() {
        assert_eq!(decide(false, false), Look::Solid);
        assert_eq!(decide(false, true), Look::Solid);
        assert_eq!(decide(true, true), Look::Blur);
        assert_eq!(decide(true, false), Look::Css);
        assert_eq!(Look::Css.as_str(), "css");
    }

    #[test]
    fn no_system_backdrop_is_ever_asked_for() {
        // Effect::Acrylic/Mica are the DWM system backdrop: it fills the whole
        // window rectangle, whatever the region (round 3B item 1).
        let src = include_str!("glass.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for banned in ["Effect::Acrylic", "Effect::Mica", "set_effects(", "DWMWA_SYSTEMBACKDROP_TYPE"] {
            assert!(!code.lines().any(|l| l.contains(banned) && !l.trim_start().starts_with("//")), "{banned}");
        }
    }

    #[test]
    fn a_hidden_island_has_an_empty_region() {
        assert_eq!(region((268.0, 0.0, 184.0, 0.0, 14.0), 1.0), None);
        assert_eq!(region((0.0, 0.0, 0.0, 30.0, 14.0), 1.0), None);
        assert_eq!(region((0.0, 0.0, 100.0, 30.0, 14.0), f64::NAN), None);
    }

    #[test]
    fn the_island_is_centred_on_the_window_it_is_in() {
        assert_eq!(centred_left(720.0, 640.0), 40.0);
        assert_eq!(centred_left(640.0, 640.0), 0.0);
        // A window not grown yet never puts the island left of its edge.
        assert_eq!(centred_left(184.0, 640.0), 0.0);
    }

    #[test]
    fn the_radius_never_exceeds_the_island() {
        let (_, top, _, _, d) = region((0.0, 0.0, 288.0, 10.0, 14.0), 1.0).unwrap();
        assert_eq!(d, 20);
        assert_eq!(top, -20);
    }
}
