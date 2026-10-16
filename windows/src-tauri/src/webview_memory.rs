// Tells WebView2 how much memory a webview may keep (round 3 §9: under 300 MB).
//
// A hidden webview (the island folded into its wake strip, the desktop Groovy
// back in the island) is asked for the Low memory target
// (ICoreWebView2_19::MemoryUsageTargetLevel): its renderer gives back caches
// and decoded images until it is shown again, when it is set back to Normal.
// Older WebView2 runtimes without the interface simply keep their memory.

use tauri::WebviewWindow;

#[cfg(windows)]
pub fn set_low(win: &WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use wv2_core::Interface;
    let label = win.label().to_string();
    let _ = win.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let Ok(core19) = core.cast::<ICoreWebView2_19>() else { return };
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        if let Err(err) = core19.SetMemoryUsageTargetLevel(level) {
            crate::log::line(format!("{label}: memory target not set: {err}"));
        }
    });
}

#[cfg(not(windows))]
pub fn set_low(_win: &WebviewWindow, _low: bool) {}
