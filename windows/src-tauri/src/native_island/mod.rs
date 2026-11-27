// The native island (prototype, off unless Groovy starts with `--native-island`).
//
// The always-on island drawn without WebView2: a layered Win32 window and a
// small software rasteriser (draw.rs), so an idle Groovy is groovy.exe alone.
// The web island is the "awake" island of island_sleep.rs: a click on the pill
// builds it (WebView2 loads only then), and when the page asks to sleep it is
// destroyed and the pill comes back. Events that would wake a sleeping island
// open it the same way, and are delivered once its page has drawn.
// See docs/plan/NATIVE_ISLAND.md.

pub mod draw;
#[cfg(windows)]
pub mod win;

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::time::Instant;

use tauri::AppHandle;

/// The switch on Groovy's command line.
pub const FLAG: &str = "--native-island";

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Asked for on the command line (Windows only; elsewhere the web island stays).
pub fn requested() -> bool {
    requested_in(std::env::args())
}

fn requested_in(mut args: impl Iterator<Item = String>) -> bool {
    cfg!(windows) && args.any(|a| a == FLAG)
}

/// The native pill stands in for the island (it started).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

#[cfg(windows)]
static OPENING: Mutex<Option<Instant>> = Mutex::new(None);

/// Makes and shows the pill. Main thread (Tauri's setup). False: use the web island.
#[cfg(windows)]
pub fn start(app: &AppHandle) -> bool {
    use tauri::{Listener, Manager};
    let (look, name, fraction) = {
        let s = app.state::<crate::Shared>().settings.lock().unwrap();
        (s.groovy_look.clone(), s.assistant.name.clone(), s.island_x)
    };
    let handle = app.clone();
    let made = win::create(win::Config {
        look,
        name,
        fraction,
        time: || {
            let t = crate::platform::local_time();
            (t.hour, t.minute, t.second)
        },
        on_input: Box::new(move |input| match input {
            win::Input::Click => crate::island_sleep::wake_up(&handle, "a click on the native island"),
            win::Input::RightClick => crate::show_hub(&handle, None),
        }),
    });
    if !made {
        crate::log::line("native island: the window could not be made, using the web island".to_string());
        return false;
    }
    ACTIVE.store(true, Ordering::SeqCst);
    win::show(true);
    let handle = app.clone();
    app.listen_any("settings-changed", move |_| {
        let (look, name) = {
            let s = handle.state::<crate::Shared>().settings.lock().unwrap();
            (s.groovy_look.clone(), s.assistant.name.clone())
        };
        let _ = handle.run_on_main_thread(move || win::restyle(&look, &name));
    });
    crate::log::line("native island: on (no WebView2 until it is clicked)".to_string());
    true
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle) -> bool {
    false
}

/// Builds the web island (island_sleep's wake). Its events are delivered once
/// its page reports the island's shape (`page_ready`).
#[cfg(windows)]
pub fn open_web(app: &AppHandle, since: Instant) {
    use tauri::Manager;
    *OPENING.lock().unwrap() = Some(since);
    if crate::island::window(app).is_some() {
        return page_ready(app);
    }
    let Some(conf) = app.config().app.windows.iter().find(|w| w.label == crate::island::WINDOW_LABEL).cloned() else {
        return;
    };
    let app = app.clone();
    // Built from its own thread, as the Control Centre is (create_hub_window).
    let _ = std::thread::Builder::new().name("groovy-web-island".into()).spawn(move || {
        let built = tauri::WebviewWindowBuilder::from_config(&app, &conf).and_then(|b| b.build());
        match built {
            Ok(win) => {
                let main = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let screen = main.state::<crate::Shared>().settings.lock().unwrap().screen.clone();
                    crate::prepare_island_window(&main, &win, &screen);
                    win::show(false);
                    crate::log::line(format!("native island: web island opened ({} native frames so far)", win::frames()));
                });
                // A page that never reports its shape still gets its events.
                std::thread::sleep(std::time::Duration::from_secs(5));
                page_ready(&app);
            }
            Err(e) => {
                crate::log::line(format!("native island: the web island could not be built: {e}"));
                *OPENING.lock().unwrap() = None;
                crate::island_sleep::woke_without_page(&app);
            }
        }
    });
}

/// The web island's page is drawing: what was kept for it goes now.
#[cfg(windows)]
pub fn page_ready(app: &AppHandle) {
    if let Some(since) = OPENING.lock().unwrap().take() {
        use tauri::Manager;
        app.state::<crate::Shared>().gate.set_active(true);
        crate::island_sleep::woke_native(app, since);
    }
}

#[cfg(not(windows))]
pub fn page_ready(_app: &AppHandle) {}

/// The web island asked to sleep: it is destroyed and the pill shows again.
#[cfg(windows)]
pub fn close_web(app: &AppHandle) {
    use tauri::Manager;
    app.state::<crate::Shared>().gate.set_active(false);
    if let Some(win) = crate::island::window(app) {
        let _ = win.destroy();
    }
    let _ = app.run_on_main_thread(|| win::show(true));
    crate::log::line("native island: web island closed, the pill is back".to_string());
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_the_flag_switches_it_on_and_only_on_windows() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>().into_iter();
        assert!(!super::requested_in(args(&["groovy.exe"])));
        assert!(!super::requested_in(args(&["groovy.exe", "--autostart"])));
        assert_eq!(super::requested_in(args(&["groovy.exe", "--native-island"])), cfg!(windows));
        assert!(!super::active(), "off unless started");
    }
}
