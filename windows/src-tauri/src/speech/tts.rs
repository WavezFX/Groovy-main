// Groovy's voice (FIXES_ROUND3B item 8c). Fish Audio first whenever it is set
// up (the user's own key, in the OS store, and a voice id), then the offline
// voice on this PC, then whatever answered before (ElevenLabs or Windows' own
// voices, in the page).
//
// The offline voice (round 4A item 1) is the light one by default: Piper's
// "libritts_r medium" through sherpa-onnx, about 80 MB on disk, fast and small
// in memory. Kokoro is the optional richer one, used when it is chosen, or
// ("auto") when it was downloaded before the light voice existed. Either is
// downloaded only when the user asks.
//
// A refused or failed Fish Audio request falls through to Kokoro for that
// reply; no credit or a rate limit also rests Fish Audio for ten minutes, as
// brain_ai.rs does with the brains, and quick settings shows why. Nothing is
// ever silent.
//
// The key goes only in the Authorization header: never in a log line, an
// error message or the settings file.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig};
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

/// The light offline voice: Piper's libritts_r medium, English (about 80 MB).
pub static LIGHT: ModelSet = ModelSet { id: "piper-en-libritts-r-medium", files: super::light_files::FILES };

/// Which offline voice speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offline {
    Light,
    Kokoro,
}

impl Offline {
    pub fn id(self) -> &'static str {
        match self {
            Offline::Light => "light",
            Offline::Kokoro => "kokoro",
        }
    }

    fn of(id: &str) -> Option<Offline> {
        match id {
            "light" => Some(Offline::Light),
            "kokoro" => Some(Offline::Kokoro),
            _ => None,
        }
    }

    fn set(self) -> &'static ModelSet {
        match self {
            Offline::Light => &LIGHT,
            Offline::Kokoro => &KOKORO,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Offline::Light => "the light voice",
            Offline::Kokoro => "Kokoro",
        }
    }
}

/// The offline voice to use, from the choice in Settings (`pref`: "light",
/// "kokoro" or "auto") and what is downloaded (`has`). "auto" is Kokoro when
/// it is there (it was the only offline voice before), else the light one. A
/// chosen voice that isn't downloaded gives way to the other if that one is.
pub fn offline_for(pref: &str, has: impl Fn(Offline) -> bool) -> Option<Offline> {
    let wanted = Offline::of(pref).unwrap_or(if has(Offline::Kokoro) { Offline::Kokoro } else { Offline::Light });
    let other = if wanted == Offline::Light { Offline::Kokoro } else { Offline::Light };
    [wanted, other].into_iter().find(|v| has(*v))
}

fn downloaded(v: Offline) -> bool {
    v.set().is_present()
}

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
    /// The offline voice: "light", "kokoro" or "auto".
    offline: String,
}

fn prefs(app: &AppHandle) -> Prefs {
    crate::flow::get(app)
        .map(|f| {
            let p = &f.store.lock().unwrap().prefs;
            Prefs { voice: p.fish_voice.trim().to_string(), model: p.fish_model.clone(), gender: p.voice_gender.clone(), pick: p.voice_pick.clone(), speed: clamp_speed(p.voice_speed), offline: p.offline_voice.clone() }
        })
        .unwrap_or(Prefs { voice: String::new(), model: FISH_DEFAULT_MODEL.into(), gender: "female".into(), pick: "fish".into(), speed: 1.0, offline: "auto".into() })
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
                crate::log::line(format!("voice: Fish Audio failed ({err:?}), the offline voice answers"));
                let mut s = fish_state().lock().unwrap();
                s.note = Some(err.note());
                if err.rests() {
                    s.rest_until = Some(Instant::now() + FISH_REST);
                }
                let _ = app.emit("tts-status-changed", ());
            }
        }
    }
    let which = offline_for(&p.offline, downloaded)?;
    offline_at(which, text, &p.gender, p.speed).map(|bytes| Audio { mime: "audio/wav", bytes, provider: which.id() })
}

// ── Kokoro ────────────────────────────────────────────────────────────────────

struct Loaded {
    which: Offline,
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

/// The light voice's speakers, picked for a steady pitch: a woman's and a man's.
pub fn light_speaker(gender: &str) -> i32 {
    if gender == "male" {
        31
    } else {
        38
    }
}

/// Memory the voice needs while it speaks (the ceiling's estimate).
pub fn needs_mb(which: Offline) -> f64 {
    match which {
        Offline::Light => crate::resources::ceiling::LIGHT_MB,
        Offline::Kokoro => crate::resources::ceiling::KOKORO_MB,
    }
}

fn config(which: Offline) -> OfflineTtsConfig {
    let model = match which {
        Offline::Light => OfflineTtsModelConfig {
            vits: OfflineTtsVitsModelConfig {
                // Its weights mapped from disk, not copied, as Kokoro's are (mapped.rs).
                model: Some(super::mapped::mapped_model(Path::new(&LIGHT.path("en_US-libritts_r-medium.onnx"))).map_or_else(|| LIGHT.path("en_US-libritts_r-medium.onnx"), |p| p.display().to_string())),
                tokens: Some(LIGHT.path("tokens.txt")),
                data_dir: Some(LIGHT.path("espeak-ng-data")),
                ..Default::default()
            },
            num_threads: 2,
            provider: super::low_memory_provider(),
            ..Default::default()
        },
        Offline::Kokoro => OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                // Its weights mapped from disk, not copied (mapped.rs).
                model: Some(super::mapped::mapped_model(Path::new(&KOKORO.path("model.int8.onnx"))).map_or_else(|| KOKORO.path("model.int8.onnx"), |p| p.display().to_string())),
                voices: Some(KOKORO.path("voices.bin")),
                tokens: Some(KOKORO.path("tokens.txt")),
                data_dir: Some(KOKORO.path("espeak-ng-data")),
                lexicon: Some(KOKORO.path("lexicon-us-en.txt")),
                lang: Some("en-us".into()),
                ..Default::default()
            },
            num_threads: 2,
            provider: super::low_memory_provider(),
            ..Default::default()
        },
    };
    OfflineTtsConfig { model, max_num_sentences: 1, ..Default::default() }
}

fn load(which: Offline) -> Option<Arc<OfflineTts>> {
    let mut e = engine().lock().unwrap();
    if let Some(l) = e.as_mut().filter(|l| l.which == which) {
        l.used = Instant::now();
        return Some(l.tts.clone());
    }
    // The other voice goes first: never both in memory.
    *e = None;
    if !which.set().is_present() {
        return None;
    }
    // Speech recognition and the voice together only when both fit (round 3D item 7).
    crate::resources::ceiling::make_room("voice", needs_mb(which));
    let started = Instant::now();
    crate::resources::loads::note(crate::resources::loads::Part::Kokoro, "the offline voice");
    let tts = Arc::new(OfflineTts::create(&config(which))?);
    crate::log::line(format!("{} loaded in {:?}", which.name(), started.elapsed()));
    *e = Some(Loaded { which, tts: tts.clone(), used: Instant::now() });
    // Unloaded once it has been quiet for the idle setting (round 3B item 10).
    crate::resources::register("kokoro", reap_idle);
    Some(tts)
}

fn reap_idle() {
    let Ok(mut e) = engine().try_lock() else { return };
    if e.as_ref().is_some_and(|l| crate::resources::idle::expired(l.used)) {
        *e = None;
        crate::log::line("the offline voice unloaded");
    }
}

/// Kokoro is in memory now.
pub fn is_loaded() -> bool {
    engine().try_lock().map(|e| e.is_some()).unwrap_or(true) || super::helper::is_running(super::helper::Kind::Tts)
}

/// Frees Kokoro now (memory pressure, or speech recognition needs the room).
/// Never waits: a sentence being spoken keeps it until the next time.
pub fn free() {
    if let Ok(mut e) = engine().try_lock() {
        if e.take().is_some() {
            crate::log::line("the offline voice freed (memory)");
        }
    }
    super::helper::stop(super::helper::Kind::Tts);
}

/// The longest piece Kokoro speaks at once. Its working memory grows with the
/// length of what it says (about 1 MB a character), so a reply is spoken in
/// clauses of about this size and joined (round 3D item 7). Round 3N tried 30: 9 MB less
/// on Windows (171 -> 162) but a call's reply took 4 s longer, so it stays 40. Each
/// clause's lead-in and trailing silence is trimmed, so the clauses join with no gap.
const CLAUSE_CHARS: usize = 40;

/// `text` in clauses of at most `max` characters: cut after punctuation where
/// it can be, between words where it must, never inside a word.
fn clauses(text: &str, max: usize) -> Vec<String> {
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max {
            pieces.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
        if word.ends_with([',', ';', ':', '.', '!', '?']) {
            pieces.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    // Short neighbours back together, so "Hi," isn't a clause of its own.
    let mut out: Vec<String> = Vec::new();
    for p in pieces {
        match out.last_mut() {
            Some(last) if last.chars().count() + 1 + p.chars().count() <= max => {
                last.push(' ');
                last.push_str(&p);
            }
            _ => out.push(p),
        }
    }
    out
}

/// Kokoro's WAV for `text`, if Kokoro is downloaded (the tests' voice).
#[cfg(test)]
pub fn kokoro_at(text: &str, gender: &str, speed: f32) -> Option<Vec<u8>> {
    offline_at(Offline::Kokoro, text, gender, speed)
}

/// The offline voice's WAV for `text` at a speed (Settings → Speaking), if it
/// is downloaded. In the voice's helper process (round 3N item 2), or here if
/// that can't start.
pub fn offline_at(which: Offline, text: &str, gender: &str, speed: f32) -> Option<Vec<u8>> {
    if super::helper::enabled() {
        if let Some(r) = super::helper::speak(which.id(), text, gender, speed) {
            return r;
        }
    }
    offline_here(which, text, gender, speed)
}

/// The helper's side: the voice named by `id` ("light" or "kokoro").
pub(crate) fn offline_here_by_id(id: &str, text: &str, gender: &str, speed: f32) -> Option<Vec<u8>> {
    offline_here(Offline::of(id)?, text, gender, speed)
}

/// `samples` without leading silence, nor trailing silence beyond `keep` seconds.
fn trim_tail(samples: &[f32], rate: f32, keep: f32) -> &[f32] {
    let end = samples.iter().rposition(|v| v.abs() > 0.01).map_or(0, |i| i + 1);
    // And the lead-in silence too, but for a moment.
    let start = samples.iter().position(|v| v.abs() > 0.01).map_or(0, |i| i.saturating_sub((0.02 * rate) as usize)).min(end);
    &samples[start..(end + (keep * rate) as usize).min(samples.len())]
}

/// The light voice speaks in clauses of up to 60 characters: 77 MB while speaking
/// against 109 at 120 (measured on Linux, anonymous memory), and as fast.
const LIGHT_CLAUSE_CHARS: usize = 60;

/// The offline voice's WAV, made in this process.
fn offline_here(which: Offline, text: &str, gender: &str, speed: f32) -> Option<Vec<u8>> {
    let tts = load(which)?;
    let (sid, max) = match which {
        Offline::Light => (light_speaker(gender), LIGHT_CLAUSE_CHARS),
        Offline::Kokoro => (kokoro_speaker(gender), CLAUSE_CHARS),
    };
    let config = GenerationConfig { sid, speed: clamp_speed(speed), ..Default::default() };
    let mut samples: Vec<f32> = Vec::new();
    let mut rate = 0;
    let pieces = clauses(text, max);
    let last = pieces.len().saturating_sub(1);
    for (i, clause) in pieces.iter().enumerate() {
        let audio = tts.generate_with_config(clause, &config, None::<fn(&[f32], f32) -> bool>)?;
        rate = audio.sample_rate();
        // Joined with no gap: each clause's own trailing silence is cut to the pause its end calls for.
        let pause = if i == last { 0.3 } else if clause.ends_with(['.', '!', '?']) { 0.25 } else if clause.ends_with([',', ';', ':']) { 0.12 } else { 0.02 };
        samples.extend_from_slice(trim_tail(audio.samples(), rate.max(1) as f32, pause));
    }
    (!samples.is_empty()).then(|| super::cloud::wav_at(&samples, rate.max(1) as u32))
}

// ── Kokoro's download (asked for in Settings → Voice) ──────────────────────────

struct Download {
    running: bool,
    fraction: f64,
    error: Option<String>,
}

fn download_state(which: Offline) -> &'static Mutex<Download> {
    static K: OnceLock<Mutex<Download>> = OnceLock::new();
    static L: OnceLock<Mutex<Download>> = OnceLock::new();
    let cell = if which == Offline::Kokoro { &K } else { &L };
    cell.get_or_init(|| Mutex::new(Download { running: false, fraction: 0.0, error: None }))
}

fn voice_state(which: Offline) -> super::ModelState {
    let d = download_state(which).lock().unwrap();
    if d.running {
        super::ModelState::Downloading { fraction: d.fraction }
    } else if which.set().is_present() {
        super::ModelState::Ready
    } else if let Some(error) = &d.error {
        super::ModelState::Failed { error: error.clone() }
    } else {
        super::ModelState::Missing
    }
}

/// The offline voices' files, downloaded now if missing (CI's peak test).
#[cfg(feature = "worktest")]
pub async fn offline_fetch() -> Result<(), String> {
    let client = super::download_client()?;
    for set in [&LIGHT, &KOKORO] {
        if !set.is_present() {
            let mut done = 0;
            super::fetch_set(&client, set, &mut done, &mut |_| {}).await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn kokoro_download(app: AppHandle) {
    download(app, Offline::Kokoro);
}

/// The light offline voice's download (round 4A item 1).
#[tauri::command]
pub fn light_download(app: AppHandle) {
    download(app, Offline::Light);
}

fn download(app: AppHandle, which: Offline) {
    {
        let mut d = download_state(which).lock().unwrap();
        if d.running {
            return;
        }
        *d = Download { running: true, fraction: 0.0, error: None };
    }
    let _ = app.emit("tts-status-changed", ());
    tauri::async_runtime::spawn(async move {
        let set = which.set();
        let total = set.total_size();
        let mut done = 0;
        let mut last = Instant::now();
        let result = match super::download_client() {
            Ok(client) => {
                super::fetch_set(&client, set, &mut done, &mut |got| {
                    if last.elapsed() > Duration::from_millis(250) {
                        last = Instant::now();
                        download_state(which).lock().unwrap().fraction = got as f64 / total as f64;
                        let _ = app.emit("tts-status-changed", ());
                    }
                })
                .await
            }
            Err(e) => Err(e),
        };
        {
            let mut d = download_state(which).lock().unwrap();
            d.running = false;
            d.error = result.as_ref().err().cloned();
        }
        crate::log::line(match &result {
            Ok(()) => format!("{} downloaded", which.name()),
            Err(e) => format!("{} download failed: {e}", which.name()),
        });
        let _ = app.emit("tts-status-changed", ());
    });
}

#[tauri::command]
pub fn kokoro_remove(app: AppHandle) {
    *engine().lock().unwrap() = None;
    super::helper::stop(super::helper::Kind::Tts);
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

// ── Played here, not in the page (round 4A item 2) ───────────────────────────
//
// A reply used to cross into the island's page as base64 and play in an
// <audio> element: each sentence's decoded copy stayed in the renderer, and a
// call added about 70 MB to it. Now Rust plays it (rodio, as the music does)
// and the page only waits for the end. The audio device is opened for each
// sentence and let go after it, so nothing stays open between replies.

/// Bumped by every stop: a sentence still being made, or queued, is dropped.
static SAY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PLAYING: Mutex<Option<Arc<rodio::Player>>> = Mutex::new(None);

/// Plays `audio` to the end, or until `tts_stop`. False: it couldn't be played.
fn play(audio: &Audio, generation: u64) -> bool {
    let stopped = || SAY_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != generation;
    if stopped() {
        return true;
    }
    let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() else { return false };
    sink.log_on_drop(false);
    let Ok(decoder) = rodio::Decoder::new(std::io::Cursor::new(audio.bytes.clone())) else { return false };
    let player = Arc::new(rodio::Player::connect_new(sink.mixer()));
    player.append(decoder);
    {
        let mut playing = PLAYING.lock().unwrap();
        // Stopped while the device was opening: don't start it.
        if stopped() {
            return true;
        }
        *playing = Some(player.clone());
    }
    player.sleep_until_end();
    let mut playing = PLAYING.lock().unwrap();
    if playing.as_ref().is_some_and(|p| Arc::ptr_eq(p, &player)) {
        *playing = None;
    }
    true
}

/// Says one sentence with Groovy's own voice (Fish Audio, then the offline
/// voice), played here. Resolves when it has been said or stopped, with who
/// said it; null: none of Groovy's voices could, the page uses Windows' voice.
#[tauri::command]
pub async fn tts_say(app: AppHandle, text: String) -> Option<&'static str> {
    let generation = SAY_GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let audio = synthesize(&app, &text)?;
        if SAY_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != generation {
            return Some(audio.provider);
        }
        play(&audio, generation).then_some(audio.provider)
    })
    .await
    .ok()
    .flatten()
}

/// Barge-in, a hang-up: what Groovy is saying stops at once, and what was
/// still to be said is dropped.
#[tauri::command]
pub fn tts_stop() {
    SAY_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(player) = PLAYING.lock().unwrap().take() {
        player.stop();
    }
}

/// Settings → Speaking: Kokoro's voice for that card, at the chosen speed.
#[tauri::command]
pub async fn kokoro_preview(app: AppHandle, gender: String) -> Result<TtsAudio, String> {
    preview(app, Offline::Kokoro, gender).await
}

/// Settings → Speaking: the light voice says a sentence.
#[tauri::command]
pub async fn light_preview(app: AppHandle, gender: String) -> Result<TtsAudio, String> {
    preview(app, Offline::Light, gender).await
}

async fn preview(app: AppHandle, which: Offline, gender: String) -> Result<TtsAudio, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = prefs(&app);
        if !which.set().is_present() {
            return Err(match which {
                Offline::Kokoro => t("Download Kokoro first."),
                Offline::Light => t("Download the light voice first."),
            });
        }
        let text = t("Hi, I'm Groovy. This is how I sound.");
        let bytes = offline_at(which, &text, &gender, p.speed).ok_or_else(|| match which {
            Offline::Kokoro => t("Kokoro didn't speak."),
            Offline::Light => t("The light voice didn't speak."),
        })?;
        Ok(TtsAudio::from(Audio { mime: "audio/wav", bytes, provider: which.id() }))
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
    /// The light offline voice (round 4A item 1).
    pub light: super::ModelState,
    pub light_mb: u64,
    /// The offline voice that would speak now: "light", "kokoro", or None (none downloaded).
    pub offline_voice: Option<&'static str>,
    /// Who speaks now: "fish", "light", "kokoro" or "system".
    pub speaking: &'static str,
}

#[tauri::command]
pub fn tts_status(app: AppHandle) -> TtsStatus {
    let p = prefs(&app);
    let key = crate::secrets::get(FISH_KEY).is_some();
    let fish_ready = key && !p.voice.is_empty();
    let resting = fish_state().lock().unwrap().rest_until.is_some_and(|t| Instant::now() < t);
    let kokoro = voice_state(Offline::Kokoro);
    let light = voice_state(Offline::Light);
    let offline = offline_for(&p.offline, downloaded);
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
        } else {
            offline.map_or("system", Offline::id)
        },
        kokoro,
        kokoro_mb: KOKORO.total_size() / 1_000_000,
        light,
        light_mb: LIGHT.total_size() / 1_000_000,
        offline_voice: offline.map(Offline::id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_drops_the_sentence_still_being_made() {
        let audio = Audio { mime: "audio/wav", bytes: super::super::cloud::wav_at(&[0.0; 160], 16_000), provider: "light" };
        let generation = SAY_GENERATION.load(std::sync::atomic::Ordering::SeqCst);
        tts_stop();
        // Stopped before it started: nothing plays, and it counts as said.
        assert!(play(&audio, generation));
        assert!(PLAYING.lock().unwrap().is_none());
    }

    #[test]
    fn the_light_voice_is_the_default_and_kokoro_stays_for_those_who_have_it() {
        let has = |list: &'static [Offline]| move |v: Offline| list.contains(&v);
        // A new install: the light voice, once it is downloaded.
        assert_eq!(offline_for("auto", has(&[Offline::Light])), Some(Offline::Light));
        assert_eq!(offline_for("auto", has(&[])), None);
        // Kokoro downloaded before: it keeps speaking until the user changes it.
        assert_eq!(offline_for("auto", has(&[Offline::Kokoro])), Some(Offline::Kokoro));
        assert_eq!(offline_for("auto", has(&[Offline::Kokoro, Offline::Light])), Some(Offline::Kokoro));
        // Chosen: that one, or the other if the chosen one isn't there.
        assert_eq!(offline_for("light", has(&[Offline::Kokoro, Offline::Light])), Some(Offline::Light));
        assert_eq!(offline_for("light", has(&[Offline::Kokoro])), Some(Offline::Kokoro));
        assert_eq!(offline_for("kokoro", has(&[Offline::Light])), Some(Offline::Light));
        assert_eq!(offline_for("nonsense", has(&[Offline::Light])), Some(Offline::Light));
        assert_ne!(light_speaker("male"), light_speaker("female"));
    }

    #[test]
    fn the_light_voice_is_a_pinned_checked_download_of_its_english_parts() {
        let names: Vec<&str> = LIGHT.files.iter().map(|f| f.name).collect();
        for needed in ["en_US-libritts_r-medium.onnx", "tokens.txt", "espeak-ng-data/phontab", "espeak-ng-data/en_dict", "espeak-ng-data/lang/gmw/en-US"] {
            assert!(names.contains(&needed), "{needed}");
        }
        assert!(LIGHT.files.iter().all(|f| f.url.contains("/resolve/fe872db7e697e9f2a45d09328953f1fd2a9df630/") && f.sha256.len() == 64 && f.size > 0));
        let mb = LIGHT.total_size() / 1_000_000;
        assert!((70..=90).contains(&mb), "{mb} MB");
        assert!(LIGHT.total_size() < KOKORO.total_size() / 2);
    }

    #[test]
    fn replies_are_spoken_in_short_clauses_cut_between_words() {
        let reply = "Hi, I'm Groovy. It's 9:05, and the Asus VivoBook S14 is charging. Your next meeting with the design team starts in twenty minutes.";
        let parts = super::clauses(reply, 40);
        assert_eq!(parts.join(" "), reply, "nothing lost or reordered");
        assert!(parts.iter().all(|p| p.chars().count() <= 40), "{parts:?}");
        assert_eq!(parts[0], "Hi, I'm Groovy. It's 9:05,", "short clauses kept together, cut after punctuation");
        assert_eq!(super::clauses("Hello there.", 40), ["Hello there."]);
        assert_eq!(super::clauses("Supercalifragilisticexpialidociousness-and-more-words", 10).len(), 1, "never inside a word");
        assert!(super::clauses("   ", 40).is_empty());
        let s = [0.0, 0.0, 0.5, -0.4, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(super::trim_tail(&s, 100.0, 0.01), &[0.0, 0.0, 0.5, -0.4, 0.0], "a moment of lead-in, the sound, then `keep` of the tail");
    }

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
            // The client may hang up as soon as it has read a refusal's status,
            // so a write after the head can fail with a broken pipe.
            let _ = (|| -> std::io::Result<()> {
                sock.write_all(head.as_bytes())?;
                for part in [&body[..half], &body[half..]] {
                    if !part.is_empty() {
                        sock.write_all(format!("{:x}\r\n", part.len()).as_bytes())?;
                        sock.write_all(part)?;
                        sock.write_all(b"\r\n")?;
                        sock.flush()?;
                    }
                }
                sock.write_all(b"0\r\n\r\n")
            })();
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

    /// Round 4A item 1: the light voice's memory while it speaks, and its speed,
    /// beside Kokoro's, in this process (Task Manager's private working set on
    /// Windows). Run on the Windows build: cargo test -p groovy voices_measured -- --ignored --nocapture
    #[test]
    #[ignore]
    fn voices_measured() {
        let client = super::super::download_client().unwrap();
        let texts = [
            "Hi, I'm Groovy. It's a quarter past three, and the report is on its way to Sam.",
            "Your next meeting is with the design team at four, in the small room upstairs.",
            "I paused the music and turned the volume down a little, like you asked.",
        ];
        // One voice per process when asked (GROOVY_VOICE=light|kokoro), so neither is measured on the other's leftovers.
        let only = std::env::var("GROOVY_VOICE").ok();
        for which in [super::Offline::Light, super::Offline::Kokoro].into_iter().filter(|w| only.as_deref().is_none_or(|o| o == w.id())) {
            let mut done = 0;
            tauri::async_runtime::block_on(super::super::fetch_set(&client, which.set(), &mut done, &mut |_| {})).expect("download");
            super::free();
            crate::resources::ceiling::give_back();
            let before = crate::resources::ceiling::own_mb();
            let mut peak = before;
            let mut times = Vec::new();
            let mut bytes = 0;
            for (i, text) in texts.iter().enumerate() {
                let started = std::time::Instant::now();
                let wav = super::offline_here(which, text, if i % 2 == 0 { "female" } else { "male" }, 1.0).expect("it speaks");
                times.push(started.elapsed().as_secs_f32());
                bytes += wav.len();
                peak = peak.max(crate::resources::ceiling::own_mb());
                std::fs::write(which.set().dir().join(format!("sample-{i}.wav")), &wav).unwrap();
            }
            println!(
                "{}: {:.0} MB before, {:.0} MB at most while speaking (+{:.0} MB); {} seconds per sentence; {} KB of speech",
                which.name(),
                before,
                peak,
                peak - before,
                times.iter().map(|t| format!("{t:.2}")).collect::<Vec<_>>().join(", "),
                bytes / 1024
            );
            super::free();
        }
    }
}
