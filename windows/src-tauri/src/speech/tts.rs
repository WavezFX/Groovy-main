// Groovy's voice (FIXES_ROUND3B item 8c). Fish Audio first whenever it is set
// up (the user's own key, in the OS store, and a voice id), then Kokoro, the
// offline voice (sherpa-onnx, downloaded only when the user asks), then
// whatever answered before (ElevenLabs or Windows' own voices, in the page).
//
// A refused or failed Fish Audio request falls through to Kokoro for that
// reply; no credit or a rate limit also rests Fish Audio for ten minutes, as
// brain_ai.rs does with the brains, and quick settings shows why. Nothing is
// ever silent.
//
// The key goes only in the Authorization header: never in a log line, an
// error message or the settings file.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig};
use tauri::{AppHandle, Emitter};

use super::ModelSet;
use crate::i18n::{t, tf};

pub const FISH_KEY: &str = "fish-audio-api-key";
pub const FISH_URL: &str = "https://api.fish.audio/v1/tts";
pub const FISH_DEFAULT_MODEL: &str = "s2-pro";
pub const FISH_MODELS: [&str; 3] = ["s2-pro", "s1", "speech-1.6"];
/// How long Fish Audio rests after no credit or a rate limit.
const FISH_REST: Duration = Duration::from_secs(600);
/// A rough price for the cost line: Fish Audio bills by text length, about
/// US$15 per million characters, about £11.50.
const GBP_PER_MILLION_CHARS: f64 = 11.5;

/// Kokoro 82M v1.0, int8 (Apache-2.0), with espeak-ng's data (about 173 MB).
pub static KOKORO: ModelSet = ModelSet { id: "kokoro-int8-v1_0", files: super::kokoro_files::FILES };

/// Something to play: an MP3 from Fish Audio or a WAV from Kokoro.
pub struct Audio {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub provider: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FishError {
    /// The key was refused.
    Key,
    NoCredit,
    Limit,
    /// Another refusal, with its HTTP status (a model it won't serve, a bad voice id…).
    Refused(u16),
    Offline,
}

impl FishError {
    pub fn from_status(status: u16) -> FishError {
        match status {
            401 | 403 => FishError::Key,
            402 => FishError::NoCredit,
            429 => FishError::Limit,
            s => FishError::Refused(s),
        }
    }

    /// What quick settings and the Voice page say.
    pub fn note(&self) -> String {
        match self {
            FishError::Key => t("Fish Audio refused the key: Groovy uses its other voice for now."),
            FishError::NoCredit => t("Fish Audio has no credit left: Groovy uses its other voice for now."),
            FishError::Limit => t("Fish Audio's rate limit was reached: Groovy uses its other voice for now."),
            FishError::Refused(s) => tf("Fish Audio refused the request (HTTP {status}): check the voice ID. Groovy uses its other voice for now.", &[("status", &s.to_string())]),
            FishError::Offline => t("Fish Audio can't be reached: Groovy uses its other voice for now."),
        }
    }

    fn rests(&self) -> bool {
        matches!(self, FishError::NoCredit | FishError::Limit)
    }
}

/// How long one Fish Audio answer may take.
pub const FISH_TIMEOUT: Duration = Duration::from_secs(30);

/// One Fish Audio request: the MP3, read as it streams in.
pub async fn fish_request(url: &str, key: &str, voice: &str, model: &str, text: &str, speed: f32) -> Result<Vec<u8>, FishError> {
    // Shared, so the connection the wake word warmed is the one used (round 3D item 10).
    let parsed = reqwest::Url::parse(url).map_err(|_| FishError::Offline)?;
    let client = crate::net::client(&parsed, FISH_TIMEOUT).map_err(|_| FishError::Offline)?;
    let mut response = client
        .post(url)
        .bearer_auth(key)
        .header("model", model)
        .json(&json!({ "text": text, "reference_id": voice, "format": "mp3", "latency": "balanced", "prosody": { "speed": speed } }))
        .send()
        .await
        .map_err(|_| FishError::Offline)?;
    if !response.status().is_success() {
        return Err(FishError::from_status(response.status().as_u16()));
    }
    let mut out = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| FishError::Offline)? {
        out.extend_from_slice(&chunk);
    }
    if out.is_empty() {
        return Err(FishError::Offline);
    }
    Ok(out)
}

struct FishState {
    rest_until: Option<Instant>,
    note: Option<String>,
}

fn fish_state() -> &'static Mutex<FishState> {
    static S: OnceLock<Mutex<FishState>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(FishState { rest_until: None, note: None }))
}

/// Why Fish Audio isn't speaking right now, for quick settings.
pub fn fish_note() -> Option<String> {
    fish_state().lock().unwrap().note.clone()
}

struct Prefs {
    voice: String,
    model: String,
    gender: String,
    /// Who speaks first (Settings → Speaking): "fish", "kokoro" or "system".
    pick: String,
    /// 0.5–2.0, every voice.
    speed: f32,
}

fn prefs(app: &AppHandle) -> Prefs {
    crate::flow::get(app)
        .map(|f| {
            let p = &f.store.lock().unwrap().prefs;
            Prefs { voice: p.fish_voice.trim().to_string(), model: p.fish_model.clone(), gender: p.voice_gender.clone(), pick: p.voice_pick.clone(), speed: clamp_speed(p.voice_speed) }
        })
        .unwrap_or(Prefs { voice: String::new(), model: FISH_DEFAULT_MODEL.into(), gender: "female".into(), pick: "fish".into(), speed: 1.0 })
}

/// A speed the voices accept: 0.5–2.0, 1.0 for anything else.
pub fn clamp_speed(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v.clamp(0.5, 2.0) } else { 1.0 }
}

fn fish_model(model: &str) -> &str {
    if FISH_MODELS.contains(&model) {
        model
    } else {
        FISH_DEFAULT_MODEL
    }
}

/// Fish Audio with this key and voice: the chosen model, then the default
/// model if the chosen one is refused.
fn fish(key: &str, voice: &str, model: &str, text: &str, speed: f32) -> Result<Vec<u8>, FishError> {
    let model = fish_model(model);
    let first = tauri::async_runtime::block_on(fish_request(FISH_URL, key, voice, model, text, speed));
    match first {
        Err(FishError::Refused(_)) if model != FISH_DEFAULT_MODEL => {
            crate::log::line(format!("voice: Fish Audio refused {model}, trying {FISH_DEFAULT_MODEL}"));
            tauri::async_runtime::block_on(fish_request(FISH_URL, key, voice, FISH_DEFAULT_MODEL, text, speed))
        }
        other => other,
    }
}

/// Groovy's voice for `text`, best first; None: the page's own voice says it.
pub fn synthesize(app: &AppHandle, text: &str) -> Option<Audio> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let p = prefs(app);
    // The system voice picked: the page says it with Windows' own voice.
    if p.pick == "system" {
        return None;
    }
    let resting = fish_state().lock().unwrap().rest_until.is_some_and(|t| Instant::now() < t);
    let fish_first = p.pick != "kokoro";
    if let (Some(key), false, false, true) = (crate::secrets::get(FISH_KEY), p.voice.is_empty(), resting, fish_first) {
        match fish(&key, &p.voice, &p.model, text, p.speed) {
            Ok(bytes) => {
                record_usage(text.chars().count() as u64);
                fish_state().lock().unwrap().note = None;
                return Some(Audio { mime: "audio/mpeg", bytes, provider: "fish" });
            }
            Err(err) => {
                crate::log::line(format!("voice: Fish Audio failed ({err:?}), Kokoro answers"));
                let mut s = fish_state().lock().unwrap();
                s.note = Some(err.note());
                if err.rests() {
                    s.rest_until = Some(Instant::now() + FISH_REST);
                }
                let _ = app.emit("tts-status-changed", ());
            }
        }
    }
    kokoro_at(text, &p.gender, p.speed).map(|bytes| Audio { mime: "audio/wav", bytes, provider: "kokoro" })
}

// ── Kokoro ────────────────────────────────────────────────────────────────────

struct Loaded {
    tts: Arc<OfflineTts>,
    used: Instant,
}

fn engine() -> &'static Mutex<Option<Loaded>> {
    static E: OnceLock<Mutex<Option<Loaded>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(None))
}

/// The speaker: af_heart or am_michael, Kokoro v1.0's warmest English voices.
pub fn kokoro_speaker(gender: &str) -> i32 {
    if gender == "male" {
        16
    } else {
        3
    }
}

/// onnxruntime without weight prepacking, which keeps a second, repacked copy
/// of Kokoro's weights (about 14 MB) for no speed it can measure here. Passed
/// as sherpa-onnx's "cpu:<file>" provider; sherpa-onnx exits when the file is
/// missing, so it is written first and left out if it can't be.
fn low_memory_provider() -> Option<String> {
    let dir = crate::settings::local_dir();
    let file = dir.join("onnxruntime-kokoro.cfg");
    crate::platform::ensure_private_dir(&dir).ok()?;
    std::fs::write(&file, "SessionConfig.session.disable_prepacking=1\n").ok()?;
    file.is_file().then(|| format!("cpu:{}", file.display()))
}

fn load() -> Option<Arc<OfflineTts>> {
    let mut e = engine().lock().unwrap();
    if let Some(l) = e.as_mut() {
        l.used = Instant::now();
        return Some(l.tts.clone());
    }
    if !KOKORO.is_present() {
        return None;
    }
    // Speech recognition and the voice together only when both fit (round 3D item 7).
    crate::ceiling::make_room("voice", crate::ceiling::KOKORO_MB);
    let started = Instant::now();
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: Some(KOKORO.path("model.int8.onnx")),
                voices: Some(KOKORO.path("voices.bin")),
                tokens: Some(KOKORO.path("tokens.txt")),
                data_dir: Some(KOKORO.path("espeak-ng-data")),
                lexicon: Some(KOKORO.path("lexicon-us-en.txt")),
                lang: Some("en-us".into()),
                ..Default::default()
            },
            num_threads: 2,
            provider: low_memory_provider(),
            ..Default::default()
        },
        max_num_sentences: 1,
        ..Default::default()
    };
    crate::loads::note(crate::loads::Part::Kokoro, "the offline voice");
    let tts = Arc::new(OfflineTts::create(&config)?);
    crate::log::line(format!("Kokoro loaded in {:?}", started.elapsed()));
    *e = Some(Loaded { tts: tts.clone(), used: Instant::now() });
    // Unloaded once it has been quiet for the idle setting (round 3B item 10).
    let _ = std::thread::Builder::new().name("groovy-kokoro-idle".into()).spawn(|| loop {
        std::thread::sleep(crate::idle::CHECK_EVERY);
        let mut e = engine().lock().unwrap();
        match e.as_ref() {
            Some(l) if crate::idle::expired(l.used) => {
                *e = None;
                crate::log::line("Kokoro unloaded");
                return;
            }
            Some(_) => {}
            None => return,
        }
    });
    Some(tts)
}

/// Kokoro is in memory now.
pub fn is_loaded() -> bool {
    engine().try_lock().map(|e| e.is_some()).unwrap_or(true)
}

/// Frees Kokoro now (memory pressure, or speech recognition needs the room).
/// Never waits: a sentence being spoken keeps it until the next time.
pub fn free() {
    if let Ok(mut e) = engine().try_lock() {
        if e.take().is_some() {
            crate::log::line("Kokoro freed (memory)");
        }
    }
}

/// Kokoro's WAV for `text` at a speed (Settings → Speaking), if Kokoro is downloaded.
pub fn kokoro_at(text: &str, gender: &str, speed: f32) -> Option<Vec<u8>> {
    let tts = load()?;
    let config = GenerationConfig { sid: kokoro_speaker(gender), speed: clamp_speed(speed), ..Default::default() };
    let audio = tts.generate_with_config(text, &config, None::<fn(&[f32], f32) -> bool>)?;
    let samples = audio.samples();
    (!samples.is_empty()).then(|| super::cloud::wav_at(samples, audio.sample_rate().max(1) as u32))
}

// ── Kokoro's download (asked for in Settings → Voice) ──────────────────────────

struct Download {
    running: bool,
    fraction: f64,
    error: Option<String>,
}

fn download_state() -> &'static Mutex<Download> {
    static D: OnceLock<Mutex<Download>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(Download { running: false, fraction: 0.0, error: None }))
}

fn kokoro_state() -> super::ModelState {
    let d = download_state().lock().unwrap();
    if d.running {
        super::ModelState::Downloading { fraction: d.fraction }
    } else if KOKORO.is_present() {
        super::ModelState::Ready
    } else if let Some(error) = &d.error {
        super::ModelState::Failed { error: error.clone() }
    } else {
        super::ModelState::Missing
    }
}

/// Kokoro's files, downloaded now if missing (CI's peak test).
pub async fn kokoro_fetch() -> Result<(), String> {
    if KOKORO.is_present() {
        return Ok(());
    }
    let client = super::download_client()?;
    let mut done = 0;
    super::fetch_set(&client, &KOKORO, &mut done, &mut |_| {}).await
}

#[tauri::command]
pub fn kokoro_download(app: AppHandle) {
    {
        let mut d = download_state().lock().unwrap();
        if d.running {
            return;
        }
        *d = Download { running: true, fraction: 0.0, error: None };
    }
    let _ = app.emit("tts-status-changed", ());
    tauri::async_runtime::spawn(async move {
        let total = KOKORO.total_size();
        let mut done = 0;
        let mut last = Instant::now();
        let result = match super::download_client() {
            Ok(client) => {
                super::fetch_set(&client, &KOKORO, &mut done, &mut |got| {
                    if last.elapsed() > Duration::from_millis(250) {
                        last = Instant::now();
                        download_state().lock().unwrap().fraction = got as f64 / total as f64;
                        let _ = app.emit("tts-status-changed", ());
                    }
                })
                .await
            }
            Err(e) => Err(e),
        };
        {
            let mut d = download_state().lock().unwrap();
            d.running = false;
            d.error = result.as_ref().err().cloned();
        }
        crate::log::line(match &result {
            Ok(()) => "Kokoro downloaded".to_string(),
            Err(e) => format!("Kokoro download failed: {e}"),
        });
        let _ = app.emit("tts-status-changed", ());
    });
}

#[tauri::command]
pub fn kokoro_remove(app: AppHandle) {
    *engine().lock().unwrap() = None;
    let _ = std::fs::remove_dir_all(KOKORO.dir());
    let _ = app.emit("tts-status-changed", ());
}

// ── What it costs ─────────────────────────────────────────────────────────────

fn usage_path() -> std::path::PathBuf {
    crate::settings::local_dir().join("fish-usage.json")
}

fn this_month() -> String {
    let now = crate::platform::local_time();
    format!("{}-{:02}", now.year, now.month)
}

/// Characters sent to Fish Audio this month (a new month starts at 0).
pub fn usage_this_month() -> u64 {
    let v: serde_json::Value = std::fs::read(usage_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    if v["month"].as_str() == Some(&this_month()) {
        v["chars"].as_u64().unwrap_or(0)
    } else {
        0
    }
}

fn record_usage(chars: u64) {
    let total = usage_this_month() + chars;
    let _ = std::fs::write(usage_path(), json!({ "month": this_month(), "chars": total }).to_string());
}

/// About how much `chars` characters cost, in pounds.
pub fn cost_gbp(chars: u64) -> f64 {
    (chars as f64 / 1_000_000.0 * GBP_PER_MILLION_CHARS * 100.0).round() / 100.0
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsAudio {
    pub mime: String,
    pub data: String,
    pub provider: String,
}

impl From<Audio> for TtsAudio {
    fn from(a: Audio) -> Self {
        TtsAudio { mime: a.mime.into(), data: crate::okgroovy::base64(&a.bytes), provider: a.provider.into() }
    }
}

/// Groovy's voice for one sentence, or null: the page then uses its own voice.
#[tauri::command]
pub async fn tts_audio(app: AppHandle, text: String) -> Option<TtsAudio> {
    tauri::async_runtime::spawn_blocking(move || synthesize(&app, &text).map(TtsAudio::from)).await.ok().flatten()
}

/// Settings → Speaking: Kokoro's voice for that card, at the chosen speed.
#[tauri::command]
pub async fn kokoro_preview(app: AppHandle, gender: String) -> Result<TtsAudio, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = prefs(&app);
        if !KOKORO.is_present() {
            return Err(t("Download Kokoro first."));
        }
        let text = t("Hi, I'm Groovy. This is how I sound.");
        let bytes = kokoro_at(&text, &gender, p.speed).ok_or_else(|| t("Kokoro didn't speak."))?;
        Ok(TtsAudio::from(Audio { mime: "audio/wav", bytes, provider: "kokoro" }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Settings → Voice → Test voice: Fish Audio alone, so its own error shows.
#[tauri::command]
pub async fn fish_test(app: AppHandle) -> Result<TtsAudio, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = prefs(&app);
        let key = crate::secrets::get(FISH_KEY).ok_or_else(|| t("Add your Fish Audio key first."))?;
        if p.voice.is_empty() {
            return Err(t("Add the voice ID first."));
        }
        let text = t("Hi, I'm Groovy. This is how I sound.");
        let bytes = fish(&key, &p.voice, &p.model, &text, p.speed).map_err(|e| e.note())?;
        record_usage(text.chars().count() as u64);
        fish_state().lock().unwrap().note = None;
        Ok(TtsAudio::from(Audio { mime: "audio/mpeg", bytes, provider: "fish" }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    /// A key and a voice id: Fish Audio speaks first.
    pub fish_ready: bool,
    pub fish_key: bool,
    pub fish_note: Option<String>,
    pub fish_models: Vec<&'static str>,
    pub month_chars: u64,
    pub month_gbp: f64,
    pub kokoro: super::ModelState,
    pub kokoro_mb: u64,
    /// Who speaks now: "fish", "kokoro" or "system".
    pub speaking: &'static str,
}

#[tauri::command]
pub fn tts_status(app: AppHandle) -> TtsStatus {
    let p = prefs(&app);
    let key = crate::secrets::get(FISH_KEY).is_some();
    let fish_ready = key && !p.voice.is_empty();
    let resting = fish_state().lock().unwrap().rest_until.is_some_and(|t| Instant::now() < t);
    let kokoro = kokoro_state();
    let chars = usage_this_month();
    TtsStatus {
        fish_ready,
        fish_key: key,
        fish_note: fish_note(),
        fish_models: FISH_MODELS.to_vec(),
        month_chars: chars,
        month_gbp: cost_gbp(chars),
        speaking: if fish_ready && !resting {
            "fish"
        } else if kokoro == super::ModelState::Ready {
            "kokoro"
        } else {
            "system"
        },
        kokoro,
        kokoro_mb: KOKORO.total_size() / 1_000_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_speed_stays_in_what_the_voices_take() {
        assert_eq!(clamp_speed(1.05), 1.05);
        assert_eq!(clamp_speed(9.0), 2.0);
        assert_eq!(clamp_speed(0.1), 0.5);
        assert_eq!(clamp_speed(f32::NAN), 1.0);
        assert_eq!(clamp_speed(0.0), 1.0, "an old prefs file without the speed");
    }
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-request Fish Audio stand-in: answers `status` with `body` in two
    /// chunks, and hands back the request it got.
    fn mock(status: u16, body: &'static [u8]) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/tts", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = sock.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let half = body.len() / 2;
            let head = format!("HTTP/1.1 {status} X\r\ncontent-type: audio/mpeg\r\ntransfer-encoding: chunked\r\n\r\n");
            sock.write_all(head.as_bytes()).unwrap();
            for part in [&body[..half], &body[half..]] {
                if !part.is_empty() {
                    sock.write_all(format!("{:x}\r\n", part.len()).as_bytes()).unwrap();
                    sock.write_all(part).unwrap();
                    sock.write_all(b"\r\n").unwrap();
                    sock.flush().unwrap();
                }
            }
            sock.write_all(b"0\r\n\r\n").unwrap();
            String::from_utf8_lossy(&req).to_string()
        });
        (url, handle)
    }

    #[test]
    fn fish_audio_streams_the_mp3_with_the_key_only_in_the_header() {
        let (url, server) = mock(200, b"ID3-fake-mp3-bytes");
        let got = tauri::async_runtime::block_on(fish_request(&url, "secret-key-123", "voice-abc", "s2-pro", "Hello there.", 1.0));
        assert_eq!(got.unwrap(), b"ID3-fake-mp3-bytes");
        let req = server.join().unwrap();
        let lower = req.to_ascii_lowercase();
        assert!(lower.contains("authorization: bearer secret-key-123"));
        assert!(lower.contains("model: s2-pro"));
        let body = &req[req.find("\r\n\r\n").unwrap()..];
        assert!(body.contains("\"reference_id\":\"voice-abc\"") && body.contains("Hello there."));
        assert!(!body.contains("secret-key-123"), "the key is never in the body");
    }

    #[test]
    fn refusals_say_why_without_the_key_and_limits_rest_fish_audio() {
        for (status, want) in [(401, FishError::Key), (402, FishError::NoCredit), (429, FishError::Limit), (400, FishError::Refused(400))] {
            let (url, server) = mock(status, b"{}");
            let got = tauri::async_runtime::block_on(fish_request(&url, "secret-key-123", "v", "s1", "Hi.", 1.0));
            server.join().unwrap();
            let err = got.unwrap_err();
            assert_eq!(err, want);
            assert!(!err.note().contains("secret-key-123"));
            assert!(!format!("{err:?}").contains("secret-key-123"));
        }
        assert!(FishError::NoCredit.rests() && FishError::Limit.rests());
        assert!(!FishError::Key.rests() && !FishError::Offline.rests());
        let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let offline = tauri::async_runtime::block_on(fish_request(&format!("http://{closed}/v1/tts"), "k", "v", "s1", "Hi.", 1.0));
        assert_eq!(offline.unwrap_err(), FishError::Offline);
    }

    #[test]
    fn an_unknown_model_is_the_default_and_the_cost_is_rough() {
        assert_eq!(fish_model("s1"), "s1");
        assert_eq!(fish_model("s2.1-pro-free"), FISH_DEFAULT_MODEL);
        assert_eq!(cost_gbp(1_000_000), 11.5);
        assert_eq!(cost_gbp(0), 0.0);
        assert_eq!((kokoro_speaker("female"), kokoro_speaker("male")), (3, 16));
    }

    #[test]
    fn the_flow_prefs_never_hold_the_key() {
        let prefs = serde_json::to_string(&crate::flow::store::Prefs::default()).unwrap();
        assert!(prefs.contains("fishVoice") && prefs.contains("fishModel"));
        let lower = prefs.to_ascii_lowercase();
        assert!(!lower.contains("fishkey") && !lower.contains("apikey") && !lower.contains("secret"), "{prefs}");
    }

    #[test]
    fn kokoro_is_a_pinned_checked_download() {
        assert!(KOKORO.files.iter().all(|f| f.url.starts_with("https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_0/resolve/2a36") && f.sha256.len() == 64));
        for needed in ["model.int8.onnx", "voices.bin", "tokens.txt", "lexicon-us-en.txt", "espeak-ng-data/phontab", "espeak-ng-data/en_dict"] {
            assert!(KOKORO.files.iter().any(|f| f.name == needed), "{needed}");
        }
        let mb = KOKORO.total_size() / 1_000_000;
        assert!((150..200).contains(&mb), "{mb} MB");
    }
}

#[cfg(test)]
mod with_models {
    /// CI on Windows (FIXES_ROUND3B item 8c): downloads Kokoro as Settings
    /// would (pinned, size and SHA-256 checked) and speaks through the same
    /// path Groovy uses; writes kokoro-test.wav next to the models.
    #[test]
    #[ignore]
    fn kokoro_downloads_and_speaks() {
        let client = super::super::download_client().unwrap();
        let mut done = 0;
        tauri::async_runtime::block_on(super::super::fetch_set(&client, &super::KOKORO, &mut done, &mut |_| {})).expect("download");
        assert!(super::KOKORO.is_present());
        let started = std::time::Instant::now();
        let wav = super::kokoro_at("Hi, I'm Groovy. It's 9:05, and the Asus VivoBook S14 is charging.", "female", 1.0).expect("Kokoro speaks");
        println!("Kokoro: {} bytes of WAV in {:?}", wav.len(), started.elapsed());
        assert!(wav.len() > 44 + 24_000 * 2, "at least a second of audio");
        std::fs::write(super::KOKORO.dir().join("kokoro-test.wav"), &wav).unwrap();
    }
}
