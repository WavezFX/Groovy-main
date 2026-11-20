// Talk to Groovy (round 4 item 3): the backend of the island's live,
// back-and-forth conversation. The island has the microphone, the turns and
// the barge-in (island/voice.ts); here are what it can't do itself:
//
//   - basic commands mid-conversation, without an AI (okgroovy/basic.rs);
//   - « Hey Groovy » keeps quiet while a conversation has the microphone;
//   - a short summary of the conversation saved to chat history;
//   - "Instant voice": a real-time voice model (OpenAI Realtime over WebRTC,
//     or Gemini Live) when the user chose it and has that key. The stored key
//     never reaches the page: Rust asks for a short-lived one and, for OpenAI,
//     answers the WebRTC offer itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::i18n::t;

/// A conversation with Groovy has the microphone.
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn voice_call_active(app: tauri::AppHandle, on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
    // A live call is a conversation until it ends (round 3D item 10).
    crate::resources::state::fire(&app, if on { crate::resources::state::Event::CallStarted } else { crate::resources::state::Event::CallEnded });
    if !on {
        free_voice_after_call();
    }
}

/// Round 4F item 3: at hang-up the voice and speech models go at once, not
/// after their helpers' 20 s idle: both helpers are stopped (and any model
/// loaded in this process freed) within 2 s. A helper still answering holds
/// its lock, so this tries again every 200 ms; a new call stops it. The
/// keyword spotter stays: it is what listens for the wake word.
fn free_voice_after_call() {
    let _ = std::thread::Builder::new().name("groovy-call-free".into()).spawn(|| {
        let tries = until_freed(HANG_UP_TRIES, active, || {
            crate::speech::unload_idle_now();
            crate::speech::tts::free();
            !(crate::speech::helper::is_running(crate::speech::helper::Kind::Stt) || crate::speech::tts::is_loaded())
        });
        crate::log::line(match tries {
            Some(n) => format!("call ended: voice and speech models freed ({} ms)", n * HANG_UP_STEP.as_millis() as u32),
            None => "call ended: a model is still in use, its helper's idle limit frees it".to_string(),
        });
    });
}

const HANG_UP_STEP: std::time::Duration = std::time::Duration::from_millis(200);
/// 9 tries, 200 ms apart: done inside 2 s.
const HANG_UP_TRIES: u32 = 9;

/// Calls `free` until it says everything is gone (Some(tries before that)),
/// stopping early, with None, when `busy` turns true (a new call) or the tries run out.
fn until_freed(tries: u32, busy: impl Fn() -> bool, free: impl Fn() -> bool) -> Option<u32> {
    for n in 0..tries {
        if busy() {
            return None;
        }
        if free() {
            return Some(n);
        }
        std::thread::sleep(HANG_UP_STEP);
    }
    None
}

/// Music, volume, the time, timers, opening things: answered at once, with no AI.
#[tauri::command]
pub fn voice_basic(app: AppHandle, text: String) -> Option<String> {
    let basic = crate::okgroovy::basic::parse(&text)?;
    Some(crate::okgroovy::basic::run(&app, &basic))
}

/// "Hey Groovy, let's talk": starts a conversation instead of one answer.
pub fn is_talk(text: &str) -> bool {
    let t = crate::okgroovy::wake::normalize(text);
    [
        "let s talk", "lets talk", "let us talk", "let s chat", "lets chat", "talk to me", "let s have a chat",
        "start a conversation", "let s have a conversation", "can we talk",
    ]
    .iter()
    .any(|p| t == *p || t.starts_with(&format!("{p} ")))
}

/// The conversation, in a few sentences, saved to chat history (Settings →
/// Voice calls → Save a summary, on by default).
#[tauri::command]
pub async fn call_summary(turns: Vec<(String, String)>) -> Result<(), String> {
    if turns.is_empty() {
        return Ok(());
    }
    let transcript: String = turns
        .iter()
        .map(|(who, text)| format!("{}: {}\n", if who == "user" { "You" } else { "Groovy" }, text.trim()))
        .collect();
    let system = "Summarise this spoken conversation between the user and Groovy in two or three short sentences: what was asked, what was answered or done. Plain text, no preamble.";
    let summary = crate::brain_ai::complete(system, &transcript, 200, Duration::from_secs(30)).await?;
    let conn = crate::db::conn()?;
    crate::memory::record_turn(&conn, None, &t("Voice conversation"), summary.trim(), "call")?;
    crate::log::line("call: summary saved to chat history");
    Ok(())
}

// ── Instant voice ─────────────────────────────────────────────────────────────

const OPENAI_KEY: &str = "openai-api-key";
const GOOGLE_KEY: &str = "google-api-key";
const OPENAI_MODEL: &str = "gpt-realtime";
const GEMINI_MODEL: &str = "models/gemini-2.5-flash-native-audio-preview-09-2025";

/// Which real-time voices this PC has a key for.
#[tauri::command]
pub fn instant_voice_options() -> Vec<&'static str> {
    let mut v = Vec::new();
    if crate::secrets::present(OPENAI_KEY) {
        v.push("openai");
    }
    if crate::secrets::present(GOOGLE_KEY) {
        v.push("gemini");
    }
    v
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())
}

fn instructions(app: &AppHandle) -> String {
    use tauri::Manager;
    let name = app
        .try_state::<crate::Shared>()
        .map(|s| s.settings.lock().unwrap().assistant.name.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Groovy".into());
    format!("You are {name}, a friendly desktop companion, talking out loud with the user. Keep answers short and natural, one or two sentences unless asked for more.")
}

/// The OpenAI Realtime answer to the page's WebRTC offer: a short-lived key
/// is minted with the user's key, and used here for the call's SDP.
#[tauri::command]
pub async fn instant_openai_answer(app: AppHandle, offer: String) -> Result<String, String> {
    let key = crate::secrets::get(OPENAI_KEY).ok_or_else(|| t("Add an OpenAI key in Settings to use Instant voice."))?;
    let session = json!({
        "session": {
            "type": "realtime",
            "model": OPENAI_MODEL,
            "instructions": instructions(&app),
            "audio": {
                "input": { "transcription": { "model": "gpt-4o-mini-transcribe" }, "turn_detection": { "type": "server_vad" } },
                "output": { "voice": "marin" }
            }
        }
    });
    let client = client()?;
    let minted: Value = client
        .post("https://api.openai.com/v1/realtime/client_secrets")
        .bearer_auth(&key)
        .json(&session)
        .send()
        .await
        .map_err(|_| t("OpenAI could not be reached."))?
        .error_for_status()
        .map_err(|e| format!("OpenAI: {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let ephemeral = minted["value"].as_str().ok_or("OpenAI sent no session key")?.to_string();
    let answer = client
        .post("https://api.openai.com/v1/realtime/calls")
        .bearer_auth(&ephemeral)
        .header("Content-Type", "application/sdp")
        .body(offer)
        .send()
        .await
        .map_err(|_| t("OpenAI could not be reached."))?
        .error_for_status()
        .map_err(|e| format!("OpenAI: {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    crate::log::line("call: instant voice (OpenAI Realtime) connected");
    Ok(answer)
}

/// A one-use Gemini Live token (a minute to connect), and the setup the page sends first.
#[tauri::command]
pub async fn instant_gemini_token(app: AppHandle) -> Result<Value, String> {
    let key = crate::secrets::get(GOOGLE_KEY).ok_or_else(|| t("Add a Gemini key in Settings to use Instant voice."))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let rfc3339 = |secs: u64| {
        let d = time_of(secs);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", d.0, d.1, d.2, d.3, d.4, d.5)
    };
    let body = json!({ "uses": 1, "expireTime": rfc3339(now + 30 * 60), "newSessionExpireTime": rfc3339(now + 60) });
    let token: Value = client()?
        .post("https://generativelanguage.googleapis.com/v1alpha/auth_tokens")
        .header("x-goog-api-key", &key)
        .json(&body)
        .send()
        .await
        .map_err(|_| t("Gemini could not be reached."))?
        .error_for_status()
        .map_err(|e| format!("Gemini: {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let name = token["name"].as_str().ok_or("Gemini sent no token")?;
    Ok(json!({
        "token": name,
        "setup": {
            "setup": {
                "model": GEMINI_MODEL,
                "generationConfig": { "responseModalities": ["AUDIO"] },
                "systemInstruction": { "parts": [{ "text": instructions(&app) }] },
                "inputAudioTranscription": {},
                "outputAudioTranscription": {}
            }
        }
    }))
}

/// UTC (year, month, day, hour, minute, second) of a Unix time.
fn time_of(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day, (rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32)
}

#[cfg(test)]
mod hang_up_tests {
    use std::cell::Cell;

    #[test]
    fn models_go_within_two_seconds_of_hang_up_and_a_new_call_stops_the_freeing() {
        // A fake helper that lets go on the third try (it was answering).
        let calls = Cell::new(0);
        let t0 = std::time::Instant::now();
        let got = super::until_freed(super::HANG_UP_TRIES, || false, || {
            calls.set(calls.get() + 1);
            calls.get() >= 3
        });
        assert_eq!(got, Some(2));
        assert!(t0.elapsed() < std::time::Duration::from_secs(2));
        // Never gone: gives up inside 2 s (the helper's own idle limit takes over).
        let t0 = std::time::Instant::now();
        assert_eq!(super::until_freed(super::HANG_UP_TRIES, || false, || false), None);
        assert!(t0.elapsed() < std::time::Duration::from_secs(2));
        // A new call started: nothing is freed.
        let freed = Cell::new(false);
        assert_eq!(super::until_freed(super::HANG_UP_TRIES, || true, || { freed.set(true); true }), None);
        assert!(!freed.get());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lets_talk_starts_a_conversation_and_little_else_does() {
        for yes in ["Let's talk.", "let's talk about my day", "Talk to me", "Can we talk?", "lets chat"] {
            assert!(is_talk(yes), "{yes}");
        }
        for no in ["talk to Sam on Teams", "what time is it", "let's go", "play some talk radio"] {
            assert!(!is_talk(no), "{no}");
        }
    }

    #[test]
    fn unix_times_become_utc_dates() {
        assert_eq!(time_of(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(time_of(1_792_176_622), (2026, 10, 16, 18, 50, 22));
        assert_eq!(time_of(951_782_400), (2000, 2, 29, 0, 0, 0));
    }
}
