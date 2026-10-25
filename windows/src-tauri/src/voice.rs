// Voice: speech to text with Whisper on this computer, through sherpa-onnx,
// the speech engine Groovy already has for Parakeet, the wake word and Kokoro
// (round 3F item 5: one speech runtime; whisper.cpp is gone). The island's
// VoiceController sends short 16 kHz segments; nothing is recorded or kept.
// Models are downloaded once, on request, into local_dir()/models.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig};
use tauri::{AppHandle, Emitter};

/// The models Groovy offers. Anything else is refused. The ".en" ones only
/// understand English; the others understand ~99 languages (Spanish, German,
/// Punjabi, Hindi…), less accurately per size. All are int8.
pub const MODELS: &[&str] = &["base", "base.en", "tiny", "tiny.en", "small"];

/// Groovy's offline speech model by default (round 3D item 7): Whisper base,
/// ~99 languages, its weights mapped from disk so Groovy stays under 400 MB.
pub const DEFAULT_MODEL: &str = "base";

/// Models chosen before round 3F item 5 (whisper.cpp's files), and what they
/// are now. Their old files are deleted once (`retire_whisper_cpp`).
pub fn current_name(model: &str) -> &str {
    match model {
        "base-q5_1" => "base",
        "base.en-q5_1" => "base.en",
        m => m,
    }
}

fn whisper_cpp_files() -> Vec<std::fs::DirEntry> {
    let Ok(dir) = std::fs::read_dir(crate::speech::models_dir()) else { return Vec::new() };
    dir.flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("ggml-") && (name.ends_with(".bin") || name.ends_with(".bin.part"))
        })
        .collect()
}

/// A whisper.cpp model was downloaded before round 3F item 5.
pub fn had_whisper_cpp() -> bool {
    !whisper_cpp_files().is_empty()
}

/// whisper.cpp's model files (ggml-*.bin), no longer read by anything: deleted
/// once to give the disk back.
pub fn retire_whisper_cpp() {
    for entry in whisper_cpp_files() {
        let name = entry.file_name().to_string_lossy().to_string();
        {
            if std::fs::remove_file(entry.path()).is_ok() {
                crate::log::line(format!("voice: removed {name} (whisper.cpp's, no longer used)"));
            }
        }
    }
}

/// A model that only understands English ("base.en", "tiny.en").
pub fn english_only(model: &str) -> bool {
    model.ends_with(".en")
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

fn set_of(model: &str) -> Result<&'static crate::speech::ModelSet, String> {
    crate::speech::whisper_set(model).ok_or_else(|| format!("Unknown speech model \"{model}\"."))
}

pub fn installed(model: &str) -> bool {
    set_of(model).is_ok_and(|s| s.is_present())
}

pub fn status(chosen: &str) -> VoiceStatus {
    let installed_list: Vec<String> = MODELS.iter().filter(|m| installed(m)).map(|m| m.to_string()).collect();
    VoiceStatus { ready: installed(chosen), installed: installed_list, available: true }
}

/// Downloads a model, reporting progress as "voice-model-progress" events.
/// Each file is checked against its size and SHA-256 before it takes its name.
pub async fn download(app: &AppHandle, model: &str) -> Result<(), String> {
    let set = set_of(model)?;
    let total = set.total_size();
    let mut last_report = 0;
    let app2 = app.clone();
    let name = model.to_string();
    crate::speech::fetch(set, &mut move |done| {
        // About every 2 MB is plenty for a progress bar.
        if done - last_report > 2 * 1024 * 1024 || done == total {
            last_report = done;
            let _ = app2.emit("voice-model-progress", Progress { model: &name, done, total });
        }
    })
    .await
    .map_err(|e| format!("The speech model didn't download ({e})."))?;
    // A model replaced on disk must be loaded again.
    *LOADED.lock().unwrap() = None;
    Ok(())
}

pub fn delete(model: &str) -> Result<(), String> {
    let set = set_of(model)?;
    *LOADED.lock().unwrap() = None;
    match std::fs::remove_dir_all(set.dir()) {
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

/// The recognizer in memory, by model and language ("base/en"): sherpa-onnx
/// fixes Whisper's language when it loads.
type Loaded = (String, OfflineRecognizer);

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

/// Frees Whisper once unused for the idle setting (the resources thread).
pub fn free_if_idle() {
    let idle = WHISPER_USED.lock().unwrap().is_some_and(crate::resources::idle::expired);
    // A transcription in progress holds the lock: try again on the next check.
    if let (true, Ok(mut loaded)) = (idle, LOADED.try_lock()) {
        if loaded.take().is_some() {
            crate::log::line("Whisper freed");
        }
    }
}

/// Transcribes up to 30 s of 16 kHz audio, on the calling thread: for
/// dictation and notes when Whisper is chosen over Parakeet (speech/mod.rs).
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

/// Encoder, decoder and tokens of a model's set.
fn files(set: &crate::speech::ModelSet, model: &str) -> (String, String, String) {
    (set.path(&format!("{model}-encoder.int8.onnx")), set.path(&format!("{model}-decoder.int8.onnx")), set.path(&format!("{model}-tokens.txt")))
}

/// A weights file mapped from disk if it can be (speech/mapped.rs), else as it is.
fn mapped(path: String) -> String {
    crate::speech::mapped::mapped_model(Path::new(&path)).map_or(path, |p| p.display().to_string())
}

fn load(model: &str, language: Option<&str>) -> Result<OfflineRecognizer, String> {
    let set = set_of(model)?;
    let (encoder, decoder, tokens) = files(set, model);
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.whisper = OfflineWhisperModelConfig {
        encoder: Some(mapped(encoder)),
        decoder: Some(mapped(decoder)),
        // Empty: Whisper finds the language itself.
        language: Some(language.unwrap_or("").to_string()),
        task: Some("transcribe".into()),
        tail_paddings: -1,
        ..Default::default()
    };
    config.model_config.tokens = Some(tokens);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    config.model_config.num_threads = threads.clamp(1, 4) as i32;
    config.model_config.provider = crate::speech::low_memory_provider();
    OfflineRecognizer::create(&config).ok_or_else(|| "The speech model couldn't be loaded.".to_string())
}

fn run(model: &str, language: Option<&str>, pcm: &[f32], wake: bool) -> Result<String, String> {
    let model = current_name(model);
    if !installed(model) {
        return Err("Download a speech model in Settings → Voice first.".into());
    }
    // The wake word is always listened for as English: "Hey Groovy".
    let language = if wake { Some("en") } else { language };
    let key = format!("{model}/{}", language.unwrap_or(""));
    *WHISPER_USED.lock().unwrap() = Some(std::time::Instant::now());
    let mut guard = LOADED.lock().unwrap();
    if guard.as_ref().map(|(k, _)| k.as_str()) != Some(key.as_str()) {
        // The one in memory goes first: never two Whispers at once.
        *guard = None;
        crate::resources::ceiling::make_room("speech", crate::resources::ceiling::WHISPER_MB);
        crate::resources::loads::note(crate::resources::loads::Part::OfflineStt, "Whisper, for speech on this PC");
        crate::resources::register("whisper", free_if_idle);
        *guard = Some((key, load(model, language)?));
    }
    let (_, recognizer) = guard.as_ref().unwrap();
    let stream = recognizer.create_stream();
    stream.accept_waveform(crate::speech::SAMPLE_RATE as i32, pcm);
    recognizer.decode(&stream);
    Ok(clean(&stream.get_result().map(|r| r.text).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_offered_models_have_files() {
        assert!(set_of("base.en").unwrap().dir().ends_with("whisper-base.en"));
        assert!(set_of("../../evil").is_err());
        assert!(set_of("large").is_err());
    }

    #[test]
    fn every_offered_model_is_pinned_encoder_decoder_and_tokens() {
        for m in MODELS {
            let set = set_of(m).unwrap();
            let (encoder, decoder, tokens) = files(set, m);
            for f in [encoder, decoder, tokens] {
                assert!(set.files.iter().any(|x| set.path(x.name) == f && x.sha256.len() == 64 && x.url.contains("/resolve/")), "{f}");
            }
        }
    }

    #[test]
    fn whisper_cpp_choices_move_to_their_twins() {
        assert_eq!(current_name("base-q5_1"), "base");
        assert_eq!(current_name("base.en-q5_1"), "base.en");
        assert_eq!(current_name("small"), "small");
        assert!(MODELS.contains(&current_name(DEFAULT_MODEL)));
    }

    #[test]
    fn english_models_always_hear_english() {
        assert_eq!(language_for("base.en", "es").as_deref(), Some("en"));
        assert_eq!(language_for("tiny.en", "es").as_deref(), Some("en"));
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

    /// Downloads Whisper base and hears CI's three clips, as dictation and
    /// « OK Groovy » do: `cargo test -p groovy whisper_base_hears -- --ignored`.
    #[test]
    #[ignore]
    fn whisper_base_hears_the_dictation_and_the_wake_phrase() {
        tauri::async_runtime::block_on(crate::speech::fetch(set_of(DEFAULT_MODEL).unwrap(), &mut |_| {})).expect("download");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/audio");
        let hear = |name: &str| {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let data = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
            let pcm: Vec<f32> = bytes[data..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
            let started = std::time::Instant::now();
            let text = transcribe_blocking(DEFAULT_MODEL, Some("en"), &pcm).unwrap();
            println!("{name}: “{text}” in {:?}", started.elapsed());
            text.to_lowercase()
        };
        assert!(hear("dictation.wav").contains("send the report to sam"));
        assert!(hear("ok-groovy-time.wav").contains("groovy"));
        assert!(!hear("not-wake.wav").contains("groovy"));
    }
}
