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
// idle. One job on the scheduler (round 3F item 4) reads the ceiling and
// reaps them: every 3 s while a model is loaded or a panel window is open,
// otherwise every 30 s.

pub mod ceiling;
pub mod idle;
pub mod loads;
pub mod state;
pub mod webview;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::scheduler::Next;

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
    drop(engines);
    loaded();
}

fn reap_all() {
    let reapers: Vec<fn()> = ENGINES.lock().unwrap().iter().map(|e| e.reap).collect();
    for reap in reapers {
        reap();
    }
}

/// With nothing loaded and only the island open, the ceiling is far away.
const AT_REST: Duration = Duration::from_secs(30);

/// A model is loaded or a panel window is open: the total can move fast, so
/// it is read every 3 s whatever the island does; otherwise every 30 s.
fn next(app: &AppHandle) -> Next {
    let panel = ceiling::PANELS.iter().any(|l| app.get_webview_window(l).is_some());
    if ceiling::models_loaded() || panel {
        Next::In(ceiling::EVERY)
    } else {
        Next::Every(AT_REST)
    }
}

static WATCH: Mutex<Option<(ceiling::Watch, Instant)>> = Mutex::new(None);

fn tick(app: &AppHandle) {
    let mut w = WATCH.lock().unwrap();
    let (watch, reaped) = w.get_or_insert_with(|| (ceiling::Watch::default(), Instant::now()));
    ceiling::check(app, watch);
    if reaped.elapsed() >= idle::CHECK_EVERY {
        *reaped = Instant::now();
        reap_all();
    }
}

/// Puts the watch on the scheduler.
pub fn start(app: &AppHandle) {
    ceiling::init(app);
    crate::scheduler::add("resources", ceiling::EVERY, next, tick);
}

/// A model just loaded: the watch goes to its quick pace now.
pub fn loaded() {
    crate::scheduler::poke("resources");
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
