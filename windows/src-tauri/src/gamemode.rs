//! Game Mode (round 3J item 3): while a full-screen game or presentation runs,
//! « Hey Groovy » stops listening, the scheduler's jobs rest and global
//! shortcuts are ignored except Mute and the island. No new polling: one
//! scheduler job, slow while the island is folded, idle when the switch is off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::scheduler::{self, Next};

pub const JOB: &str = "game-mode";
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// A full-screen game or presentation is running and Groovy is being quiet.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

fn enabled(app: &AppHandle) -> bool {
    app.try_state::<crate::Shared>().is_none_or(|s| s.settings.lock().unwrap().quiet_during_games)
}

fn next(app: &AppHandle) -> Next {
    if enabled(app) || active() { Next::Every(Duration::from_secs(5)) } else { Next::Idle }
}

fn run(app: &AppHandle) {
    let busy = enabled(app) && crate::platform::fullscreen_busy();
    if ACTIVE.swap(busy, Ordering::Relaxed) != busy {
        crate::log::line(format!("game mode: {}", if busy { "quiet" } else { "back" }));
        crate::okgroovy::sync(app);
        // The jobs that rested catch up now.
        if !busy {
            scheduler::recheck_pace(app);
        }
    }
}

pub fn start() {
    scheduler::add(JOB, Duration::from_secs(20), next, run);
}
