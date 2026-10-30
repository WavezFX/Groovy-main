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

/// Round 3D item 10's WebView2 audit: features Groovy never uses are off in
/// every window — the password and form autofill prompts, the status bar for
/// link targets, and (in release builds) the developer tools.
#[cfg(windows)]
pub fn trim(win: &WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use wv2_core::Interface;
    let label = win.label().to_string();
    let _ = win.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let Ok(settings) = core.Settings() else { return };
        let _ = settings.SetIsStatusBarEnabled(false);
        if cfg!(not(debug_assertions)) {
            let _ = settings.SetAreDevToolsEnabled(false);
        }
        match settings.cast::<ICoreWebView2Settings4>() {
            Ok(s4) => {
                let _ = s4.SetIsPasswordAutosaveEnabled(false);
                let _ = s4.SetIsGeneralAutofillEnabled(false);
            }
            Err(err) => crate::log::line(format!("{label}: autofill left as it is: {err}")),
        }
    });
}

#[cfg(not(windows))]
pub fn trim(_win: &WebviewWindow) {}
