// « OK Groovy » on Windows and Linux: say it, then ask anything. Port of the
// Mac's Voice/ (VoiceEngine.swift and its friends).
//
// While it is on, the microphone is open and a cheap energy detector cuts what
// you say into stretches; only a stretch of speech goes to Parakeet, on this
// computer, and only a stretch that starts with « OK Groovy » goes further:
// what you said after it goes to Claude with your own key (brain.rs), never
// the audio. Groovy shows it all in the island —
// listening, thinking, then the answer, said out loud by the system's voice
// (the island's speechSynthesis) or ElevenLabs with your key.
//
// It stops listening while you dictate, while a call is recorded and while it
// speaks itself.

pub mod basic;
pub mod brain;
pub mod wake;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::flow::live;
use crate::speech::{self, audio};

/// What the island shows of a conversation.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct VoiceView {
    /// "listening", "thinking", "speaking".
    pub phase: String,
    /// What you said (after « OK Groovy »).
    pub heard: String,
    /// Groovy's answer.
    pub answer: String,
}

struct Running {
    stop: Arc<AtomicBool>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Rust waits on these for the island's answers and the end of speech.
fn pending() -> &'static Mutex<HashMap<u64, mpsc::Sender<String>>> {
    static P: OnceLock<Mutex<HashMap<u64, mpsc::Sender<String>>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Whether « OK Groovy » can listen: the speech model. An AI is not needed
/// to listen: the basic commands work without one, and Groovy says when a
/// question needs one (FIXES_ROUND4 item 1).
pub fn requirements_met() -> bool {
    speech::is_ready()
}

/// The microphone « OK Groovy » listens to, and why it couldn't open it.
static MIC: Mutex<(String, Option<String>)> = Mutex::new((String::new(), None));
/// "Test OK Groovy": each step is reported until then.
static TEST_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);
/// The missing speech model was told once this run.
static TOLD_NO_SPEECH: AtomicBool = AtomicBool::new(false);

/// One step of "Test OK Groovy": "wake", "words", "answer", "spoken".
fn test_step(app: &AppHandle, step: &str, ok: bool, detail: &str) {
    let mut until = TEST_UNTIL.lock().unwrap();
    if until.is_none_or(|t| Instant::now() > t) {
        *until = None;
        return;
    }
    if step == "spoken" || !ok {
        *until = None;
    }
    let _ = app.emit("okgroovy-test", json!({ "step": step, "ok": ok, "detail": detail }));
}

/// The answer is out: the follow-up window starts, or "thanks" / "stop" ends the conversation.
fn replied(app: &AppHandle, question: &str) {
    use crate::resources::state::{fire, is_dismissal, Event};
    fire(app, if is_dismissal(question) { Event::Dismissed } else { Event::ReplyDone });
}

/// Says it and shows it in the island, with a button when there is one.
fn tell(app: &AppHandle, heard: &str, text: String, action: Option<(String, &str)>) {
    show(app, Some(VoiceView { phase: "speaking".into(), heard: heard.into(), answer: text.clone() }));
    live::announce(app, "error", "#FFB020", text.clone(), action, 12);
    speak(app, &text);
    crate::resources::state::fire(app, crate::resources::state::Event::ReplyDone);
}

/// Starts or stops listening to follow the preference.
pub fn sync(app: &AppHandle) {
    let wanted = crate::flow::get(app).is_some_and(|f| f.store.lock().unwrap().prefs.voice);
    // On, but it can't hear anything yet: say so, with the download.
    if wanted && !speech::is_ready() && !TOLD_NO_SPEECH.swap(true, Ordering::Relaxed) {
        live::announce(
            app,
            "download",
            "#FFB020",
            crate::i18n::t("« OK Groovy » is on, but Groovy can't hear it yet: download the speech model."),
            Some((crate::i18n::t("Download"), "openSpeechSetup")),
            15,
        );
    }
    let mut running = RUNNING.lock().unwrap();
    match (wanted && requirements_met(), running.is_some()) {
        (true, false) => {
            let stop = Arc::new(AtomicBool::new(false));
            let app2 = app.clone();
            let stop2 = stop.clone();
            std::thread::Builder::new().name("groovy-voice".into()).spawn(move || run(app2, stop2)).ok();
            *running = Some(Running { stop });
            speech::fetch_kws();
            crate::log::line("voice: « OK Groovy » listening");
        }
        (false, true) => {
            if let Some(r) = running.take() {
                r.stop.store(true, Ordering::Relaxed);
            }
            let dictation = crate::flow::get(app).is_some_and(|f| f.store.lock().unwrap().prefs.enabled);
            if !dictation && !crate::notes::is_recording(app) {
                speech::unload();
            }
            crate::log::line("voice: « OK Groovy » off");
        }
        _ => {}
    }
}

/// Cloud speech answers first (Settings → Voice), when it has a key.
fn cloud_first(app: &AppHandle) -> bool {
    crate::flow::get(app).is_some_and(|f| f.store.lock().unwrap().prefs.cloud_speech) && !speech::cloud::available().is_empty()
}

/// The words of a command: the cloud engine first when it is set up, the
/// PC's own (loaded now if it isn't) when there is none or it fails.
fn hear(app: &AppHandle, samples: &[f32]) -> Option<String> {
    if cloud_first(app) {
        if let Some((text, _)) = tauri::async_runtime::block_on(speech::cloud::transcribe(samples, &[], None)) {
            return Some(text.trim().to_string());
        }
    }
    speech::transcribe(samples).map(|t| t.text.trim().to_string())
}

/// « OK Groovy » is listening.
pub fn is_listening() -> bool {
    RUNNING.lock().unwrap().is_some()
}

/// Starts again on another microphone.
pub fn restart(app: &AppHandle) {
    if let Some(r) = RUNNING.lock().unwrap().take() {
        r.stop.store(true, Ordering::Relaxed);
    }
    sync(app);
}

fn show(app: &AppHandle, view: Option<VoiceView>) {
    live::update(app, |s| s.voice = view);
}

enum Mode {
    /// Waiting for « OK Groovy ».
    Wake,
    /// Heard « OK Groovy » alone: what comes next is the question.
    Command { text: String, until: Instant },
    /// Groovy asked something back: an answer comes without « OK Groovy ».
    FollowUp { until: Instant },
}

fn run(app: AppHandle, stop: Arc<AtomicBool>) {
    let mic = crate::flow::get(&app).map(|f| f.store.lock().unwrap().prefs.mic.clone()).unwrap_or_default();
    let mic_name = if mic.is_empty() {
        audio::input_devices().into_iter().find(|d| d.default).map(|d| d.name).unwrap_or_else(|| crate::i18n::t("the default microphone"))
    } else {
        mic.clone()
    };
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let capture = audio::Capture::start(audio::Source::Mic(if mic.is_empty() { None } else { Some(mic) }), move |s| {
        let _ = tx.send(s.to_vec());
    });
    let mut capture = match capture {
        Ok(c) => {
            *MIC.lock().unwrap() = (mic_name, None);
            c
        }
        Err(err) => {
            crate::log::line(format!("voice: microphone: {err}"));
            let text = crate::i18n::tf("« OK Groovy » can't hear you: the microphone “{mic}” didn't open.", &[("mic", &mic_name)]);
            *MIC.lock().unwrap() = (mic_name, Some(err));
            tell(&app, "", text, Some((crate::i18n::t("Choose a microphone"), "openVoice")));
            *RUNNING.lock().unwrap() = None;
            return;
        }
    };
    let mut vad = wake::Vad::default();
    // The small spotter hears « OK Groovy »; the full engine is asked only after it.
    let mut spotter = speech::kws::Spotter::load();
    let mut mode = Mode::Wake;
    let mut brain = brain::Brain::default();
    let mut hide_at: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let chunk = match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(c) => Some(c),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let now = Instant::now();
        // The island's last answer goes away after a while.
        if hide_at.is_some_and(|t| now > t) {
            hide_at = None;
            show(&app, None);
        }
        match &mode {
            Mode::Command { until, .. } | Mode::FollowUp { until } if now > *until => {
                mode = Mode::Wake;
                vad.hangover = 25;
                show(&app, None);
            }
            _ => {}
        }
        // Dictation, recordings and conversations have the microphone: Groovy doesn't listen meanwhile.
        if live::dictating(&app) || crate::notes::is_recording(&app) || crate::calls::active() {
            vad.reset();
            continue;
        }
        let Some(chunk) = chunk else { continue };
        for segment in vad.feed(&chunk) {
            if segment.len() < speech::SAMPLE_RATE as usize * 35 / 100 {
                continue;
            }
            // Not loaded yet, or his name (to wake to) changed since: load it again.
            if (spotter.is_none() || spotter.as_ref().is_some_and(|k| k.stale())) && speech::kws::is_present() {
                spotter = speech::kws::Spotter::load();
            }
            let spotted = matches!(mode, Mode::Wake) && spotter.is_some();
            if spotted && !spotter.as_ref().is_some_and(|k| k.heard(&segment)) {
                continue;
            }
            if spotted {
                // Conversation: the cloud engines are warmed now, in parallel (round 3D item 10).
                crate::resources::state::fire(&app, crate::resources::state::Event::Wake);
                // Loads now, while the command is still being said.
                if !cloud_first(&app) {
                    speech::get_ready(&app);
                }
            }
            let Some(heard) = hear(&app, &segment) else { continue };
            if heard.is_empty() && !spotted {
                continue;
            }
            // The spotter heard the wake phrase even if the engine wrote it oddly.
            let wake_rest = wake::after_wake(&heard).or_else(|| spotted.then(|| wake::drop_wake_words(&heard)));
            let question = match &mut mode {
                Mode::Wake => match wake_rest {
                    Some(rest) => {
                        crate::log::line("voice: « OK Groovy » heard");
                        test_step(&app, "wake", true, "");
                        hide_at = None;
                        if crate::flow::text::word_count(&rest) >= 2 {
                            Some(rest)
                        } else {
                            show(&app, Some(VoiceView { phase: "listening".into(), heard: rest.clone(), answer: String::new() }));
                            mode = Mode::Command { text: rest, until: now + Duration::from_secs(7) };
                            // A question may come with pauses in it.
                            vad.hangover = 40;
                            None
                        }
                    }
                    None => None,
                },
                Mode::Command { text, .. } => {
                    let full = format!("{text} {heard}").trim().to_string();
                    Some(full)
                }
                Mode::FollowUp { .. } => Some(heard),
            };
            let Some(question) = question else { continue };
            mode = Mode::Wake;
            vad.hangover = 25;
            test_step(&app, "words", true, &question);
            show(&app, Some(VoiceView { phase: "thinking".into(), heard: question.clone(), answer: String::new() }));
            // "OK Groovy, let's talk": a conversation in the island (round 4 item 3).
            if crate::calls::is_talk(&question) {
                crate::log::line("voice: starting a conversation");
                show(&app, None);
                let _ = app.emit_to(crate::island::WINDOW_LABEL, "voice-call-start", ());
                continue;
            }
            // Music, volume, the time, timers, opening things: no AI needed.
            let answer = if let Some(b) = basic::parse(&question) {
                crate::log::line("voice: answered without AI");
                basic::run(&app, &b)
            } else if !crate::brain_ai::available() {
                let text = crate::i18n::t("I heard you, but I need an AI to answer that. Add a free Gemini key in Settings.");
                test_step(&app, "answer", false, &text);
                tell(&app, &question, text, Some((crate::i18n::t("Set up"), "openAssistant")));
                hide_at = Some(Instant::now() + Duration::from_secs(8));
                continue;
            } else {
                match brain.respond(&app, &question) {
                    Ok(r) if r.text.is_empty() && r.acted => "✓".to_string(),
                    Ok(r) if r.text.is_empty() => crate::i18n::t("I have no answer to that."),
                    Ok(r) => r.text,
                    Err(err) => {
                        let name = crate::brain_ai::active_name().unwrap_or("AI");
                        let text = if crate::brain_ai::is_limit(&err) {
                            crate::i18n::tf("{name} has reached its limit for now. Try again later, or add another AI in Settings.", &[("name", name)])
                        } else {
                            crate::i18n::tf("I couldn't reach {name}: {error}", &[("name", name), ("error", err.trim())])
                        };
                        test_step(&app, "answer", false, &text);
                        tell(&app, &question, text, Some((crate::i18n::t("Settings"), "openAssistant")));
                        hide_at = Some(Instant::now() + Duration::from_secs(8));
                        continue;
                    }
                }
            };
            test_step(&app, "answer", true, &answer);
            if answer == "✓" {
                replied(&app, &question);
                show(&app, Some(VoiceView { phase: "speaking".into(), heard: question, answer }));
                test_step(&app, "spoken", true, "");
                hide_at = Some(Instant::now() + Duration::from_secs(5));
                continue;
            }
            show(&app, Some(VoiceView { phase: "speaking".into(), heard: question.clone(), answer: answer.clone() }));
            speak(&app, &answer);
            test_step(&app, "spoken", true, "");
            replied(&app, &question);
            vad.reset();
            // Drop what the microphone heard of Groovy's own voice.
            while rx.try_recv().is_ok() {}
            if answer.trim_end().ends_with('?') {
                mode = Mode::FollowUp { until: Instant::now() + Duration::from_secs(8) };
                vad.hangover = 40;
                show(&app, Some(VoiceView { phase: "listening".into(), heard: String::new(), answer }));
            } else {
                hide_at = Some(Instant::now() + Duration::from_secs(6));
            }
        }
    }
    capture.stop();
    show(&app, None);
}

// ── Status and the test (Control Centre → OK Groovy, quick settings) ──────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OkStatus {
    /// Switched on in Settings.
    pub on: bool,
    pub listening: bool,
    pub speech: bool,
    /// The microphone it listens to (or would), and why it didn't open.
    pub mic: String,
    pub mic_error: Option<String>,
    /// The AI that answers first, if any.
    pub ai: Option<String>,
}

#[tauri::command]
pub fn okgroovy_status(app: AppHandle) -> OkStatus {
    let (prefs_on, mic_pref) = crate::flow::get(&app)
        .map(|f| {
            let s = f.store.lock().unwrap();
            (s.prefs.voice, s.prefs.mic.clone())
        })
        .unwrap_or_default();
    let (mut mic, mic_error) = MIC.lock().unwrap().clone();
    if mic.is_empty() {
        mic = if mic_pref.is_empty() {
            audio::input_devices().into_iter().find(|d| d.default).map(|d| d.name).unwrap_or_default()
        } else {
            mic_pref
        };
    }
    OkStatus {
        on: prefs_on,
        listening: is_listening(),
        speech: speech::is_ready(),
        mic,
        mic_error,
        ai: crate::brain_ai::active_name().map(str::to_string),
    }
}

/// "Test OK Groovy": the next « OK Groovy » reports each step for 30 seconds.
#[tauri::command]
pub fn okgroovy_test() -> Result<(), String> {
    if !is_listening() {
        return Err(if !speech::is_ready() {
            crate::i18n::t("Download the speech model first.")
        } else if MIC.lock().unwrap().1.is_some() {
            crate::i18n::t("The microphone didn't open: choose another one.")
        } else {
            crate::i18n::t("Turn on « OK Groovy » first.")
        });
    }
    *TEST_UNTIL.lock().unwrap() = Some(Instant::now() + Duration::from_secs(30));
    Ok(())
}

// ── Speaking ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Say {
    id: u64,
    text: String,
    lang: Option<&'static str>,
    /// Groovy's voice (Fish Audio's MP3, Kokoro's WAV) or ElevenLabs' MP3, base64.
    audio: Option<String>,
    mime: Option<&'static str>,
    gender: String,
}

/// Says `text` and returns once it is said (or after a sensible time).
fn speak(app: &AppHandle, text: &str) {
    let (output, gender) = crate::flow::get(app)
        .map(|f| {
            let p = &f.store.lock().unwrap().prefs;
            (p.voice_output.clone(), p.voice_gender.clone())
        })
        .unwrap_or_else(|| ("system".into(), "female".into()));
    if output == "off" {
        std::thread::sleep(Duration::from_millis(1500));
        return;
    }
    // Groovy's voice (Fish Audio, then Kokoro) sentence by sentence: the
    // next sentence is made while this one plays, so the first words come
    // quickly (round 3B item 8c). Without either, the whole answer goes as
    // before: ElevenLabs or the system voice in the island.
    let sentences = split_sentences(text);
    let (tx, rx) = mpsc::channel::<(String, Option<crate::speech::tts::Audio>)>();
    let app2 = app.clone();
    let first = crate::speech::tts::synthesize(app, &sentences[0]);
    if first.is_none() {
        let audio = if output == "elevenlabs" { elevenlabs(text, &gender).map(|mp3| base64(&mp3)) } else { None };
        say_one(app, text, audio, Some("audio/mpeg"), &gender);
        return;
    }
    let rest: Vec<String> = sentences[1..].to_vec();
    let _ = tx.send((sentences[0].clone(), first));
    std::thread::spawn(move || {
        for s in rest {
            let audio = crate::speech::tts::synthesize(&app2, &s);
            if tx.send((s, audio)).is_err() {
                return;
            }
        }
    });
    for (sentence, audio) in rx {
        let (data, mime) = match audio {
            Some(a) => (Some(base64(&a.bytes)), Some(a.mime)),
            None => (None, None),
        };
        say_one(app, &sentence, data, mime, &gender);
    }
}

/// Sends one piece to the island and waits until it has been said.
fn say_one(app: &AppHandle, text: &str, audio: Option<String>, mime: Option<&'static str>, gender: &str) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    pending().lock().unwrap().insert(id, tx);
    let _ = app.emit_to(crate::island::WINDOW_LABEL, "voice-say", Say { id, text: text.into(), lang: wake::speech_lang(text), audio, mime, gender: gender.into() });
    let words = crate::flow::text::word_count(text) as u64;
    let _ = rx.recv_timeout(Duration::from_millis(4000 + words * 600));
    pending().lock().unwrap().remove(&id);
}

/// An answer cut after each sentence (never empty).
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        let end = matches!(c, '.' | '!' | '?' | '。' | '！' | '？');
        let next_space = chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        // "9.05" or "v1.0" are not the end of a sentence.
        if end && next_space && cur.trim().chars().count() > 1 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    if out.is_empty() {
        out.push(text.trim().to_string());
    }
    out
}

/// The island says it has finished speaking (`id`), or the island answers a question.
#[tauri::command]
pub fn voice_done(id: u64, text: Option<String>) {
    if let Some(tx) = pending().lock().unwrap().remove(&id) {
        let _ = tx.send(text.unwrap_or_default());
    }
}

/// Where the webview has no voice (WebKitGTK often): speech-dispatcher or
/// espeak-ng, when installed.
#[tauri::command]
pub async fn voice_say_native(text: String, lang: Option<String>) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            let lang = lang.unwrap_or_default();
            let short = lang.split('-').next().unwrap_or("").to_string();
            if let Some(spd) = crate::platform::find_on_path("spd-say") {
                let mut cmd = std::process::Command::new(spd);
                cmd.arg("-w");
                if !short.is_empty() {
                    cmd.args(["-l", &short]);
                }
                return cmd.arg(&text).status().is_ok_and(|s| s.success());
            }
            if let Some(espeak) = crate::platform::find_on_path("espeak-ng") {
                let mut cmd = std::process::Command::new(espeak);
                if !short.is_empty() {
                    cmd.args(["-v", &short]);
                }
                return cmd.arg(&text).status().is_ok_and(|s| s.success());
            }
            false
        }
        #[cfg(windows)]
        {
            let _ = (text, lang);
            false
        }
    })
    .await
    .unwrap_or(false)
}

/// ElevenLabs' MP3 for `text`, with the user's key; None and the system voice speaks.
fn elevenlabs(text: &str, gender: &str) -> Option<Vec<u8>> {
    let key = crate::secrets::get("elevenlabs-api-key")?;
    tauri::async_runtime::block_on(async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().ok()?;
        let voices: Value = client.get("https://api.elevenlabs.io/v1/voices").header("xi-api-key", &key).send().await.ok()?.json().await.ok()?;
        let voice = pick_voice(voices["voices"].as_array()?, gender)?;
        let response = client
            .post(format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128"))
            .header("xi-api-key", &key)
            .header("accept", "audio/mpeg")
            .json(&json!({ "text": text, "model_id": "eleven_flash_v2_5" }))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            crate::log::line(format!("voice: ElevenLabs {}", response.status()));
            return None;
        }
        Some(response.bytes().await.ok()?.to_vec())
    })
}

/// First English voice of the gender (a young one first for a man); else any
/// voice of the gender; else the first one. Same choice as the Mac.
pub fn pick_voice(voices: &[Value], gender: &str) -> Option<String> {
    let label = |v: &Value, k: &str| v["labels"][k].as_str().unwrap_or("").to_lowercase();
    let english = |v: &Value| {
        let lang = label(v, "language");
        if !lang.is_empty() {
            return lang.starts_with("en");
        }
        let accent = label(v, "accent");
        accent.is_empty() || ["american", "british", "australian", "irish", "english", "us", "uk"].iter().any(|a| accent.contains(a))
    };
    let of_gender: Vec<&Value> = voices.iter().filter(|v| label(v, "gender") == gender).collect();
    let english_ones: Vec<&&Value> = of_gender.iter().filter(|v| english(v)).collect();
    let young = if gender == "male" { english_ones.iter().find(|v| label(v, "age").contains("young")).copied() } else { None };
    let v = young.or_else(|| english_ones.first().copied()).copied().or_else(|| of_gender.first().copied()).or_else(|| voices.first())?;
    v["voice_id"].as_str().map(String::from)
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

// ── The tools ─────────────────────────────────────────────────────────────────

/// Asks the island (it knows the pills and the services' data) and waits for its answer.
pub fn ask_island(app: &AppHandle, kind: &str, input: Value) -> Option<String> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    pending().lock().unwrap().insert(id, tx);
    let _ = app.emit_to(crate::island::WINDOW_LABEL, "voice-ask", json!({ "id": id, "kind": kind, "input": input }));
    let answer = rx.recv_timeout(Duration::from_secs(5)).ok();
    pending().lock().unwrap().remove(&id);
    answer
}

/// Runs one of Claude's tools: what goes back to Claude, and whether something changed.
pub fn run_tool(app: &AppHandle, name: &str, input: &Value) -> (String, bool) {
    match name {
        "pills" => {
            let answer = ask_island(app, "pills", input.clone()).unwrap_or_else(|| "the island did not answer".into());
            let acted = input["action"].as_str() != Some("list") && !answer.contains("unknown");
            (answer, acted)
        }
        "service_info" => (ask_island(app, "info", json!({})).unwrap_or_else(|| "no data".into()), false),
        "music" => music(app, input),
        "skills" => crate::skills::manage::voice_tool(app, input),
        "open_app" => open_app(input["name"].as_str().unwrap_or("")),
        "pc" => {
            use crate::pc::{Action, Change, Discord, WindowHow};
            let text = |k: &str| input[k].as_str().unwrap_or("").trim().to_string();
            let level = input["level"].as_u64().map(|n| n.min(100) as u8);
            let change = match (input["change"].as_str(), level) {
                (Some("up"), _) => Change::Up,
                (Some("down"), _) => Change::Down,
                (Some("mute"), _) => Change::Mute,
                (Some("unmute"), _) => Change::Unmute,
                (_, Some(n)) => Change::Set(n),
                _ => Change::Up,
            };
            let app = (!text("app").is_empty()).then(|| text("app"));
            let on = input["on"].as_bool().unwrap_or(true);
            let action = match input["action"].as_str().unwrap_or("") {
                "switch_to" => Action::SwitchTo(text("app")),
                "close" => Action::Close(text("app")),
                "minimise" => Action::Window { app, how: WindowHow::Minimise },
                "maximise" => Action::Window { app, how: WindowHow::Maximise },
                "snap_left" => Action::Window { app, how: WindowHow::SnapLeft },
                "snap_right" => Action::Window { app, how: WindowHow::SnapRight },
                "other_monitor" => Action::Window { app, how: WindowHow::OtherMonitor },
                "volume" => match level {
                    Some(n) => Action::Volume(n),
                    None => return ("volume needs a level from 0 to 100".into(), false),
                },
                "app_volume" => Action::AppVolume { app: text("app"), change },
                "output_device" => Action::OutputDevice,
                "discord_open" => Action::Discord(Discord::Open),
                "discord_mute" => Action::Discord(Discord::Mute),
                "discord_deafen" => Action::Discord(Discord::Deafen),
                "discord_link" => Action::Discord(Discord::Link(text("url"))),
                "spotify_play" => Action::SpotifyPlay(text("text")),
                "open_recent" => Action::OpenRecent((!text("text").is_empty()).then(|| text("text"))),
                "dark_mode" => Action::Theme { dark: true },
                "light_mode" => Action::Theme { dark: false },
                "do_not_disturb" => Action::DoNotDisturb,
                "night_light" => Action::NightLight,
                "wifi" => Action::Radio { wifi: true, on },
                "bluetooth" => Action::Radio { wifi: false, on },
                "brightness" => Action::Brightness(change),
                "set_clipboard" => Action::SetClipboard(text("text")),
                "type" => Action::Type(text("text")),
                "memory_use" => Action::MemoryUse,
                other => return (format!("unknown pc action {other}"), false),
            };
            match crate::tools::handle(crate::tools::Call::Pc(action)) {
                (answer, None) => (answer, true),
                (question, Some(_)) => (format!("Needs the user's permission first: {question}"), false),
            }
        }
        "browser" => {
            use crate::browser::Action;
            use crate::tools::Call;
            let text = |k: &str| input[k].as_str().unwrap_or("").trim().to_string();
            let call = match input["action"].as_str().unwrap_or("") {
                "open" => {
                    let url = text("url");
                    Call::OpenUrl(if url.starts_with("http://") || url.starts_with("https://") { url } else { format!("https://{url}") })
                }
                "search" => Call::WebSearch(text("query")),
                "new_tab" => Call::Browser(Action::NewTab),
                "close_tab" => Call::Browser(Action::CloseTab),
                "back" => Call::Browser(Action::Back),
                "forward" => Call::Browser(Action::Forward),
                "reload" => Call::Browser(Action::Reload),
                "next_tab" => Call::Browser(Action::NextTab),
                "previous_tab" => Call::Browser(Action::PreviousTab),
                "page" => Call::Browser(Action::Page),
                "press" => Call::Browser(Action::Press(text("name"))),
                other => return (format!("unknown browser action {other}"), false),
            };
            match crate::tools::handle(call) {
                (answer, None) => (answer, true),
                (question, Some(_)) => (format!("Needs the user's permission first: {question}"), false),
            }
        }
        "prepare_email" => {
            let to = input["recipient"].as_str().unwrap_or("").trim().to_string();
            if !to.contains('@') {
                return ("No email address: ask the user to spell it.".into(), false);
            }
            let enc = |s: &str| -> String {
                s.bytes()
                    .map(|b| match b {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                        _ => format!("%{b:02X}"),
                    })
                    .collect()
            };
            let url = format!(
                "mailto:{}?subject={}&body={}",
                enc(&to).replace("%40", "@"),
                enc(input["subject"].as_str().unwrap_or("")),
                enc(input["body"].as_str().unwrap_or(""))
            );
            crate::platform::open_url(&url);
            (format!("The mail app opened with the email to {to}. Nothing is sent until the user sends it."), true)
        }
        _ => (format!("unknown tool {name}"), false),
    }
}

fn music(app: &AppHandle, input: &Value) -> (String, bool) {
    let action = input["action"].as_str().unwrap_or("");
    let (control, value) = match action {
        "play_pause" => ("playPause", None),
        "next" => ("next", None),
        "previous" => ("previous", None),
        "set_volume" => ("volume", Some(input["volume"].as_f64().unwrap_or(50.0).clamp(0.0, 100.0))),
        "search" => {
            let q = input["query"].as_str().unwrap_or("").trim();
            if q.is_empty() {
                return ("missing query".into(), false);
            }
            let encoded: String = q.chars().map(|c| if c.is_alphanumeric() { c.to_string() } else { "%20".into() }).collect();
            crate::platform::open_url(&format!("spotify:search:{encoded}"));
            return (format!("Spotify opened on a search for {q}: the user picks what to play."), true);
        }
        _ => return ("unknown action".into(), false),
    };
    let app2 = app.clone();
    let done = tauri::async_runtime::block_on(crate::spotify::spotify_control(app2, control.into(), value));
    if done {
        ("done".into(), true)
    } else {
        ("Spotify is not running or not playing: tell the user to open it.".into(), false)
    }
}

/// Opens an app whose name matches: a Start menu shortcut on Windows, a
/// .desktop entry on Linux. Nothing else is ever run.
fn open_app(name: &str) -> (String, bool) {
    let wanted = wake::normalize(name);
    if wanted.is_empty() {
        return ("no name".into(), false);
    }
    match find_app(&wanted) {
        Some((label, launch)) => {
            launch();
            (format!("{label} opened"), true)
        }
        None => (format!("No app named {name} on this computer."), false),
    }
}

#[cfg(windows)]
fn find_app(wanted: &str) -> Option<(String, Box<dyn FnOnce()>)> {
    let mut roots = Vec::new();
    if let Some(p) = std::env::var_os("APPDATA") {
        roots.push(std::path::PathBuf::from(p).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Some(p) = std::env::var_os("ProgramData") {
        roots.push(std::path::PathBuf::from(p).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    let mut best: Option<(usize, std::path::PathBuf)> = None;
    let mut stack = roots;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                let stem = wake::normalize(&path.file_stem().unwrap_or_default().to_string_lossy());
                let score = if stem == wanted { 0 } else if stem.starts_with(wanted) { 1 } else if stem.contains(wanted) { 2 } else { continue };
                if best.as_ref().is_none_or(|(s, p)| score < *s || (score == *s && path.as_os_str().len() < p.as_os_str().len())) {
                    best = Some((score, path));
                }
            }
        }
    }
    let (_, path) = best?;
    let label = path.file_stem()?.to_string_lossy().to_string();
    Some((label, Box::new(move || crate::platform::reveal_folder(&path.to_string_lossy()))))
}

#[cfg(target_os = "linux")]
fn find_app(wanted: &str) -> Option<(String, Box<dyn FnOnce()>)> {
    let mut dirs = vec![std::path::PathBuf::from("/usr/share/applications"), std::path::PathBuf::from("/var/lib/flatpak/exports/share/applications")];
    dirs.push(crate::platform::home_dir().join(".local/share/applications"));
    dirs.push(crate::platform::home_dir().join(".local/share/flatpak/exports/share/applications"));
    let mut best: Option<(usize, String, String)> = None;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if text.lines().any(|l| l.trim() == "NoDisplay=true") {
                continue;
            }
            let Some(app_name) = text.lines().find_map(|l| l.strip_prefix("Name=")).map(str::trim) else { continue };
            let n = wake::normalize(app_name);
            let score = if n == wanted { 0 } else if n.starts_with(wanted) { 1 } else if n.contains(wanted) { 2 } else { continue };
            let id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if best.as_ref().is_none_or(|(s, _, _)| score < *s) {
                best = Some((score, app_name.to_string(), id));
            }
        }
    }
    let (_, label, id) = best?;
    Some((label, Box::new(move || {
        let _ = std::process::Command::new("gtk-launch").arg(&id).spawn();
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_spoken_sentence_by_sentence() {
        assert_eq!(split_sentences("It's 9.05 now. Shall I set a timer? Done!"), vec!["It's 9.05 now.", "Shall I set a timer?", "Done!"]);
        assert_eq!(split_sentences("No full stop"), vec!["No full stop"]);
        assert_eq!(split_sentences(""), vec![""]);
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn the_voice_is_picked_like_on_the_mac() {
        let voices: Vec<Value> = serde_json::from_str(r#"[
            {"voice_id":"a","labels":{"gender":"male","accent":"american","age":"middle aged"}},
            {"voice_id":"b","labels":{"gender":"male","accent":"british","age":"young"}},
            {"voice_id":"c","labels":{"gender":"female","accent":"american"}},
            {"voice_id":"d","labels":{"gender":"female","language":"fr"}}
        ]"#).unwrap();
        assert_eq!(pick_voice(&voices, "male").as_deref(), Some("b"));
        assert_eq!(pick_voice(&voices, "female").as_deref(), Some("c"));
        assert_eq!(pick_voice(&[], "female"), None);
    }
}

/// With the models downloaded: `GROOVY_TEST_WAKE_WAV=… cargo test -- --ignored wake_in`.
#[cfg(test)]
mod with_models {
    #[test]
    #[ignore]
    fn hears_the_wake_phrase_in_a_recording() {
        let path = std::env::var("GROOVY_TEST_WAKE_WAV").expect("GROOVY_TEST_WAKE_WAV");
        let bytes = std::fs::read(&path).unwrap();
        let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let pcm: Vec<f32> = bytes[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
        let mut r = crate::speech::audio::Resampler::new(rate);
        let mut samples = Vec::new();
        r.process(&pcm, &mut samples);
        let mut vad = super::wake::Vad::default();
        let mut heard = Vec::new();
        for chunk in samples.chunks(1600) {
            for segment in vad.feed(chunk) {
                let text = crate::speech::transcribe(&segment).expect("models").text;
                println!("segment {:.2} s: {text}", segment.len() as f32 / 16_000.0);
                heard.push(super::wake::after_wake(&text));
            }
        }
        println!("{heard:?}");
        assert!(heard.iter().any(|h| h.as_deref().is_some_and(|q| !q.is_empty())));
    }
}
