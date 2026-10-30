// What dictation, a recorded call and « OK Groovy » are doing, shown in the
// island (round 3: one island only, no window of its own at the bottom of the
// screen). Rust keeps the state and sends it whole to the island page on every
// change (`live-state`); messages with a button go there as notices
// (`island-notice`), which the island shows and times out itself.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::island::WINDOW_LABEL;

/// What the island shows of dictation, calls and « OK Groovy ».
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LiveState {
    /// "idle", "listening", "thinking", "locked", "copied", "failed", "pastedAgain".
    pub phase: String,
    /// The dictation key, as the page writes it ("right_ctrl"…).
    pub key: String,
    /// When the call being recorded started (ms since 1970), or none.
    pub note_started_at: Option<u64>,
    /// Dictation is on.
    pub enabled: bool,
    /// « OK Groovy » is talking with you.
    pub voice: Option<crate::okgroovy::VoiceView>,
}

/// A message in the island, with an optional button.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: u64,
    /// "mic", "lock", "clipboard", "download", "check", "error", "bolt".
    pub icon: String,
    pub tint: String,
    pub text: String,
    /// The button's label, if there is one.
    pub action: Option<String>,
    /// What the button does: "openSettings", "openNotes"…
    pub action_id: Option<String>,
    /// How long it stays when nobody looks at it.
    pub seconds: u64,
}

/// The notices still on show, so their buttons can be answered.
const KEEP: usize = 8;

pub struct Live {
    state: Mutex<LiveState>,
    notices: Mutex<Vec<Notice>>,
    seq: AtomicU64,
}

impl Live {
    pub fn state(&self) -> LiveState {
        self.state.lock().unwrap().clone()
    }
}

pub fn setup(app: &AppHandle) {
    app.manage(Arc::new(Live {
        state: Mutex::new(LiveState { phase: "idle".into(), ..Default::default() }),
        notices: Mutex::new(Vec::new()),
        seq: AtomicU64::new(1),
    }));
}

pub fn get(app: &AppHandle) -> Option<Arc<Live>> {
    app.try_state::<Arc<Live>>().map(|s| s.inner().clone())
}

/// A dictation is under way: the microphone is Flow's.
pub fn dictating(app: &AppHandle) -> bool {
    get(app).is_some_and(|p| is_dictating(&p.state.lock().unwrap().phase))
}

fn is_dictating(phase: &str) -> bool {
    matches!(phase, "listening" | "locked" | "thinking")
}

/// Changes what the island shows.
pub fn update(app: &AppHandle, change: impl FnOnce(&mut LiveState)) {
    let Some(live) = get(app) else { return };
    let state = {
        let mut s = live.state.lock().unwrap();
        let before = s.clone();
        change(&mut s);
        if *s == before {
            return;
        }
        s.clone()
    };
    let _ = app.emit_to(WINDOW_LABEL, "live-state", &state);
}

/// The nine voice levels, about 30 times a second while you speak.
pub fn bands(app: &AppHandle, levels: &[f32]) {
    let _ = app.emit_to(WINDOW_LABEL, "live-bands", levels);
}

/// A message in the island, with a button to act; the island takes it away
/// after `seconds`, unless the pointer is on it.
pub fn announce(app: &AppHandle, icon: &str, tint: &str, text: String, action: Option<(String, &str)>, seconds: u64) {
    let Some(live) = get(app) else { return };
    let id = live.seq.fetch_add(1, Ordering::Relaxed);
    let (label, action_id) = match action {
        Some((label, id)) => (Some(label), Some(id.to_string())),
        None => (None, None),
    };
    let n = Notice { id, icon: icon.into(), tint: tint.into(), text, action: label, action_id, seconds };
    {
        let mut list = live.notices.lock().unwrap();
        list.push(n.clone());
        let extra = list.len().saturating_sub(KEEP);
        list.drain(..extra);
    }
    crate::log::line(format!("island notice: {}", n.text));
    let _ = app.emit_to(WINDOW_LABEL, "island-notice", &n);
}

/// Takes a notice away (its ✕, its button, or its time ran out) and gives
/// it back, so its button can be done.
pub fn dismiss(app: &AppHandle, id: u64) -> Option<Notice> {
    let live = get(app)?;
    let mut list = live.notices.lock().unwrap();
    let at = list.iter().position(|n| n.id == id)?;
    Some(list.remove(at))
}

/// Notices still waiting, for an island page that (re)loads.
pub fn pending(app: &AppHandle) -> Vec<Notice> {
    get(app).map(|l| l.notices.lock().unwrap().clone()).unwrap_or_default()
}

/// Back to rest after a short message.
pub fn rest_after(app: &AppHandle, phase: &str, ms: u64) {
    let app = app.clone();
    let phase = phase.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        update(&app, |s| {
            if s.phase == phase {
                s.phase = "idle".into();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listening_locked_and_thinking_hold_the_microphone() {
        for p in ["listening", "locked", "thinking"] {
            assert!(is_dictating(p), "{p}");
        }
        for p in ["idle", "copied", "failed", "pastedAgain"] {
            assert!(!is_dictating(p), "{p}");
        }
    }
}
