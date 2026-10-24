// What heavy part was loaded, and when (round 3D item 9: nothing loads until
// it's needed). Every model, engine or helper process that costs real memory
// notes itself here as it loads: the offline speech engines, Kokoro, the wake
// word spotter, the built-in local model's server and a request that makes
// Ollama load a model. Tests and CI read the counts to prove that in cloud
// mode none of the offline ones ever loaded; groovy.log keeps one line each.
//
// Also the cold start: from the process starting to the island first drawn.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;

/// The heavy parts, by the name the log and the report use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Part {
    /// Parakeet or Whisper on the PC.
    OfflineStt,
    Kokoro,
    WakeWord,
    /// The built-in model's llama-server (a helper process).
    LocalLlm,
    /// A chat sent to Ollama or LM Studio, which load their model for it.
    LocalServer,
}

impl Part {
    pub const ALL: [Part; 5] = [Part::OfflineStt, Part::Kokoro, Part::WakeWord, Part::LocalLlm, Part::LocalServer];

    fn index(self) -> usize {
        Part::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        match self {
            Part::OfflineStt => "offline-stt",
            Part::Kokoro => "kokoro",
            Part::WakeWord => "wake-word",
            Part::LocalLlm => "local-llm",
            Part::LocalServer => "local-server",
        }
    }
}

static COUNTS: [AtomicU64; 5] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// A heavy part was loaded now, for this reason.
pub fn note(part: Part, why: &str) {
    COUNTS[part.index()].fetch_add(1, Ordering::Relaxed);
    crate::log::line(format!("load: {} ({why})", part.name()));
}

pub fn count(part: Part) -> u64 {
    COUNTS[part.index()].load(Ordering::Relaxed)
}

// ── Cold start ────────────────────────────────────────────────────────────────

fn started() -> &'static Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    T.get_or_init(Instant::now)
}

/// Called first thing in main: the clock for the cold start.
pub fn process_started() {
    let _ = started();
}

static ISLAND_VISIBLE_MS: Mutex<Option<u64>> = Mutex::new(None);

/// The island page drew itself for the first time.
pub fn island_visible() {
    let mut v = ISLAND_VISIBLE_MS.lock().unwrap();
    if v.is_none() {
        let ms = started().elapsed().as_millis() as u64;
        *v = Some(ms);
        crate::log::line(format!("startup: island visible after {ms} ms"));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub counts: Vec<(&'static str, u64)>,
    pub island_visible_ms: Option<u64>,
}

pub fn report() -> Report {
    Report { counts: Part::ALL.iter().map(|p| (p.name(), count(*p))).collect(), island_visible_ms: *ISLAND_VISIBLE_MS.lock().unwrap() }
}

/// For CI and Settings → System: what has loaded since start-up.
#[tauri::command]
pub fn load_report() -> Report {
    report()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_are_counted_by_part() {
        let before = count(Part::Kokoro);
        note(Part::Kokoro, "test");
        assert_eq!(count(Part::Kokoro), before + 1);
        assert!(report().counts.iter().any(|(n, c)| *n == "kokoro" && *c >= 1));
        assert_eq!(Part::ALL.map(|p| p.index()), [0, 1, 2, 3, 4]);
    }
}
