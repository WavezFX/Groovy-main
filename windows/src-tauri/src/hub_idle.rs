// Settings left open (round 4A item 3): idle must stay under 150 MB with the
// Settings window open too.
//
// - While the window doesn't have the focus its webview is asked for
//   WebView2's Low memory target, as the folded island is.
// - Left without the focus for ten minutes it is closed, and the next time
//   Settings is opened it comes back on the page it was on. Never while it is
//   busy: the page says so (a text field being typed in, something in
//   progress), and so does a model download.
//
// Nothing runs while Settings has the focus or is closed: one thread waits
// only while the window sits unfocused.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::HUB_LABEL;

/// Settings is closed after this long without the focus.
const AFTER: Duration = Duration::from_secs(600);

static BLURRED: Mutex<Option<Instant>> = Mutex::new(None);
static WAITING: AtomicBool = AtomicBool::new(false);
/// The page Settings was on when it was closed for being idle.
static PLACE: Mutex<Option<String>> = Mutex::new(None);

#[cfg(feature = "ci")]
fn after() -> Duration {
    std::env::var("GROOVY_HUB_IDLE_SECS").ok().and_then(|s| s.parse().ok()).map(Duration::from_secs).unwrap_or(AFTER)
}

#[cfg(not(feature = "ci"))]
fn after() -> Duration {
    AFTER
}

/// Whether an unfocused Settings window may be closed now.
pub fn may_close(unfocused_for: Option<Duration>, after: Duration, busy: bool, downloading: bool) -> bool {
    unfocused_for.is_some_and(|d| d >= after) && !busy && !downloading
}

/// Only a known section id is kept (the same rule as the window's address).
pub fn valid_page(page: &str) -> bool {
    !page.is_empty() && page.len() <= 32 && page.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The Settings window got or lost the focus.
pub fn focus_changed(app: &AppHandle, win: &WebviewWindow, focused: bool) {
    crate::resources::webview::set_low(win, !focused);
    if focused {
        *BLURRED.lock().unwrap() = None;
        return;
    }
    *BLURRED.lock().unwrap() = Some(Instant::now());
    if WAITING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("groovy-hub-idle".into()).spawn(move || {
        loop {
            let Some(since) = *BLURRED.lock().unwrap() else { break };
            let left = after().saturating_sub(since.elapsed());
            if left.is_zero() {
                // The page answers with `hub_idle_reply`: it knows whether it is busy.
                if app.get_webview_window(HUB_LABEL).is_some() {
                    let _ = app.emit_to(HUB_LABEL, "hub-idle", ());
                }
                *BLURRED.lock().unwrap() = None;
                break;
            }
            std::thread::sleep(left.min(Duration::from_secs(30)));
        }
        WAITING.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        WAITING.store(false, Ordering::SeqCst);
    }
}

/// Settings was closed (by the user or here): nothing waits any more.
pub fn closed() {
    *BLURRED.lock().unwrap() = None;
}

/// The Settings page's answer to `hub-idle`: where it is and whether it is busy.
#[tauri::command]
pub fn hub_idle_reply(app: AppHandle, page: String, busy: bool) {
    let Some(win) = app.get_webview_window(HUB_LABEL) else { return };
    let focused = win.is_focused().unwrap_or(false);
    // The wait is over by the time the page answers: only "busy" and the focus are left to check.
    if focused || !may_close(Some(after()), after(), busy, crate::models::downloading()) {
        if !focused {
            // Still unfocused but busy: ask again after another wait.
            focus_changed(&app, &win, false);
        }
        return;
    }
    if valid_page(&page) {
        *PLACE.lock().unwrap() = Some(page);
    }
    crate::log::line("control centre closed after sitting unused (it reopens on the same page)".to_string());
    closed();
    let _ = win.destroy();
}

/// The page to reopen on, once, after an idle close.
pub fn take_place() -> Option<String> {
    PLACE.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_is_closed_only_after_the_wait_and_never_while_busy() {
        let after = Duration::from_secs(600);
        assert!(!may_close(None, after, false, false), "focused, or already closed");
        assert!(!may_close(Some(Duration::from_secs(599)), after, false, false));
        assert!(may_close(Some(Duration::from_secs(600)), after, false, false));
        assert!(!may_close(Some(Duration::from_secs(900)), after, true, false), "something is being typed or is in progress");
        assert!(!may_close(Some(Duration::from_secs(900)), after, false, true), "a model is downloading");
    }

    #[test]
    fn only_a_section_id_is_remembered() {
        assert!(valid_page("brains"));
        assert!(valid_page("layout-tiles"));
        assert!(!valid_page(""));
        assert!(!valid_page("brains&x=1"));
        assert!(!valid_page("../index.html"));
        assert!(!valid_page(&"a".repeat(33)));
    }

    #[test]
    fn the_remembered_page_is_used_once() {
        *PLACE.lock().unwrap() = Some("voice".into());
        assert_eq!(take_place().as_deref(), Some("voice"));
        assert_eq!(take_place(), None);
    }
}
