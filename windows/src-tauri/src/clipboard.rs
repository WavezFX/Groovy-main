//! Clipboard history (round 3J item 4): opt-in, the last 20 text items, in
//! memory only. Windows tells us when the clipboard changes (a message-only
//! window with a format listener), so nothing is polled and nothing runs while
//! the switch is off. Items a password manager marks as private, and anything
//! over 10 KB, are never kept. History is never sent to an AI by itself.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;

#[cfg_attr(not(windows), allow(dead_code))]
const MAX_ITEMS: usize = 20;
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_BYTES: usize = 10 * 1024;

static HISTORY: Mutex<VecDeque<Entry>> = Mutex::new(VecDeque::new());

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub text: String,
    pub at: i64,
}

/// Puts a copied text at the front: no empty or oversized text, no repeats (it just moves up).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn push(history: &mut VecDeque<Entry>, text: &str, at: i64) -> bool {
    if text.trim().is_empty() || text.len() > MAX_BYTES {
        return false;
    }
    history.retain(|e| e.text != text);
    history.push_front(Entry { text: text.to_string(), at });
    history.truncate(MAX_ITEMS);
    true
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn record(text: &str) {
    push(&mut HISTORY.lock().unwrap(), text, crate::db::now());
}

pub fn clear() {
    HISTORY.lock().unwrap().clear();
}

/// The switch changed: the listener starts or stops, and turning it off forgets everything.
pub fn apply(enabled: bool) {
    if enabled {
        listener::start();
    } else {
        listener::stop();
        clear();
    }
}

#[tauri::command]
pub fn clipboard_history() -> Vec<Entry> {
    HISTORY.lock().unwrap().iter().cloned().collect()
}

#[tauri::command]
pub fn clipboard_clear() {
    clear();
}

/// Puts item `index` back on the clipboard, ready to paste.
#[tauri::command]
pub fn clipboard_copy(index: usize) -> Result<(), String> {
    let text = HISTORY.lock().unwrap().get(index).map(|e| e.text.clone()).ok_or("No such item.")?;
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).map_err(|e| e.to_string())
}

#[cfg(windows)]
mod listener {
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows::core::w;
    use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW, RemoveClipboardFormatListener,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage,
        RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_CLOSE,
        WM_DESTROY, WNDCLASSW,
    };

    static WINDOW: AtomicIsize = AtomicIsize::new(0);

    /// Password managers mark what must not reach any history.
    fn private() -> bool {
        unsafe {
            let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            if exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok() {
                return true;
            }
            let include = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
            let mut allowed = true;
            if include != 0 && IsClipboardFormatAvailable(include).is_ok() && OpenClipboard(None).is_ok() {
                if let Ok(h) = GetClipboardData(include) {
                    let g = HGLOBAL(h.0);
                    let p = GlobalLock(g) as *const u32;
                    if !p.is_null() {
                        allowed = *p != 0;
                        let _ = GlobalUnlock(g);
                    }
                }
                let _ = CloseClipboard();
            }
            !allowed
        }
    }

    fn changed() {
        if private() {
            return;
        }
        if let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            super::record(&text);
        }
    }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_CLIPBOARDUPDATE => {
                changed();
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = RemoveClipboardFormatListener(hwnd);
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    pub fn start() {
        if WINDOW.load(Ordering::SeqCst) != 0 {
            return;
        }
        // Claimed before the thread runs, so a second call can't start another.
        WINDOW.store(-1, Ordering::SeqCst);
        let spawned = std::thread::Builder::new().name("groovy-clipboard".into()).spawn(|| unsafe {
            let Ok(module) = GetModuleHandleW(None) else {
                WINDOW.store(0, Ordering::SeqCst);
                return;
            };
            let class = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: module.into(), lpszClassName: w!("GroovyClipboardWatch"), ..Default::default() };
            RegisterClassW(&class);
            let made = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class.lpszClassName,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(module.into()),
                None,
            );
            let Ok(hwnd) = made else {
                WINDOW.store(0, Ordering::SeqCst);
                return;
            };
            if AddClipboardFormatListener(hwnd).is_err() {
                let _ = DestroyWindow(hwnd);
                WINDOW.store(0, Ordering::SeqCst);
                return;
            }
            WINDOW.store(hwnd.0 as isize, Ordering::SeqCst);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            WINDOW.store(0, Ordering::SeqCst);
        });
        if spawned.is_err() {
            WINDOW.store(0, Ordering::SeqCst);
        }
    }

    pub fn stop() {
        let h = WINDOW.load(Ordering::SeqCst);
        if h > 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

// Linux and the rest: the history stays empty (the switch does nothing there yet).
#[cfg(not(windows))]
mod listener {
    pub fn start() {}
    pub fn stop() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_twenty_newest_first_without_repeats() {
        let mut h = VecDeque::new();
        for i in 0..25 {
            push(&mut h, &format!("item {i}"), i);
        }
        assert_eq!(h.len(), 20);
        assert_eq!(h[0].text, "item 24");
        push(&mut h, "item 10", 99);
        assert_eq!(h[0].text, "item 10");
        assert_eq!(h.iter().filter(|e| e.text == "item 10").count(), 1);
    }

    #[test]
    fn empty_and_large_text_is_never_kept() {
        let mut h = VecDeque::new();
        assert!(!push(&mut h, "   ", 1));
        assert!(!push(&mut h, &"x".repeat(MAX_BYTES + 1), 1));
        assert!(push(&mut h, &"x".repeat(MAX_BYTES), 1));
    }
}
