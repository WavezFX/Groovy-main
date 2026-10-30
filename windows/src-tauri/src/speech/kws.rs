// « OK Groovy »'s ear (round 3B item 10): a 3.3 M-parameter keyword spotter
// (sherpa-onnx's zipformer trained on GigaSpeech, Apache-2.0, about 5 MB)
// listens for "OK Groovy", "Okay Groovy" and "Hey Groovy". Only stretches of
// speech reach it (wake::Vad gates it, so silence costs almost nothing); only
// once it fires is the full speech engine, cloud or Parakeet, asked for the
// words. It comes with the speech models, in one 17.6 MB archive that is
// checked against its SHA-256 before the four files it needs are kept.

use std::io::Read;
use std::path::PathBuf;

use sherpa_onnx::{KeywordSpotter, KeywordSpotterConfig, OnlineTransducerModelConfig};

use super::SAMPLE_RATE;

const ARCHIVE_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2";
const ARCHIVE_SIZE: u64 = 17_626_723;
const ARCHIVE_SHA256: &str = "f170013b4716e41b62b9bfd809687c207cef798ef9bc6534d524e17af9b6561a";
const ARCHIVE_DIR: &str = "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01";

/// (name, size, SHA-256) of each file kept from the archive.
const FILES: &[(&str, u64, &str)] = &[
    ("encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx", 4_807_159, "1e721676515bcd42a186979733981213c66c80db680e1cc582dfedf3be76e678"),
    ("decoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx", 277_985, "e40ff43297abe815e8898494c17e71bba2152d9d40fa3eb803f75d0f7533329a"),
    ("joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx", 163_380, "eae9da0c7e1e6c6a3f4cc42d167899c388f6c6701b94cb96320e4f55df79624c"),
    ("tokens.txt", 5_006, "fd2ded4050a55d2b1578870ba8697d02371980217806b7558bd0a5cc60f3ba53"),
];

/// The wake phrases in the model's BPE pieces (its bpe.model, by sentencepiece).
const KEYWORDS: &str = "▁O K ▁G RO O V Y @OK_GROOVY\n▁OKAY ▁G RO O V Y @OKAY_GROOVY\n▁HE Y ▁G RO O V Y @HEY_GROOVY";
/// « Also wake to his name » (round 5 item 6): the name he was given, and a
/// count that moves on each change so a loaded spotter knows to load again.
static NAME: std::sync::Mutex<(Option<String>, u64)> = std::sync::Mutex::new((None, 0));

/// The name to wake to as well, or None. Settings call it at launch and on each save.
pub fn set_name(name: Option<&str>) {
    let name = name.map(str::trim).filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("groovy")).map(str::to_string);
    let mut current = NAME.lock().unwrap();
    if current.0 != name {
        *current = (name, current.1 + 1);
        drop(current);
        *SHARED.lock().unwrap() = None;
    }
}

fn name_generation() -> u64 {
    NAME.lock().unwrap().1
}

/// A name in the model's BPE pieces, longest piece first, as sentencepiece
/// splits it: "PIPPA" → "▁PI PP A". None when a letter isn't the model's
/// (it knows English letters and the apostrophe) or no piece fits.
pub fn pieces(name: &str, vocab: &std::collections::HashSet<String>) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for word in name.split_whitespace() {
        let word = word.trim_matches(|c: char| matches!(c, '.' | ',' | '!' | '?' | '-')).to_ascii_uppercase();
        if word.is_empty() {
            continue;
        }
        if !word.chars().all(|c| c.is_ascii_uppercase() || c == '\'') {
            return None;
        }
        let chars: Vec<char> = format!("▁{word}").chars().collect();
        let mut at = 0;
        while at < chars.len() {
            let end = (at + 1..=chars.len()).rev().find(|&end| vocab.contains(&chars[at..end].iter().collect::<String>()))?;
            out.push(chars[at..end].iter().collect());
            at = end;
        }
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// The wake phrases, with his name (alone and after "Hey") when one is set and fits.
fn keywords(vocab: &std::collections::HashSet<String>) -> String {
    let mut all = KEYWORDS.to_string();
    if let Some(p) = NAME.lock().unwrap().0.as_deref().and_then(|n| pieces(n, vocab)) {
        all.push_str(&format!("\n{p} @HIS_NAME\n▁HE Y {p} @HEY_HIS_NAME"));
    }
    all
}

/// Tuned on synthetic voices: 14 of 16 wake phrases heard, none of 16 other
/// sentences ("OK, let's go…", "…really groovy") taken for one.
const SCORE: f32 = 1.5;
const THRESHOLD: f32 = 0.15;

pub fn dir() -> PathBuf {
    super::models_dir().join("kws-zipformer-gigaspeech-3.3M")
}

fn path(name: &str) -> String {
    dir().join(name).to_string_lossy().to_string()
}

pub fn is_present() -> bool {
    FILES.iter().all(|(name, size, _)| std::fs::metadata(dir().join(name)).is_ok_and(|m| m.len() == *size))
}

pub fn download_mb() -> u64 {
    ARCHIVE_SIZE / 1_000_000
}

fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads the archive, checks it, and keeps the spotter's four files.
pub async fn download(client: &reqwest::Client) -> Result<(), String> {
    if is_present() {
        return Ok(());
    }
    let mut response = client.get(ARCHIVE_URL).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("wake word model — HTTP {}", response.status().as_u16()));
    }
    let mut bytes = Vec::with_capacity(ARCHIVE_SIZE as usize);
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > ARCHIVE_SIZE {
            break;
        }
    }
    if bytes.len() as u64 != ARCHIVE_SIZE || sha256(&bytes) != ARCHIVE_SHA256 {
        return Err("wake word model: the file that came in is not the expected one".into());
    }
    tauri::async_runtime::spawn_blocking(move || unpack(&bytes)).await.map_err(|e| e.to_string())?
}

/// Takes the four files out of the checked archive; nothing else is written.
pub fn unpack(archive: &[u8]) -> Result<(), String> {
    let dir = dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let name = entry.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
        let Some(file) = name.strip_prefix(&format!("{ARCHIVE_DIR}/")) else { continue };
        let Some((name, size, sum)) = FILES.iter().find(|(n, _, _)| *n == file) else { continue };
        let mut data = Vec::with_capacity(*size as usize);
        entry.by_ref().take(size + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() as u64 != *size || sha256(&data) != *sum {
            return Err(format!("wake word model: {name} is not the expected file"));
        }
        std::fs::write(dir.join(name), &data).map_err(|e| e.to_string())?;
    }
    if is_present() {
        Ok(())
    } else {
        Err("wake word model: a file is missing from the archive".into())
    }
}

pub fn remove() {
    let _ = std::fs::remove_dir_all(dir());
}

/// The island's own "Hey Groovy" listener (Settings → Voice) shares one
/// spotter, freed once unused for the idle setting.
static SHARED: std::sync::Mutex<Option<(Spotter, std::time::Instant)>> = std::sync::Mutex::new(None);

/// Whether the wake phrase is in `speech`; None when the spotter isn't downloaded.
pub fn heard_shared(speech: &[f32]) -> Option<bool> {
    let mut shared = SHARED.lock().unwrap();
    if shared.is_none() {
        *shared = Some((Spotter::load()?, std::time::Instant::now()));
    }
    let (spotter, used) = shared.as_mut()?;
    *used = std::time::Instant::now();
    Some(spotter.heard(speech))
}

pub fn free_if_idle() {
    let mut shared = SHARED.lock().unwrap();
    if shared.as_ref().is_some_and(|(_, used)| crate::resources::idle::expired(*used)) {
        *shared = None;
    }
}

/// The spotter, loaded while « OK Groovy » listens (a few MB).
pub struct Spotter {
    kws: KeywordSpotter,
    /// The name it was loaded with (name_generation): another means load again.
    generation: u64,
}

impl Spotter {
    pub fn load() -> Option<Spotter> {
        if !is_present() {
            return None;
        }
        let vocab: std::collections::HashSet<String> = std::fs::read_to_string(dir().join("tokens.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect();
        let generation = name_generation();
        let config = KeywordSpotterConfig {
            model_config: sherpa_onnx::OnlineModelConfig {
                transducer: OnlineTransducerModelConfig {
                    encoder: Some(path(FILES[0].0)),
                    decoder: Some(path(FILES[1].0)),
                    joiner: Some(path(FILES[2].0)),
                },
                tokens: Some(path("tokens.txt")),
                num_threads: 1,
                ..Default::default()
            },
            keywords_score: SCORE,
            keywords_threshold: THRESHOLD,
            keywords_buf: Some(keywords(&vocab)),
            ..Default::default()
        };
        crate::resources::loads::note(crate::resources::loads::Part::WakeWord, "« OK Groovy » is on");
        crate::resources::register("wake-word", free_if_idle);
        KeywordSpotter::create(&config).map(|kws| Spotter { kws, generation })
    }

    /// His name changed (or « Also wake to his name » did) since this one was loaded.
    pub fn stale(&self) -> bool {
        self.generation != name_generation()
    }

    /// The wake phrase is in this stretch of speech (16 kHz mono).
    pub fn heard(&self, speech: &[f32]) -> bool {
        let stream = self.kws.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, speech);
        // Half a second of silence lets the last word through.
        stream.accept_waveform(SAMPLE_RATE as i32, &vec![0.0; SAMPLE_RATE as usize / 2]);
        stream.input_finished();
        while self.kws.is_ready(&stream) {
            self.kws.decode(&stream);
            if self.kws.get_result(&stream).is_some_and(|r| !r.keyword.is_empty()) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keywords_name_groovy_three_ways() {
        let lines: Vec<&str> = KEYWORDS.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.contains("▁G RO O V Y @")));
        assert_eq!(FILES.iter().map(|f| f.1).sum::<u64>(), 5_253_530);
    }

    #[test]
    fn a_name_is_split_into_the_models_pieces() {
        let vocab: std::collections::HashSet<String> = ["▁PI", "▁P", "P", "PP", "A", "▁MO", "CH", "I", "▁", "O", "'"].iter().map(|s| s.to_string()).collect();
        assert_eq!(pieces("Pippa", &vocab).as_deref(), Some("▁PI PP A"));
        assert_eq!(pieces("  mochi! ", &vocab).as_deref(), Some("▁MO CH I"));
        assert_eq!(pieces("Pi Mochi", &vocab).as_deref(), Some("▁PI ▁MO CH I"));
        // Letters the model doesn't know: no keyword, rather than a wrong one.
        assert_eq!(pieces("Zoë", &vocab), None);
        assert_eq!(pieces("小明", &vocab), None);
        assert_eq!(pieces("Xu", &vocab), None);
        assert_eq!(pieces("", &vocab), None);
    }

    #[test]
    fn the_name_joins_the_wake_phrases_and_groovy_is_not_added_twice() {
        let vocab: std::collections::HashSet<String> = ["▁PI", "PP", "A"].iter().map(|s| s.to_string()).collect();
        set_name(Some("Pippa"));
        let with = keywords(&vocab);
        assert!(with.ends_with("\n▁PI PP A @HIS_NAME\n▁HE Y ▁PI PP A @HEY_HIS_NAME"), "{with}");
        let before = name_generation();
        set_name(Some(" Pippa "));
        assert_eq!(name_generation(), before, "the same name: nothing to reload");
        set_name(Some("Groovy"));
        assert_eq!(keywords(&vocab), KEYWORDS);
        set_name(None);
        assert_eq!(keywords(&vocab), KEYWORDS);
    }

    #[test]
    fn a_wrong_archive_writes_nothing() {
        let before = is_present();
        assert!(unpack(b"not an archive").is_err());
        assert_eq!(is_present(), before);
    }

    /// Run on the Windows build (it downloads the model): cargo test -p groovy kws_ -- --ignored
    #[test]
    #[ignore]
    fn kws_hears_ok_groovy_and_nothing_else() {
        let client = super::super::download_client().unwrap();
        tauri::async_runtime::block_on(download(&client)).expect("download");
        let spotter = Spotter::load().expect("the spotter loads");
        let wav = |name: &str| super::super::tests_audio(name);
        assert!(spotter.heard(&wav("ok-groovy-time.wav")), "OK Groovy");
        assert!(!spotter.heard(&wav("not-wake.wav")), "OK, let's go to the store");
        assert!(!spotter.heard(&wav("dictation.wav")), "a dictation");
        println!("the spotter heard « OK Groovy » and nothing else");
    }
}
