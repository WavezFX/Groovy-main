// When the models Groovy loads on demand give their memory back (round 3B
// item 10): Parakeet, Whisper, Kokoro and the local AI are loaded when they
// are needed and freed once unused for this long. Settings → Voice → Free
// memory after: 30 s, 2 min (the default), 5 min or never.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Seconds unused before a model is freed; 0 keeps it loaded.
static LIMIT_SECS: AtomicU64 = AtomicU64::new(120);

/// How often the loaded models are looked at.
pub const CHECK_EVERY: Duration = Duration::from_secs(10);

/// The setting's value in seconds ("30s", "2m", "5m" or "keep"); anything else is 2 min.
pub fn seconds(choice: &str) -> u64 {
    match choice {
        "30s" => 30,
        "5m" => 300,
        "keep" => 0,
        _ => 120,
    }
}

pub fn set(choice: &str) {
    LIMIT_SECS.store(seconds(choice), Ordering::Relaxed);
}

/// None when models stay loaded.
pub fn limit() -> Option<Duration> {
    match LIMIT_SECS.load(Ordering::Relaxed) {
        0 => None,
        s => Some(Duration::from_secs(s)),
    }
}

/// Unused for longer than the setting allows.
pub fn expired(last_used: Instant) -> bool {
    limit().is_some_and(|l| last_used.elapsed() > l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_reads_as_seconds_and_keep_means_never() {
        assert_eq!(seconds("30s"), 30);
        assert_eq!(seconds("2m"), 120);
        assert_eq!(seconds("5m"), 300);
        assert_eq!(seconds("keep"), 0);
        assert_eq!(seconds("whatever"), 120);
    }
}
