// Reading a streamed answer (round 4D item 3): server-sent event lines for
// the keyed cloud brains (Claude, the OpenAI-compatible services), and a
// throttle so the island gets the text so far about 15 times a second.

use std::time::{Duration, Instant};

/// How often the text so far goes out at most (the same pace as the local servers').
pub const EVERY: Duration = Duration::from_millis(1000 / 15);
/// A line longer than this is not an answer.
const MAX_LINE: usize = 1024 * 1024;

/// Hands each `data:` payload of an event stream to `on_data`, whole lines
/// only (a chunk may end in the middle of a line or a character).
/// `on_data` returns an error to stop.
pub async fn data_lines(mut reply: reqwest::Response, mut on_data: impl FnMut(&str) -> Result<(), String>, lost: impl Fn(String) -> String) -> Result<(), String> {
    let mut pending = Vec::<u8>::new();
    while let Some(chunk) = reply.chunk().await.map_err(|e| lost(e.to_string()))? {
        pending.extend_from_slice(&chunk);
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            if let Some(data) = data_of(&String::from_utf8_lossy(&line)) {
                on_data(&data)?;
            }
        }
        if pending.len() > MAX_LINE {
            return Err(crate::i18n::t("The server's answer is too large."));
        }
    }
    if let Some(data) = data_of(&String::from_utf8_lossy(&pending)) {
        on_data(&data)?;
    }
    Ok(())
}

/// The payload of one `data:` line; None for other lines and `[DONE]`.
pub fn data_of(line: &str) -> Option<String> {
    let payload = line.trim_end().strip_prefix("data:")?.trim_start();
    (!payload.is_empty() && payload != "[DONE]").then(|| payload.to_string())
}

/// Lets the text so far out at most every [`EVERY`]; the last state always goes out with `flush`.
pub struct Throttle {
    last: Option<Instant>,
}

impl Throttle {
    pub fn new() -> Self {
        Throttle { last: None }
    }
    pub fn due(&mut self) -> bool {
        if self.last.is_some_and(|t| t.elapsed() < EVERY) {
            return false;
        }
        self.last = Some(Instant::now());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_data_lines_are_read_and_done_ends_nothing() {
        assert_eq!(data_of("data: {\"a\":1}\r\n").as_deref(), Some("{\"a\":1}"));
        assert_eq!(data_of("event: ping"), None);
        assert_eq!(data_of("data: [DONE]"), None);
        assert_eq!(data_of(""), None);
    }

    #[test]
    fn the_throttle_lets_the_first_through_then_waits() {
        let mut t = Throttle::new();
        assert!(t.due());
        assert!(!t.due());
    }
}
