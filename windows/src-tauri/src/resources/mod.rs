// One resource manager (round 3F item 3): what is loaded and when it goes,
// the 400 MB ceiling, WebView2 memory and the operating states, with one
// thread for all of it.
//
// - `idle`: how long an unused model stays loaded (Settings → Voice).
// - `loads`: what heavy part loaded, and when; the cold start.
// - `ceiling`: the 400 MB ceiling, making room for a model, giving back.
// - `webview`: WebView2's memory target and the trimmed settings.
// - `state`: Standby, Conversation and Offline.
//
// Each engine that loads on demand (Parakeet, Whisper, the wake-word spotter,
// Kokoro, the built-in model's server) registers how to free itself once
// idle; the one "groovy-resources" thread reaps them all every
// `idle::CHECK_EVERY` and reads the ceiling every few seconds.

pub mod ceiling;
pub mod idle;
pub mod loads;
pub mod state;
pub mod webview;

use std::sync::Mutex;
use std::time::Instant;

use tauri::AppHandle;

/// An engine loaded on demand: `reap` frees it if it has been idle for the
/// setting (and leaves it alone otherwise, or while it is busy).
struct Engine {
    name: &'static str,
    reap: fn(),
}

static ENGINES: Mutex<Vec<Engine>> = Mutex::new(Vec::new());

/// `name` frees itself through `reap` once idle. Registering twice is a no-op.
pub fn register(name: &'static str, reap: fn()) {
    let mut engines = ENGINES.lock().unwrap();
    if !engines.iter().any(|e| e.name == name) {
        engines.push(Engine { name, reap });
    }
}

fn reap_all() {
    let reapers: Vec<fn()> = ENGINES.lock().unwrap().iter().map(|e| e.reap).collect();
    for reap in reapers {
        reap();
    }
}

/// Starts the one thread.
pub fn start(app: &AppHandle) {
    ceiling::init(app);
    let app = app.clone();
    let _ = std::thread::Builder::new().name("groovy-resources".into()).spawn(move || {
        let mut watch = ceiling::Watch::default();
        let mut reaped = Instant::now();
        loop {
            std::thread::sleep(ceiling::EVERY);
            ceiling::check(&app, &mut watch);
            if reaped.elapsed() >= idle::CHECK_EVERY {
                reaped = Instant::now();
                reap_all();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static REAPED: AtomicU32 = AtomicU32::new(0);

    fn count() {
        REAPED.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn each_engine_registers_once_and_is_reaped_by_the_one_pass() {
        register("test-engine", count);
        register("test-engine", count);
        assert_eq!(ENGINES.lock().unwrap().iter().filter(|e| e.name == "test-engine").count(), 1);
        let before = REAPED.load(Ordering::Relaxed);
        reap_all();
        assert_eq!(REAPED.load(Ordering::Relaxed), before + 1);
    }
}
