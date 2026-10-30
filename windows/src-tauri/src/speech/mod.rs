// Speech to text on this computer: NVIDIA's Parakeet (25 languages) through
// sherpa-onnx, and the small models that tell the voices of a call apart. The
// Mac runs the same Parakeet through FluidAudio (ParakeetEngine.swift).
//
// Nothing is downloaded until the user clicks Download in Groovy → Settings.
// The files come from Hugging Face at a pinned revision, are checked against
// their SHA-256 and kept in %LOCALAPPDATA%\Groovy\Models (Windows) or
// ~/.local/share/groovy/Models (Linux). From then on everything runs offline.

pub mod audio;
pub mod check;
pub mod cloud;
mod kokoro_files;
pub mod mapped;
mod whisper_files;
pub use whisper_files::set as whisper_set;
pub mod kws;
pub mod tts;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
    OfflineTransducerModelConfig,
};
use tauri::{AppHandle, Emitter};

use crate::notes::builder::{TimedWord, Turn};

/// Every model works on 16 kHz mono.
pub const SAMPLE_RATE: u32 = 16_000;

pub(crate) struct ModelFile {
    pub(crate) name: &'static str,
    pub(crate) url: &'static str,
    size: u64,
    pub(crate) sha256: &'static str,
}

/// A set of files downloaded together.
pub struct ModelSet {
    pub id: &'static str,
    pub(crate) files: &'static [ModelFile],
}

impl ModelSet {
    pub fn dir(&self) -> PathBuf {
        models_dir().join(self.id)
    }

    pub(crate) fn path(&self, name: &str) -> String {
        self.dir().join(name).to_string_lossy().to_string()
    }

    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Every file is there with its size (the hash was checked when it came in).
    pub fn is_present(&self) -> bool {
        self.files.iter().all(|f| std::fs::metadata(self.dir().join(f.name)).map(|m| m.len() == f.size).unwrap_or(false))
    }
}

/// Parakeet TDT 0.6B v3, int8: the dictation and the calls' words (about 670 MB).
pub static PARAKEET: ModelSet = ModelSet {
    id: "parakeet-tdt-0.6b-v3-int8",
    files: &[
        ModelFile {
            name: "encoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/encoder.int8.onnx",
            size: 652_184_281,
            sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
        },
        ModelFile {
            name: "decoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/decoder.int8.onnx",
            size: 11_845_275,
            sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
        },
        ModelFile {
            name: "joiner.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/joiner.int8.onnx",
            size: 6_355_277,
            sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
        },
        ModelFile {
            name: "tokens.txt",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/tokens.txt",
            size: 93_939,
            sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
        },
    ],
};

/// pyannote's segmentation and a speaker embedding model: who speaks when, in a
/// call (about 30 MB).
pub static DIARIZER: ModelSet = ModelSet {
    id: "diarizer",
    files: &[
        ModelFile {
            name: "segmentation.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-pyannote-segmentation-3-0/resolve/9403a6902bb58e3d5ae8c7e77c3422de279db2e0/model.int8.onnx",
            size: 1_540_506,
            sha256: "d582f4b4c6b48205de7e0643c57df0df5615a3c176189be3fc461e9d18827b5d",
        },
        ModelFile {
            name: "embedding.onnx",
            url: "https://huggingface.co/csukuangfj/speaker-embedding-models/resolve/0743f301363dec56491a490f6d6cbc9d67f9a3bf/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
            size: 28_281_164,
            sha256: "aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2",
        },
    ],
};

pub fn models_dir() -> PathBuf {
    crate::settings::local_dir().join("Models")
}

// ── Download state ────────────────────────────────────────────────────────────

/// What the Settings page shows about the models.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ModelState {
    Missing,
    Downloading { fraction: f64 },
    Ready,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsStatus {
    pub parakeet: ModelState,
    /// Size of the whole download in MB, both sets.
    pub download_mb: u64,
}

struct Downloads {
    running: bool,
    fraction: f64,
    error: Option<String>,
}

fn downloads() -> &'static Mutex<Downloads> {
    static D: OnceLock<Mutex<Downloads>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(Downloads { running: false, fraction: 0.0, error: None }))
}

/// Both sets count as one download: dictation and the Notetaker are ready together.
pub fn status() -> ModelsStatus {
    let d = downloads().lock().unwrap();
    let parakeet = if d.running {
        ModelState::Downloading { fraction: d.fraction }
    } else if PARAKEET.is_present() && DIARIZER.is_present() {
        ModelState::Ready
    } else if let Some(error) = &d.error {
        ModelState::Failed { error: error.clone() }
    } else {
        ModelState::Missing
    };
    ModelsStatus { parakeet, download_mb: (PARAKEET.total_size() + DIARIZER.total_size()) / 1_000_000 + kws::download_mb() }
}

/// Whisper instead of Parakeet, for the languages Parakeet doesn't know
/// (Punjabi, Hindi, Japanese, Chinese…): (model, language). Set from Flow's
/// preferences and the voice settings (flow::sync_engine).
static WHISPER: Mutex<Option<(String, Option<String>)>> = Mutex::new(None);

pub fn use_whisper(choice: Option<(String, Option<String>)>) {
    *WHISPER.lock().unwrap() = choice;
}

fn whisper() -> Option<(String, Option<String>)> {
    WHISPER.lock().unwrap().clone()
}

/// Whether speech can be turned into text now, with the engine chosen.
pub fn is_ready() -> bool {
    match whisper() {
        Some((model, _)) => crate::voice::installed(&model),
        None => PARAKEET.is_present(),
    }
}

/// Whisper gives text without word times: the words are spread evenly over
/// the stretch, which is enough to place them in a speaker's turn.
pub fn spread_words(text: &str, start: f64, length: f64) -> Vec<TimedWord> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let step = length / words.len() as f64;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| TimedWord { text: (*w).to_string(), start: start + i as f64 * step, end: start + (i + 1) as f64 * step })
        .collect()
}

fn transcribe_whisper(model: &str, language: Option<&str>, samples: &[f32]) -> Option<Transcript> {
    let mut out = Transcript::default();
    for (offset, chunk) in audio::split_for_model(samples, SAMPLE_RATE, MAX_CHUNK_SECONDS) {
        if chunk.len() < (SAMPLE_RATE as usize) / 4 {
            continue;
        }
        let text = match crate::voice::transcribe_blocking(model, language, chunk) {
            Ok(t) => t,
            Err(e) => {
                crate::log::line(format!("speech: whisper failed: {e}"));
                return None;
            }
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        if !out.text.is_empty() {
            out.text.push(' ');
        }
        out.text.push_str(text);
        out.words.extend(spread_words(text, offset as f64 / SAMPLE_RATE as f64, chunk.len() as f64 / SAMPLE_RATE as f64));
    }
    Some(out)
}

fn emit_status(app: &AppHandle) {
    let _ = app.emit("speech-models", status());
    crate::okgroovy::sync(app);
}

/// Downloads what is missing, with the progress in `speech-models` events.
pub fn download(app: AppHandle) {
    {
        let mut d = downloads().lock().unwrap();
        if d.running {
            return;
        }
        d.running = true;
        d.fraction = 0.0;
        d.error = None;
    }
    emit_status(&app);
    tauri::async_runtime::spawn(async move {
        let result = download_all(&app).await;
        {
            let mut d = downloads().lock().unwrap();
            d.running = false;
            if let Err(err) = &result {
                d.error = Some(err.clone());
            }
        }
        match result {
            Ok(()) => {
                crate::log::line("speech models downloaded");
            }
            Err(err) => crate::log::line(format!("speech models download failed: {err}")),
        }
        emit_status(&app);
    });
}

pub(crate) async fn download_all(app: &AppHandle) -> Result<(), String> {
    let total = PARAKEET.total_size() + DIARIZER.total_size();
    let mut done: u64 = 0;
    let client = download_client()?;
    let mut last_emit = Instant::now();
    for set in [&PARAKEET, &DIARIZER] {
        fetch_set(&client, set, &mut done, &mut |got| {
            if last_emit.elapsed() > Duration::from_millis(250) {
                last_emit = Instant::now();
                downloads().lock().unwrap().fraction = got as f64 / total as f64;
                emit_status(app);
            }
        })
        .await?;
    }
    // « OK Groovy »'s ear comes with them (round 3B item 10).
    kws::download(&client).await?;
    Ok(())
}

/// The models came before the wake word spotter did: it follows on its own
/// when « OK Groovy » is turned on (the speech models were already agreed to).
pub fn fetch_kws() {
    static FETCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if kws::is_present() || !PARAKEET.is_present() || FETCHING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async {
        let result = match download_client() {
            Ok(client) => kws::download(&client).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => crate::log::line("voice: wake word spotter downloaded"),
            Err(e) => crate::log::line(format!("voice: wake word spotter download failed: {e}")),
        }
        FETCHING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

pub fn download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("Groovy/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// onnxruntime without weight prepacking, which keeps a second, repacked copy
/// of a model's weights in private memory (14 MB for Kokoro, more for Whisper)
/// for no speed it can measure here. Passed as sherpa-onnx's "cpu:<file>"
/// provider; sherpa-onnx exits when the file is missing, so it is written
/// first and left out if it can't be.
pub(crate) fn low_memory_provider() -> Option<String> {
    let dir = crate::settings::local_dir();
    let file = dir.join("onnxruntime-low-memory.cfg");
    crate::platform::ensure_private_dir(&dir).ok()?;
    std::fs::write(&file, "SessionConfig.session.disable_prepacking=1\n").ok()?;
    file.is_file().then(|| format!("cpu:{}", file.display()))
}

/// Downloads `set`'s missing files; `progress` gets the bytes done so far.
pub(crate) async fn fetch(set: &ModelSet, progress: &mut (dyn FnMut(u64) + Send)) -> Result<(), String> {
    let client = download_client()?;
    let mut done = 0;
    fetch_set(&client, set, &mut done, progress).await
}

/// Downloads a set's missing files, each checked against its size and
/// SHA-256 before it takes its name. `done` counts bytes across sets.
async fn fetch_set(client: &reqwest::Client, set: &ModelSet, done: &mut u64, progress: &mut (dyn FnMut(u64) + Send)) -> Result<(), String> {
    let dir = set.dir();
    for file in set.files {
        let path = dir.join(file.name);
        if std::fs::metadata(&path).map(|m| m.len() == file.size).unwrap_or(false) {
            *done += file.size;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let part = path.with_file_name(format!("{}.part", path.file_name().unwrap_or_default().to_string_lossy()));
        let mut response = client.get(file.url).send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("{} — HTTP {}", file.name, response.status().as_u16()));
        }
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        let mut got: u64 = 0;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            out.write_all(&chunk).map_err(|e| e.to_string())?;
            hash.update(&chunk);
            got += chunk.len() as u64;
            if got > file.size {
                break;
            }
            progress(*done + got);
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out);
        let digest = hash.finish();
        let hex: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        if got != file.size || hex != file.sha256 {
            let _ = std::fs::remove_file(&part);
            return Err(format!("{}: the file that came in is not the expected one", file.name));
        }
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
        *done += file.size;
    }
    Ok(())
}

/// Deletes the models (Settings → Remove the models).
pub fn remove(app: &AppHandle) {
    unload();
    let _ = std::fs::remove_dir_all(PARAKEET.dir());
    let _ = std::fs::remove_dir_all(DIARIZER.dir());
    kws::remove();
    downloads().lock().unwrap().error = None;
    emit_status(app);
}

// ── Recognition ───────────────────────────────────────────────────────────────

fn recognizer_slot() -> &'static Mutex<Option<Arc<OfflineRecognizer>>> {
    static R: OnceLock<Mutex<Option<Arc<OfflineRecognizer>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(None))
}

/// Serialises loading: two callers never load the 650 MB model twice.
fn load_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

fn threads() -> i32 {
    std::thread::available_parallelism().map(|n| n.get().clamp(1, 4) as i32).unwrap_or(2)
}

/// When Parakeet was last used: it is freed once unused for the idle setting (resources/idle.rs).
static LAST_USED: Mutex<Option<Instant>> = Mutex::new(None);

fn touch() {
    *LAST_USED.lock().unwrap() = Some(Instant::now());
}

/// Parakeet is in memory now.
pub fn is_loaded() -> bool {
    recognizer_slot().lock().unwrap().is_some()
}

/// The loaded recognizer, loading it on first use (about 2 s).
pub fn recognizer() -> Option<Arc<OfflineRecognizer>> {
    touch();
    if let Some(r) = recognizer_slot().lock().unwrap().clone() {
        return Some(r);
    }
    if !PARAKEET.is_present() {
        return None;
    }
    let _guard = load_lock().lock().unwrap();
    if let Some(r) = recognizer_slot().lock().unwrap().clone() {
        return Some(r);
    }
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(PARAKEET.path("encoder.int8.onnx")),
        decoder: Some(PARAKEET.path("decoder.int8.onnx")),
        joiner: Some(PARAKEET.path("joiner.int8.onnx")),
    };
    config.model_config.tokens = Some(PARAKEET.path("tokens.txt"));
    config.model_config.model_type = Some("nemo_transducer".into());
    config.model_config.num_threads = threads();
    crate::resources::ceiling::make_room("speech", 670.0);
    let started = Instant::now();
    crate::resources::loads::note(crate::resources::loads::Part::OfflineStt, "Parakeet, for speech on this PC");
    crate::resources::register("parakeet", reap_idle);
    let r = OfflineRecognizer::create(&config).map(Arc::new);
    match &r {
        Some(_) => crate::log::line(format!("Parakeet loaded in {:?}", started.elapsed())),
        None => crate::log::line("Parakeet could not be loaded"),
    }
    *recognizer_slot().lock().unwrap() = r.clone();
    r
}

/// Loads the engine that will be used, before the words come: shows
/// "Getting ready…" in the island when that takes more than a moment.
/// Nothing is loaded ahead of need (round 3B item 10): this runs when
/// dictation, a call or a command has started.
pub fn get_ready(app: &AppHandle) {
    if whisper().is_some() || !PARAKEET.is_present() || is_loaded() {
        return;
    }
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done2 = done.clone();
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_NOTE_AFTER);
        if !done2.load(std::sync::atomic::Ordering::Relaxed) {
            crate::flow::live::announce(&app2, "bolt", "#8E8E93", crate::i18n::t("Getting ready…"), None, 3);
        }
    });
    std::thread::spawn(move || {
        let _ = recognizer();
        done.store(true, std::sync::atomic::Ordering::Relaxed);
    });
}

/// Loading for longer than this shows "Getting ready…".
const READY_NOTE_AFTER: Duration = Duration::from_millis(300);

/// Memory pressure (round 3D item 7): frees the recognizers unless one is in use.
pub fn unload_idle_now() {
    if let Ok(mut slot) = recognizer_slot().try_lock() {
        if slot.take().is_some() {
            crate::log::line("Parakeet freed (memory)");
        }
    }
    crate::voice::free_now();
    kws::free_if_idle();
}

/// Frees the model's memory (dictation turned off, models removed).
pub fn unload() {
    if recognizer_slot().lock().unwrap().take().is_some() {
        crate::log::line("Parakeet freed");
    }
}

/// Frees Parakeet once unused for the idle setting (the resources thread).
fn reap_idle() {
    let last = *LAST_USED.lock().unwrap();
    if is_loaded() && last.is_some_and(crate::resources::idle::expired) {
        unload();
    }
}

/// What was said, and each word with its place in the audio.
#[derive(Debug, Clone, Default)]
pub struct Transcript {
    pub text: String,
    pub words: Vec<TimedWord>,
}

/// Longest stretch given to the model at once. Longer audio is cut where it is
/// quietest, near this length.
const MAX_CHUNK_SECONDS: f64 = 28.0;

/// Transcribes 16 kHz mono audio.
pub fn transcribe(samples: &[f32]) -> Option<Transcript> {
    if let Some((model, language)) = whisper() {
        return transcribe_whisper(&model, language.as_deref(), samples);
    }
    let rec = recognizer()?;
    let mut out = Transcript::default();
    for (offset, chunk) in audio::split_for_model(samples, SAMPLE_RATE, MAX_CHUNK_SECONDS) {
        if chunk.len() < (SAMPLE_RATE as usize) / 4 {
            continue;
        }
        let stream = rec.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, chunk);
        rec.decode(&stream);
        let Some(result) = stream.get_result() else { continue };
        let start = offset as f64 / SAMPLE_RATE as f64;
        let words = words_from_tokens(&result.tokens, result.timestamps.as_deref().unwrap_or(&[]), start, chunk.len() as f64 / SAMPLE_RATE as f64);
        let text = result.text.trim();
        if !text.is_empty() {
            if !out.text.is_empty() {
                out.text.push(' ');
            }
            out.text.push_str(text);
        }
        out.words.extend(words);
    }
    Some(out)
}

/// Parakeet's tokens (" Ne", " vous", " dem", "ande", "z") → words with times.
/// A token starting with a space starts a word; a word ends where the next
/// token begins (or a moment after its last token, for the last word).
pub fn words_from_tokens(tokens: &[String], timestamps: &[f32], offset: f64, length: f64) -> Vec<TimedWord> {
    let mut out: Vec<TimedWord> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let at = offset + timestamps.get(i).copied().unwrap_or(0.0) as f64;
        let starts_word = token.starts_with(' ') || token.starts_with('▁') || out.is_empty();
        let piece = token.trim_start_matches([' ', '▁']);
        if starts_word {
            if let Some(last) = out.last_mut() {
                last.end = last.end.max(at);
            }
            if piece.is_empty() {
                continue;
            }
            out.push(TimedWord { text: piece.to_string(), start: at, end: at + 0.08 });
        } else if let Some(last) = out.last_mut() {
            last.text.push_str(piece);
            last.end = at + 0.08;
        }
    }
    if let Some(last) = out.last_mut() {
        last.end = (last.end + 0.25).min(offset + length).max(last.start);
    }
    out.retain(|w| !w.text.is_empty());
    out
}

// ── Who speaks when ───────────────────────────────────────────────────────────

/// The voices of a recording, as turns "0", "1"… Empty when it can't tell.
pub fn diarize(samples: &[f32]) -> Vec<Turn> {
    if !DIARIZER.is_present() || samples.len() < SAMPLE_RATE as usize * 2 {
        return Vec::new();
    }
    let mut config = OfflineSpeakerDiarizationConfig::default();
    config.segmentation.pyannote.model = Some(DIARIZER.path("segmentation.int8.onnx"));
    config.segmentation.num_threads = threads();
    config.embedding.model = Some(DIARIZER.path("embedding.onnx"));
    config.embedding.num_threads = threads();
    config.clustering.num_clusters = -1;
    // Higher than the default 0.5: a voice is not split in two over a call.
    config.clustering.threshold = 0.7;
    let Some(d) = OfflineSpeakerDiarization::create(&config) else {
        crate::log::line("diarizer could not be loaded");
        return Vec::new();
    };
    let Some(result) = d.process(samples) else { return Vec::new() };
    result
        .sort_by_start_time()
        .into_iter()
        .map(|s| Turn { speaker: s.speaker.to_string(), start: s.start as f64, end: s.end as f64 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_words_are_spread_over_the_stretch() {
        let w = spread_words("ਸਤ ਸ੍ਰੀ ਅਕਾਲ ji", 10.0, 2.0);
        assert_eq!(w.len(), 4);
        assert_eq!((w[0].start, w[3].end), (10.0, 12.0));
        assert_eq!(w[1].text, "ਸ੍ਰੀ");
        assert!(spread_words("  ", 0.0, 1.0).is_empty());
    }

    #[test]
    fn whisper_takes_over_only_when_chosen() {
        use_whisper(Some(("small".into(), Some("pa".into()))));
        assert_eq!(whisper(), Some(("small".into(), Some("pa".into()))));
        use_whisper(None);
        assert_eq!(whisper(), None);
    }

    fn toks(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tokens_make_words_with_their_times() {
        let words = words_from_tokens(
            &toks(&[" Ne", " vous", " dem", "ande", "z", " pas", "."]),
            &[0.0, 0.16, 0.32, 0.4, 0.56, 0.72, 0.8],
            10.0,
            2.0,
        );
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Ne", "vous", "demandez", "pas."]);
        assert!((words[2].start - 10.32).abs() < 1e-6);
        assert!(words[2].end >= 10.72 - 1e-9);
        assert!(words[3].end <= 12.0);
    }

    #[test]
    fn the_downloads_add_up() {
        assert_eq!(PARAKEET.total_size(), 652_184_281 + 11_845_275 + 6_355_277 + 93_939);
        assert!(DIARIZER.total_size() < 40_000_000);
        for set in [&PARAKEET, &DIARIZER] {
            for f in set.files {
                assert_eq!(f.sha256.len(), 64, "{}", f.name);
                assert!(f.url.starts_with("https://huggingface.co/"), "{}", f.name);
            }
        }
    }
}

/// A 16-bit mono 16 kHz WAV from src-tauri/tests/audio, as samples.
#[cfg(test)]
pub fn tests_audio(name: &str) -> Vec<f32> {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/audio").join(name)).unwrap();
    let data = bytes.windows(4).position(|w| w == b"data").expect("a data chunk") + 8;
    bytes[data..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect()
}

/// With the models downloaded: `GROOVY_TEST_WAV=… cargo test -- --ignored speech`.
#[cfg(test)]
mod with_models {
    #[test]
    #[ignore]
    fn transcribes_a_recording() {
        let path = std::env::var("GROOVY_TEST_WAV").expect("GROOVY_TEST_WAV");
        let bytes = std::fs::read(&path).unwrap();
        let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let pcm: Vec<f32> = bytes[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
        let mut r = super::audio::Resampler::new(rate);
        let mut samples = Vec::new();
        r.process(&pcm, &mut samples);
        let t = super::transcribe(&samples).expect("models");
        println!("{}\n{:?}", t.text, t.words.iter().map(|w| (&w.text, (w.start * 100.0).round() / 100.0)).collect::<Vec<_>>());
        assert!(!t.text.is_empty());
        let turns = super::diarize(&samples);
        println!("{turns:?}");
    }
}
