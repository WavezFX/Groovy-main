// CI's realistic test (round 3B item 10). With GROOVY_WORK_TEST set to a
// folder of 16 kHz recordings (src-tauri/tests/audio), Groovy downloads the
// speech models, the wake word spotter, llama.cpp and the smallest local
// model if they aren't there, then does real work with them: one dictation,
// one « OK Groovy » command and one local AI reply. Then it waits for the
// idle unload and writes work-test.json, which scripts/work-test.ps1 reads
// before it measures Groovy's idle memory and CPU. Nothing here runs
// without that variable.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::speech;

const MODEL: &str = "qwen2.5-0.5b";
/// The idle unload is waited for at most this long.
const UNLOAD_WAIT: Duration = Duration::from_secs(240);

pub fn start(app: &AppHandle) {
    let Ok(dir) = std::env::var("GROOVY_WORK_TEST") else { return };
    // The shortest idle setting a user can choose, so CI doesn't wait 2 minutes.
    crate::idle::set("30s");
    let app = app.clone();
    let _ = std::thread::Builder::new().name("groovy-work-test".into()).spawn(move || {
        let mut steps: Vec<Value> = Vec::new();
        let mut step = |name: &str, started: Instant, result: Result<String, String>| {
            let ok = result.is_ok();
            let detail = result.unwrap_or_else(|e| e);
            crate::log::line(format!("work test: {name}: {} ({} ms) {detail}", if ok { "ok" } else { "FAILED" }, started.elapsed().as_millis()));
            steps.push(json!({ "name": name, "ok": ok, "ms": started.elapsed().as_millis() as u64, "detail": detail }));
        };
        let dir = PathBuf::from(dir);
        let t = Instant::now();
        step("models downloaded", t, tauri::async_runtime::block_on(models(&app)));
        // Round 3D item 9: with every model on disk, none was loaded "just in case".
        let t = Instant::now();
        step("nothing loaded at start-up", t, nothing_loaded());
        let t = Instant::now();
        step("dictation", t, dictation(&app, &dir));
        let t = Instant::now();
        step("OK Groovy command", t, command(&app, &dir));
        let t = Instant::now();
        step("back to Standby after the follow-up window", t, back_to_standby());
        let t = Instant::now();
        step("local AI reply", t, tauri::async_runtime::block_on(local_reply(&app)));
        let t = Instant::now();
        step("models freed when idle", t, wait_for_unload());
        let ok = steps.iter().all(|s| s["ok"] == true);
        let report = json!({ "done": true, "ok": ok, "steps": steps });
        let path = crate::platform::local_dir().join("work-test.json");
        if let Err(e) = std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap_or_default()) {
            crate::log::line(format!("work test: could not write the report: {e}"));
        }
    });
}

/// No offline engine, local model or helper was loaded before any work asked for one.
fn nothing_loaded() -> Result<String, String> {
    use crate::loads::{count, Part};
    let loaded: Vec<&str> = [Part::OfflineStt, Part::Kokoro, Part::LocalLlm, Part::LocalServer].into_iter().filter(|p| count(*p) > 0).map(|p| p.name()).collect();
    let visible = crate::loads::report().island_visible_ms.map(|ms| format!("island visible after {ms} ms")).unwrap_or_default();
    if loaded.is_empty() {
        Ok(visible)
    } else {
        Err(format!("loaded before any work: {}", loaded.join(", ")))
    }
}

async fn models(app: &AppHandle) -> Result<String, String> {
    if !(speech::PARAKEET.is_present() && speech::kws::is_present()) {
        speech::download_all(app).await?;
    }
    if crate::models::server_exe().is_none() {
        crate::models::models_install_runtime(app.clone(), "cpu".into()).await?;
    }
    let file = crate::models::models_download(app.clone(), MODEL.into()).await?;
    Ok(format!("Parakeet, the spotter, llama.cpp and {file}"))
}

fn wav(dir: &Path, name: &str) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    let data = bytes.windows(4).position(|w| w == b"data").ok_or("no data chunk")? + 8;
    Ok(bytes[data.min(bytes.len())..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect())
}

fn dictation(app: &AppHandle, dir: &Path) -> Result<String, String> {
    let samples = wav(dir, "dictation.wav")?;
    speech::get_ready(app);
    let text = speech::transcribe(&samples).ok_or("Parakeet did not load")?.text;
    if text.to_lowercase().contains("report") {
        Ok(text)
    } else {
        Err(format!("heard “{text}”"))
    }
}

fn command(app: &AppHandle, dir: &Path) -> Result<String, String> {
    let spotter = speech::kws::Spotter::load().ok_or("the spotter did not load")?;
    if spotter.heard(&wav(dir, "not-wake.wav")?) {
        return Err("“OK, let's go to the store” was taken for « OK Groovy »".into());
    }
    let samples = wav(dir, "ok-groovy-time.wav")?;
    let t = Instant::now();
    if !spotter.heard(&samples) {
        return Err("the spotter did not hear « OK Groovy »".into());
    }
    let wake_ms = t.elapsed().as_millis();
    // Round 3D item 10: the wake word starts a conversation.
    crate::runtime_state::fire(app, crate::runtime_state::Event::Wake);
    let t = Instant::now();
    let heard = speech::transcribe(&samples).ok_or("Parakeet did not load")?.text;
    let words_ms = t.elapsed().as_millis();
    let rest = crate::okgroovy::wake::after_wake(&heard).unwrap_or_else(|| crate::okgroovy::wake::drop_wake_words(&heard));
    let basic = crate::okgroovy::basic::parse(&rest).ok_or_else(|| format!("no command in “{heard}”"))?;
    let t = Instant::now();
    let answer = crate::okgroovy::basic::run(app, &basic);
    let answer_ms = t.elapsed().as_millis();
    crate::runtime_state::fire(app, crate::runtime_state::Event::ReplyDone);
    Ok(format!("“{heard}” → {answer} (wake heard in {wake_ms} ms, words on this PC in {words_ms} ms, answer in {answer_ms} ms)"))
}

/// The conversation ends by itself after the follow-up window, and what it held goes.
fn back_to_standby() -> Result<String, String> {
    use crate::runtime_state::{state, RunState, FOLLOW_UP};
    let t = Instant::now();
    while state() == RunState::Conversation && t.elapsed() < FOLLOW_UP + Duration::from_secs(4) {
        std::thread::sleep(Duration::from_millis(200));
    }
    let s = state();
    if s == RunState::Conversation {
        return Err(format!("still in a conversation after {} s", t.elapsed().as_secs()));
    }
    let clients = crate::net::shared_clients();
    let mem = crate::system::read_now().groovy_memory / (1024 * 1024);
    if clients > 0 {
        return Err(format!("{s:?} but {clients} warmed connection(s) kept"));
    }
    Ok(format!("{s:?} after {:.1} s, connections released, groovy.exe {mem} MB", t.elapsed().as_secs_f64()))
}

async fn local_reply(app: &AppHandle) -> Result<String, String> {
    let file = crate::models::CATALOG.iter().find(|m| m.id == MODEL).ok_or("not in the catalogue")?.file;
    let prefs = crate::assistant::AssistantPrefs { ai_mode: "local".into(), local_model: file.into(), ..Default::default() };
    let url = crate::models::ensure_running(&prefs, Some(app)).await?;
    let body = json!({
        "model": file,
        "messages": [{ "role": "user", "content": "Say hello in five words." }],
        "max_tokens": 24,
    });
    let reply: Value = reqwest::Client::new()
        .post(format!("{url}/v1/chat/completions"))
        .json(&body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    crate::models::touch();
    let text = reply["choices"][0]["message"]["content"].as_str().unwrap_or_default().trim().to_string();
    if text.is_empty() {
        Err(format!("no answer: {reply}"))
    } else {
        Ok(text)
    }
}

fn wait_for_unload() -> Result<String, String> {
    let started = Instant::now();
    while started.elapsed() < UNLOAD_WAIT {
        if !speech::is_loaded() && crate::models::running_url().is_none() {
            return Ok(format!("Parakeet and llama-server gone after {} s", started.elapsed().as_secs()));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(format!("still loaded: Parakeet {}, llama-server {}", speech::is_loaded(), crate::models::running_url().is_some()))
}
