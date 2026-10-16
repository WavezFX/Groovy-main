// The glass island (FIXES_ROUND3 item 2c). Windows draws the frosted glass
// itself: Acrylic on Windows 11, the plain blur on Windows 10, behind a
// transparent webview. Groovy only tells it once, so the glass costs next to
// no CPU or memory. The window is the fixed 720×320 panel, so a window region
// clips it to the island's shape (square top, rounded bottom corners) and no
// frosted rectangle shows around it; the region follows the island as it
// opens and closes (set_island_rect).
//
// Solid (the black island) when the user chose it, when Windows' own
// "Transparency effects" switch is off, or while battery saver is on: the
// watcher looks again every 30 seconds.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::island::WINDOW_LABEL;

static ON: AtomicBool = AtomicBool::new(false);
/// The island's last shape: window-logical x, y, width, height and the
/// bottom corners' radius.
static SHAPE: Mutex<(f64, f64, f64, f64, f64)> = Mutex::new((0.0, 0.0, 0.0, 0.0, 0.0));

/// Glass is drawn when it is chosen and Windows allows it right now.
pub fn wanted(style: &str, transparency: bool, battery_saver: bool) -> bool {
    style != "solid" && transparency && !battery_saver
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

pub fn is_on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Turns the glass on or off as the settings and Windows say, and tells the
/// island page (which then draws a light tint instead of black).
pub fn apply(app: &AppHandle) {
    let style = app
        .try_state::<crate::Shared>()
        .map(|s| s.settings.lock().unwrap().island_style.clone())
        .unwrap_or_else(|| "glass".into());
    let on = cfg!(windows) && wanted(&style, os::transparency_on(), os::battery_saver_on());
    if ON.swap(on, Ordering::Relaxed) == on {
        return;
    }
    let Some(win) = app.get_webview_window(WINDOW_LABEL) else { return };
    os::set_effect(&win, on);
    reshape(&win);
    crate::log::line(format!("island style: {}", if on { "glass" } else { "solid" }));
    let _ = app.emit_to(WINDOW_LABEL, "island-glass", on);
}

/// The island moved or changed size: the glass follows it.
pub fn shape(app: &AppHandle, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    *SHAPE.lock().unwrap() = (x, y, w, h, radius);
    if !is_on() {
        return;
    }
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        reshape(&win);
    }
}

fn reshape(win: &tauri::WebviewWindow) {
    if is_on() {
        let scale = win.scale_factor().unwrap_or(1.0);
        os::set_region(win, region(*SHAPE.lock().unwrap(), scale));
    } else {
        os::clear_region(win);
    }
}

/// Looks again every 30 seconds: transparency switched off, battery saver on.
pub fn spawn_watch(app: AppHandle) {
    apply(&app);
    if !cfg!(windows) {
        return;
    }
    let _ = std::thread::Builder::new().name("groovy-glass".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        apply(&app);
    });
}

#[tauri::command]
pub fn island_glass() -> bool {
    is_on()
}

#[cfg(windows)]
mod os {
    use tauri::window::{Effect, EffectsBuilder};
    use tauri::WebviewWindow;
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ};
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

    pub fn set_effect(win: &WebviewWindow, on: bool) {
        if on {
            let effect = if windows_11() { Effect::Acrylic } else { Effect::Blur };
            // The page adds the dark tint (rgba(18,20,24,0.42)) that keeps white
            // text readable over bright wallpapers.
            let effects = EffectsBuilder::new().effect(effect).build();
            let _ = win.set_effects(effects);
        } else {
            let _ = win.set_effects(None);
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

    pub fn clear_region(win: &WebviewWindow) {
        let Some(hwnd) = hwnd(win) else { return };
        unsafe {
            SetWindowRgn(hwnd, None, true);
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
    pub fn set_effect(_win: &WebviewWindow, _on: bool) {}
    pub fn set_region(_win: &WebviewWindow, _region: Option<(i32, i32, i32, i32, i32)>) {}
    pub fn clear_region(_win: &WebviewWindow) {}
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_hidden_island_has_an_empty_region() {
        assert_eq!(region((268.0, 0.0, 184.0, 0.0, 14.0), 1.0), None);
        assert_eq!(region((0.0, 0.0, 0.0, 30.0, 14.0), 1.0), None);
        assert_eq!(region((0.0, 0.0, 100.0, 30.0, 14.0), f64::NAN), None);
    }

    #[test]
    fn the_radius_never_exceeds_the_island() {
        let (_, top, _, _, d) = region((0.0, 0.0, 288.0, 10.0, 14.0), 1.0).unwrap();
        assert_eq!(d, 20);
        assert_eq!(top, -20);
    }
}
