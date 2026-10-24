// Voice: speech to text with Whisper (whisper.cpp through whisper-rs), on this
// computer. The island's VoiceController sends short 16 kHz segments; nothing
// is recorded or kept. Models are downloaded once, on request, from the
// whisper.cpp model repository on Hugging Face, into local_dir()/models.
//
// Built with the "voice" Cargo feature (on by default). Without it, voice
// reports itself unavailable and every other part of Groovy works as before.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::platform;

/// The models Groovy offers. Anything else is refused. The ".en" ones only
/// understand English; the others understand ~99 languages (Spanish, German,
/// Punjabi, Hindi…), less accurately per size. The "-q5_1" ones are the same
/// models quantized: a third of the size and memory (round 3D item 7).
pub const MODELS: &[&str] = &["base-q5_1", "base.en-q5_1", "tiny.en", "base.en", "tiny", "base", "small"];

/// Groovy's offline speech model by default (round 3D item 7): Whisper base,
/// quantized, ~99 languages, about 57 MB, so Groovy stays under 400 MB.
pub const DEFAULT_MODEL: &str = "base-q5_1";

/// What each download must be, from the whisper.cpp repository on Hugging Face:
/// (model, size in bytes, SHA-256). A file that doesn't match is deleted.
const PINNED: &[(&str, u64, &str)] = &[
    ("base-q5_1", 59_707_625, "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898"),
    ("base.en-q5_1", 59_721_011, "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f"),
    ("tiny.en", 77_704_715, "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"),
    ("base.en", 147_964_211, "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002"),
    ("tiny", 77_691_713, "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21"),
    ("base", 147_951_465, "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"),
    ("small", 487_601_967, "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
];

fn pinned(model: &str) -> Option<(u64, &'static str)> {
    PINNED.iter().find(|(m, _, _)| *m == model).map(|(_, size, sha)| (*size, *sha))
}

/// A model that only understands English ("base.en", "base.en-q5_1").
pub fn english_only(model: &str) -> bool {
    model.ends_with(".en") || model.contains(".en-")
}

/// The language Whisper is told to expect: English for the ".en" models,
/// otherwise the one picked in Settings (an ISO 639-1 code), or None to let
/// it detect the language itself.
pub fn language_for(model: &str, picked: &str) -> Option<String> {
    if english_only(model) {
        return Some("en".into());
    }
    let code = picked.trim().to_lowercase();
    let valid = (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase());
    (valid && code != "auto").then_some(code)
}

/// The smallest a real model file can be: a cut-off download is never loaded.
const MIN_MODEL_BYTES: u64 = 30 * 1024 * 1024;

/// Longest segment accepted, in samples (30 s at 16 kHz) — Whisper's window.
const MAX_SAMPLES: usize = 16_000 * 30;

#[derive(Serialize)]
pub struct VoiceStatus {
    pub ready: bool,
    pub installed: Vec<String>,
    pub available: bool,
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    model: &'a str,
    done: u64,
    total: u64,
}

fn models_dir() -> PathBuf {
    platform::local_dir().join("models")
}

fn model_path(model: &str) -> Result<PathBuf, String> {
    if !MODELS.contains(&model) {
        return Err(format!("Unknown speech model \"{model}\"."));
    }
    Ok(models_dir().join(format!("ggml-{model}.bin")))
}

pub fn installed(model: &str) -> bool {
    model_path(model)
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .is_some_and(|m| m.len() >= MIN_MODEL_BYTES)
}

pub fn status(chosen: &str) -> VoiceStatus {
    let installed_list: Vec<String> = MODELS.iter().filter(|m| installed(m)).map(|m| m.to_string()).collect();
    let available = cfg!(feature = "voice");
    VoiceStatus {
        ready: available && installed(chosen),
        installed: installed_list,
        available,
    }
}

/// Downloads a model, reporting progress as "voice-model-progress" events. The
/// file is written next to its final place and renamed only once complete.
pub async fn download(app: &AppHandle, model: &str) -> Result<(), String> {
    let path = model_path(model)?;
    std::fs::create_dir_all(models_dir()).map_err(|e| e.to_string())?;
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin");
    let mut resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("The download didn't start ({e})."))?;
    let total = resp.content_length().unwrap_or(0);
    let part = path.with_extension("bin.part");
    let (expected_size, expected_sha) = pinned(model).ok_or("Unknown speech model.")?;
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut done: u64 = 0;
    let mut last_report: u64 = 0;
    loop {
        let chunk = match resp.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Err(format!("The download stopped ({e})."));
            }
        };
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
        hash.update(&chunk);
        done += chunk.len() as u64;
        if done > expected_size {
            let _ = std::fs::remove_file(&part);
            return Err("The download is larger than expected; it was discarded.".into());
        }
        // About every 2 MB is plenty for a progress bar.
        if done - last_report > 2 * 1024 * 1024 || done == total {
            last_report = done;
            let _ = app.emit("voice-model-progress", Progress { model, done, total });
        }
    }
    drop(file);
    if done < MIN_MODEL_BYTES || (total > 0 && done != total) || done != expected_size {
        let _ = std::fs::remove_file(&part);
        return Err("The downloaded file is incomplete.".into());
    }
    let got: String = hash.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect();
    if got != expected_sha {
        let _ = std::fs::remove_file(&part);
        return Err("The speech model failed verification (SHA-256 mismatch); it was discarded.".into());
    }
    std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    // A model replaced on disk must be loaded again.
    *LOADED.lock().unwrap() = None;
    Ok(())
}

pub fn delete(model: &str) -> Result<(), String> {
    let path = model_path(model)?;
    *LOADED.lock().unwrap() = None;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// 16-bit samples to the -1…1 floats Whisper takes.
pub fn to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

/// Whisper's text, tidied: no "[BLANK_AUDIO]"-style markers, single spaces.
#[cfg(any(feature = "voice", test))]
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Whisper ──────────────────────────────────────────────────────────────────

#[cfg(feature = "voice")]
type Loaded = (String, whisper_rs::WhisperContext);
#[cfg(not(feature = "voice"))]
type Loaded = (String, ());

/// The model in memory, by name: loaded on the first segment, kept while used.
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

/// When Whisper was last used: freed once unused for the idle setting (round 3B item 10).
static WHISPER_USED: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Whether a Whisper model is in memory.
pub fn is_loaded() -> bool {
    LOADED.try_lock().map(|l| l.is_some()).unwrap_or(true)
}

/// Frees Whisper now unless it is transcribing (memory pressure, round 3D item 7).
pub fn free_now() {
    if let Ok(mut loaded) = LOADED.try_lock() {
        if loaded.take().is_some() {
            crate::log::line("Whisper freed (memory)");
        }
    }
}

/// Frees Whisper once unused for the idle setting (speech::spawn_idle_reaper).
pub fn free_if_idle() {
    let idle = WHISPER_USED.lock().unwrap().is_some_and(crate::idle::expired);
    // A transcription in progress holds the lock: try again on the next check.
    if let (true, Ok(mut loaded)) = (idle, LOADED.try_lock()) {
        if loaded.take().is_some() {
            crate::log::line("Whisper freed");
        }
    }
}

/// Transcribes up to 30 s of 16 kHz audio, on the calling thread: for
/// dictation and notes when Whisper is chosen over Parakeet (speech/mod.rs).
/// The dictation vocabulary Whisper is prompted with (Flow's dictionary).
static VOCABULARY: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn set_vocabulary(words: &[String]) {
    *VOCABULARY.lock().unwrap() = words.to_vec();
}

pub fn transcribe_blocking(model: &str, language: Option<&str>, pcm: &[f32]) -> Result<String, String> {
    run(model, language, pcm, false)
}

/// Transcribes one segment. Runs on a blocking thread (it takes a moment).
pub async fn transcribe(model: String, language: Option<String>, samples: Vec<i16>, wake: bool) -> Result<String, String> {
    if samples.is_empty() {
        return Ok(String::new());
    }
    if samples.len() > MAX_SAMPLES {
        return Err("That was too long to listen to in one go.".into());
    }
    tauri::async_runtime::spawn_blocking(move || run(&model, language.as_deref(), &to_f32(&samples), wake))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(feature = "voice")]
fn run(model: &str, language: Option<&str>, pcm: &[f32], wake: bool) -> Result<String, String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let path = model_path(model)?;
    if !installed(model) {
        return Err("Download a speech model in Settings → Voice first.".into());
    }
    *WHISPER_USED.lock().unwrap() = Some(std::time::Instant::now());
    let mut guard = LOADED.lock().unwrap();
    if guard.as_ref().map(|(name, _)| name.as_str()) != Some(model) {
        crate::ceiling::make_room("speech", crate::ceiling::WHISPER_MB);
        crate::loads::note(crate::loads::Part::OfflineStt, "Whisper, for speech on this PC");
        let ctx = WhisperContext::new_with_params(&path.to_string_lossy(), WhisperContextParameters::default())
            .map_err(|e| format!("The speech model couldn't be loaded ({e})."))?;
        *guard = Some((model.to_string(), ctx));
    }
    let (_, ctx) = guard.as_ref().unwrap();
    let mut state = ctx.create_state().map_err(|e| e.to_string())?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // The wake word is always listened for as English: "Hey Groovy".
    params.set_language(if wake { Some("en") } else { language });
    params.set_translate(false);
    params.set_no_context(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    // The name, so Whisper spells it right, then the user's vocabulary (round 3B item 6).
    let prompt = crate::flow::vocab::whisper_prompt(&VOCABULARY.lock().unwrap());
    params.set_initial_prompt(&prompt);
    params.set_single_segment(wake);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    params.set_n_threads(threads.clamp(1, 8) as i32);

    state.full(params, pcm).map_err(|e| e.to_string())?;
    let n = state.full_n_segments().map_err(|e| e.to_string())?;
    let mut text = String::new();
    for i in 0..n {
        if let Ok(segment) = state.full_get_segment_text(i) {
            text.push_str(&segment);
            text.push(' ');
        }
    }
    Ok(clean(&text))
}

#[cfg(not(feature = "voice"))]
fn run(_model: &str, _language: Option<&str>, _pcm: &[f32], _wake: bool) -> Result<String, String> {
    Err("This build of Groovy has no speech engine.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_offered_models_have_a_path() {
        assert!(model_path("base.en").unwrap().ends_with("ggml-base.en.bin"));
        assert!(model_path("../../evil").is_err());
        assert!(model_path("large").is_err());
    }

    #[test]
    fn every_offered_model_is_pinned() {
        for m in MODELS {
            let (size, sha) = pinned(m).unwrap_or_else(|| panic!("{m} not pinned"));
            assert!(size >= MIN_MODEL_BYTES);
            assert_eq!(sha.len(), 64);
        }
        assert!(pinned("large").is_none());
    }

    #[test]
    fn english_models_always_hear_english() {
        assert_eq!(language_for("base.en", "es").as_deref(), Some("en"));
        assert_eq!(language_for("base.en-q5_1", "es").as_deref(), Some("en"));
        assert_eq!(language_for("base-q5_1", "es").as_deref(), Some("es"));
        assert!(model_path(DEFAULT_MODEL).unwrap().ends_with("ggml-base-q5_1.bin"));
        assert_eq!(language_for("base", "es").as_deref(), Some("es"));
        assert_eq!(language_for("small", "pa").as_deref(), Some("pa"));
        assert_eq!(language_for("base", "").as_deref(), None, "empty = detect");
        assert_eq!(language_for("base", "auto").as_deref(), None);
        assert_eq!(language_for("base", "e$").as_deref(), None);
        assert_eq!(language_for("base", "english").as_deref(), None);
    }

    #[test]
    fn whisper_markers_and_extra_spaces_are_removed() {
        assert_eq!(clean(" [BLANK_AUDIO] "), "");
        assert_eq!(clean(" Hey Groovy,  (music) what's up? "), "Hey Groovy, what's up?");
    }

    #[test]
    fn samples_become_floats() {
        assert_eq!(to_f32(&[0, 16384, -32768]), vec![0.0, 0.5, -1.0]);
    }
}
