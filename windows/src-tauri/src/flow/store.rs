// Everything the Groovy window shows and edits about dictation: the history,
// the numbers, the dictionary, the snippets, the styles and the preferences.
// Small JSON files in the app's folder (%LOCALAPPDATA%\Groovy\Flow or
// ~/.local/share/groovy/Flow), on this computer only. Port of FlowStore.swift.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::logic::FlowKey;
use super::text::{self, new_id, Day, Replacement, Snippet};

/// One dictation, kept for the history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    /// Milliseconds since 1970.
    pub date: u64,
    pub text: String,
    /// The app it was pasted in ("Slack"), when known.
    #[serde(default)]
    pub app: Option<String>,
    pub words: u64,
    /// How long you spoke.
    pub seconds: f64,
}

/// The numbers of one day. Kept even when the history is emptied.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DayStats {
    pub words: u64,
    pub seconds: f64,
    pub dictations: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    /// "2026-10-14" → that day's numbers.
    pub days: BTreeMap<String, DayStats>,
    /// App name → words dictated in it.
    pub apps: BTreeMap<String, u64>,
}

/// How the words are written in an app, when the clean-up is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Only the dictionary, replacements and snippets: code, terminals, AI chats.
    Verbatim,
    /// Without the "um" and the repeats, punctuation fixed, the tone kept.
    #[default]
    Natural,
    /// Full sentences, for mails.
    Polished,
}

/// An app with its own style. `id` is the program's name in lower case
/// ("outlook", "slack", "windowsterminal"), what the foreground window tells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppStyle {
    pub id: String,
    pub name: String,
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SummaryEngine {
    /// Groovy's AI (brain_ai.rs): the local model by default, Claude when chosen.
    /// Written "claude" before Groovy made it local-first.
    #[default]
    #[serde(alias = "claude")]
    Ai,
    Off,
}

/// The preferences of dictation and of the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Hold a key to dictate anywhere. Off until the user turns it on.
    pub enabled: bool,
    pub key: FlowKey,
    /// The microphone by name; empty is the system's default.
    pub mic: String,
    /// Days the history is kept: 0 = not kept, -1 = forever.
    pub keep_days: i64,
    /// Clean the words with Claude before pasting them (needs the Claude key).
    pub cleanup: bool,
    /// The style of the apps that have none of their own.
    pub default_style: Style,
    pub summary_engine: SummaryEngine,
    /// What turns speech into text: "whisper" (≈99 languages, the model and
    /// language of Settings → Voice calls; the default, a small quantized
    /// model) or "parakeet" (25 European languages, the most accurate, about
    /// 670 MB: over Groovy's 400 MB limit, so only when chosen; round 3D item 7).
    pub engine: String,
    /// Set once the small speech model was made the default: installs from
    /// before round 3D item 7 that were on Parakeet move to Whisper once.
    /// Missing from an older file means false.
    #[serde(default)]
    pub small_speech_set: bool,
    /// The guided speech setup (microphone, model, a test) was done once.
    pub speech_setup_done: bool,
    /// Open the Groovy window when Groovy is opened (not when it starts with the computer).
    pub open_at_launch: bool,
    /// « Hey Groovy »: listen for it and answer with Groovy's AI (local by default).
    pub voice: bool,
    /// How Groovy answers out loud: "system", "elevenlabs" or "off".
    pub voice_output: String,
    /// The ElevenLabs voice: "female" or "male".
    pub voice_gender: String,
    /// Fish Audio (round 3B item 8c): the voice's id from its page on
    /// fish.audio (`reference_id`), and the model. The key is in the OS store.
    pub fish_voice: String,
    pub fish_model: String,
    /// Who speaks first (Settings → Speaking, round 3D item 3): "fish" (Fish
    /// Audio, then Kokoro), "kokoro" (Kokoro on this PC) or "system" (Windows' voice).
    pub voice_pick: String,
    /// How fast every voice speaks, 0.5–2.0.
    pub voice_speed: f32,
    /// The offline voice (round 4A item 1): "light", "kokoro", or "auto"
    /// (Kokoro if it was downloaded before, else the light voice).
    pub offline_voice: String,
    /// Cloud speech-to-text first when a key is there (speech/cloud.rs), the
    /// PC's engine after; off: only the PC's engine (round 3B item 6).
    pub cloud_speech: bool,
    /// Where speech becomes text (round 4B item 2): "local" (this PC only, the
    /// default) or "cloud-first" (a cloud service when a key exists, this PC
    /// after). Missing from an older file means "" until `settle_speech_mode`.
    #[serde(default)]
    pub speech_mode: String,
    /// The one-time notice about `speech_mode` is still to be shown.
    #[serde(default)]
    pub speech_mode_notice: bool,
    /// Where his voice is made (round 4B item 3): "local" (the light voice,
    /// or Kokoro if downloaded and chosen, then Windows' own voice; the
    /// default) or "cloud" (Fish Audio or ElevenLabs with the user's key, the
    /// local voices after). Missing from an older file means "" until `settle_voice_mode`.
    #[serde(default)]
    pub voice_mode: String,
    /// The one-time notice about `voice_mode` is still to be shown.
    #[serde(default)]
    pub voice_mode_notice: bool,
    /// Learn names and models from what is corrected within a minute of a dictation.
    pub learn_words: bool,
}

impl Prefs {
    /// Once, for prefs from before round 3D item 7: Parakeet (the old default,
    /// about 670 MB) gives way to the small Whisper model. True when changed.
    pub fn offer_small_speech(&mut self) -> bool {
        if self.small_speech_set {
            return false;
        }
        if self.engine == "parakeet" {
            self.engine = "whisper".into();
        }
        self.small_speech_set = true;
        true
    }
}

/// Speech becomes text on this PC only; no cloud request is ever built.
pub const SPEECH_LOCAL: &str = "local";
/// A cloud speech service first when it has a key, this PC after.
pub const SPEECH_CLOUD_FIRST: &str = "cloud-first";

/// His voice is made on this PC only; no request goes to a voice service.
pub const VOICE_LOCAL: &str = "local";
/// Fish Audio or ElevenLabs with the user's key first, this PC after.
pub const VOICE_CLOUD: &str = "cloud";

impl Prefs {
    /// Once, for prefs from before round 4B item 2 (no `speech_mode` yet): an
    /// install that used cloud speech (on, with a key) keeps it as
    /// "cloud-first"; everyone else is "local". True when it changed.
    pub fn settle_speech_mode(&mut self, has_cloud_key: bool) -> bool {
        if self.speech_mode == SPEECH_LOCAL || self.speech_mode == SPEECH_CLOUD_FIRST {
            return false;
        }
        let keep_cloud = self.cloud_speech && has_cloud_key;
        self.speech_mode = if keep_cloud { SPEECH_CLOUD_FIRST } else { SPEECH_LOCAL }.into();
        self.cloud_speech = keep_cloud;
        self.speech_mode_notice = true;
        true
    }

    /// Keeps the old switch (`cloud_speech`, still what the Dictation page
    /// shows) and `speech_mode` in step after Settings changed one of them.
    pub fn follow_speech_switch(&mut self, before: &Prefs) {
        if self.cloud_speech != before.cloud_speech {
            self.speech_mode = if self.cloud_speech { SPEECH_CLOUD_FIRST } else { SPEECH_LOCAL }.into();
        } else if self.speech_mode != SPEECH_CLOUD_FIRST && self.speech_mode != SPEECH_LOCAL {
            self.speech_mode = before.speech_mode.clone();
        }
        self.cloud_speech = self.speech_mode == SPEECH_CLOUD_FIRST;
    }

    /// Once, for prefs from before round 4B item 3 (no `voice_mode` yet): an
    /// install with a cloud voice set up (Fish Audio picked with its key and a
    /// voice, or ElevenLabs answering with its key) keeps "cloud"; everyone
    /// else is "local". True when it changed.
    pub fn settle_voice_mode(&mut self, has_fish_key: bool, has_eleven_key: bool) -> bool {
        if self.voice_mode == VOICE_LOCAL || self.voice_mode == VOICE_CLOUD {
            return false;
        }
        let fish = has_fish_key && !self.fish_voice.trim().is_empty() && self.voice_pick == "fish";
        let eleven = has_eleven_key && self.voice_output == "elevenlabs";
        self.voice_mode = if fish || eleven { VOICE_CLOUD } else { VOICE_LOCAL }.into();
        self.voice_mode_notice = true;
        true
    }

    /// A cloud voice (Fish Audio, ElevenLabs) may be asked at all; keys are checked by the caller.
    pub fn cloud_voice(&self) -> bool {
        self.voice_mode == VOICE_CLOUD
    }

    /// Cloud speech may be asked at all (keys are checked by the caller).
    pub fn cloud_speech_first(&self) -> bool {
        self.speech_mode == SPEECH_CLOUD_FIRST
    }

    /// The cloud speech services a request may be built for: none in "local",
    /// whatever has a key (`available`) in "cloud-first".
    pub fn speech_services(&self, available: impl FnOnce() -> Vec<crate::speech::cloud::Service>) -> Vec<crate::speech::cloud::Service> {
        if self.cloud_speech_first() {
            available()
        } else {
            Vec::new()
        }
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            enabled: false,
            key: FlowKey::default(),
            mic: String::new(),
            keep_days: 30,
            cleanup: false,
            default_style: Style::Natural,
            summary_engine: SummaryEngine::Ai,
            engine: "whisper".into(),
            small_speech_set: true,
            speech_setup_done: false,
            open_at_launch: true,
            voice: false,
            voice_output: "system".into(),
            voice_gender: "female".into(),
            fish_voice: String::new(),
            fish_model: "s2-pro".into(),
            voice_pick: "fish".into(),
            voice_speed: 1.0,
            offline_voice: "auto".into(),
            cloud_speech: false,
            speech_mode: SPEECH_LOCAL.into(),
            speech_mode_notice: false,
            voice_mode: VOICE_LOCAL.into(),
            voice_mode_notice: false,
            learn_words: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Words {
    pub words: Vec<String>,
    pub replacements: Vec<Replacement>,
}

/// Apps that get a style from the start: mails polished, chats natural, code
/// and AI chats as said.
pub fn default_app_styles() -> Vec<AppStyle> {
    let s = |id: &str, name: &str, style| AppStyle { id: id.into(), name: name.into(), style };
    vec![
        s("outlook", "Outlook", Style::Polished),
        s("olk", "Outlook (new)", Style::Polished),
        s("thunderbird", "Thunderbird", Style::Polished),
        s("slack", "Slack", Style::Natural),
        s("discord", "Discord", Style::Natural),
        s("whatsapp", "WhatsApp", Style::Natural),
        s("telegram", "Telegram", Style::Natural),
        s("teams", "Microsoft Teams", Style::Natural),
        s("claude", "Claude", Style::Verbatim),
        s("windowsterminal", "Windows Terminal", Style::Verbatim),
        s("code", "Visual Studio Code", Style::Verbatim),
        s("cursor", "Cursor", Style::Verbatim),
        s("gnome-terminal-server", "Terminal", Style::Verbatim),
        s("konsole", "Konsole", Style::Verbatim),
        s("kitty", "kitty", Style::Verbatim),
    ]
}

pub struct Store {
    pub history: Vec<Entry>,
    pub stats: Stats,
    pub words: Words,
    pub snippets: Vec<Snippet>,
    pub app_styles: Vec<AppStyle>,
    pub prefs: Prefs,
    /// When the history is not kept, the last dictation still waits here
    /// (memory only) for "paste again".
    last_unkept: Option<String>,
    folder: PathBuf,
}

fn load<T: DeserializeOwned>(folder: &std::path::Path, name: &str) -> Option<T> {
    let text = std::fs::read_to_string(folder.join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn today() -> Day {
    let t = crate::platform::local_time();
    Day { year: t.year as i32, month: t.month, day: t.day }
}

impl Store {
    pub fn folder() -> PathBuf {
        crate::settings::local_dir().join("Flow")
    }

    pub fn load() -> Store {
        Self::load_from(Self::folder())
    }

    pub fn load_from(folder: PathBuf) -> Store {
        let mut store = Store {
            history: load(&folder, "history.json").unwrap_or_default(),
            stats: load(&folder, "stats.json").unwrap_or_default(),
            words: load(&folder, "words.json").unwrap_or_default(),
            snippets: load(&folder, "snippets.json").unwrap_or_default(),
            app_styles: load(&folder, "styles.json").unwrap_or_else(default_app_styles),
            prefs: load(&folder, "prefs.json").unwrap_or_default(),
            last_unkept: None,
            folder,
        };
        store.prune(now_ms());
        if store.prefs.offer_small_speech() {
            store.save_prefs();
        }
        // A brand-new install gets "local" from Default; only an older file is settled here.
        if store.prefs.speech_mode.is_empty() && store.prefs.settle_speech_mode(!crate::speech::cloud::available().is_empty()) {
            store.save_prefs();
        }
        if store.prefs.voice_mode.is_empty()
            && store.prefs.settle_voice_mode(crate::secrets::present(crate::speech::tts::FISH_KEY), crate::secrets::present("elevenlabs-api-key"))
        {
            store.save_prefs();
        }
        store
    }

    fn save<T: Serialize>(&self, value: &T, name: &str) {
        let write = || -> std::io::Result<()> {
            crate::platform::ensure_private_dir(&self.folder)?;
            let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
            let tmp = self.folder.join(format!("{name}.tmp"));
            std::fs::write(&tmp, text)?;
            std::fs::rename(&tmp, self.folder.join(name))
        };
        if let Err(err) = write() {
            crate::log::line(format!("flow: could not save {name}: {err}"));
        }
    }

    pub fn save_prefs(&self) {
        self.save(&self.prefs, "prefs.json");
    }
    pub fn save_words(&self) {
        self.save(&self.words, "words.json");
    }
    pub fn save_snippets(&self) {
        self.save(&self.snippets, "snippets.json");
    }
    pub fn save_styles(&self) {
        self.save(&self.app_styles, "styles.json");
    }
    fn save_history(&self) {
        self.save(&self.history, "history.json");
    }
    fn save_stats(&self) {
        self.save(&self.stats, "stats.json");
    }

    /// The words with the replacements and the dictionary's spelling, snippets set aside.
    pub fn prepare(&self, raw: &str) -> text::Prepared {
        text::prepare(raw, &self.words.words, &self.words.replacements, &self.snippets)
    }

    pub fn style_for(&self, app: Option<&str>) -> Style {
        app.and_then(|id| self.app_styles.iter().find(|s| s.id == id))
            .map(|s| s.style)
            .unwrap_or(self.prefs.default_style)
    }

    /// The last thing dictated, for "paste again".
    pub fn last_text(&self) -> Option<String> {
        self.history.first().map(|e| e.text.clone()).or_else(|| self.last_unkept.clone())
    }

    pub fn record(&mut self, text: &str, app: Option<&str>, seconds: f64, day: Day, now: u64) -> Option<Entry> {
        let count = text::word_count(text) as u64;
        if count == 0 {
            return None;
        }
        let d = self.stats.days.entry(day.key()).or_default();
        d.words += count;
        d.seconds += seconds;
        d.dictations += 1;
        if let Some(app) = app.filter(|a| !a.is_empty()) {
            *self.stats.apps.entry(app.to_string()).or_default() += count;
        }
        self.save_stats();
        if self.prefs.keep_days == 0 {
            self.last_unkept = Some(text.to_string());
            return None;
        }
        let entry = Entry { id: new_id(), date: now, text: text.to_string(), app: app.map(String::from), words: count, seconds };
        self.history.insert(0, entry.clone());
        self.prune(now);
        self.save_history();
        Some(entry)
    }

    pub fn delete(&mut self, id: &str) {
        self.history.retain(|e| e.id != id);
        self.save_history();
    }

    /// Empties the history; the numbers stay.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.last_unkept = None;
        self.save_history();
    }

    /// Empties the history and the numbers.
    pub fn clear_everything(&mut self) {
        self.clear_history();
        self.stats = Stats::default();
        self.save_stats();
    }

    pub fn prune(&mut self, now: u64) {
        let before = self.history.len();
        if self.prefs.keep_days == 0 {
            self.history.clear();
        } else if self.prefs.keep_days > 0 {
            let limit = now.saturating_sub(self.prefs.keep_days as u64 * 86_400_000);
            self.history.retain(|e| e.date >= limit);
        }
        if self.history.len() != before {
            self.save_history();
        }
    }

    pub fn streak(&self, today: Day) -> u32 {
        let days: HashSet<String> = self.stats.days.iter().filter(|(_, d)| d.words > 0).map(|(k, _)| k.clone()).collect();
        text::streak(&days, today)
    }

    /// Words, seconds and dictations since `from` (None: everything).
    pub fn totals(&self, from: Option<Day>) -> (u64, f64, u64) {
        let from = from.map(Day::key);
        let mut out = (0, 0.0, 0);
        for (key, d) in &self.stats.days {
            if from.as_ref().is_none_or(|f| key >= f) {
                out.0 += d.words;
                out.1 += d.seconds;
                out.2 += d.dictations;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parakeet_gives_way_to_the_small_whisper_model_once() {
        let mut old: Prefs = serde_json::from_str(r#"{"engine": "parakeet"}"#).unwrap();
        assert!(old.offer_small_speech());
        assert_eq!(old.engine, "whisper");
        old.engine = "parakeet".into();
        assert!(!old.offer_small_speech(), "chosen again, Parakeet stays");
        assert_eq!(Prefs::default().engine, "whisper");
        assert!(Prefs::default().small_speech_set);
    }

    fn temp_store() -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("groovy-flow-{}-{}", std::process::id(), new_id()));
        (Store::load_from(dir.clone()), dir)
    }

    #[test]
    fn a_dictation_counts_and_is_kept_for_the_chosen_days() {
        let (mut s, dir) = temp_store();
        let day = Day { year: 2026, month: 10, day: 14 };
        let now = 1_800_000_000_000;
        s.record("bonjour à tous", Some("slack"), 2.0, day, now).unwrap();
        s.record("?", None, 1.0, day, now);
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.stats.days["2026-10-14"].words, 3);
        assert_eq!(s.stats.apps["slack"], 3);
        assert_eq!(s.streak(day), 1);
        assert_eq!(s.totals(None), (3, 2.0, 1));
        // Older than 30 days: gone from the history, still in the numbers.
        s.prune(now + 31 * 86_400_000);
        assert!(s.history.is_empty());
        assert_eq!(s.totals(Some(day)).0, 3);
        // Not kept: the last one still waits for "paste again".
        s.prefs.keep_days = 0;
        assert!(s.record("encore un mot", None, 1.0, day, now).is_none());
        assert_eq!(s.last_text().as_deref(), Some("encore un mot"));
        // Saved on disk and read back.
        let again = Store::load_from(dir.clone());
        assert_eq!(again.stats.days["2026-10-14"].dictations, 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn styles_follow_the_app_or_the_default() {
        let (mut s, dir) = temp_store();
        assert_eq!(s.style_for(Some("outlook")), Style::Polished);
        assert_eq!(s.style_for(Some("code")), Style::Verbatim);
        assert_eq!(s.style_for(Some("notepad")), Style::Natural);
        s.prefs.default_style = Style::Polished;
        assert_eq!(s.style_for(None), Style::Polished);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prefs_read_from_an_older_file_keep_their_defaults() {
        let p: Prefs = serde_json::from_str(r#"{"enabled":true,"key":"right_alt"}"#).unwrap();
        assert!(p.enabled);
        assert_eq!(p.key, FlowKey::RightAlt);
        assert_eq!(p.keep_days, 30);
        assert!(p.open_at_launch);
    }

    #[test]
    fn speech_is_local_by_default_and_builds_no_cloud_request_with_keys_present() {
        use crate::speech::cloud::Service;
        let all = || Service::ALL.to_vec();
        let fresh = Prefs::default();
        assert_eq!(fresh.speech_mode, SPEECH_LOCAL);
        assert!(!fresh.cloud_speech_first());
        // Every service "has a key", yet local mode offers none of them.
        assert!(fresh.speech_services(all).is_empty());
        let mut cloud = fresh.clone();
        cloud.speech_mode = SPEECH_CLOUD_FIRST.into();
        assert_eq!(cloud.speech_services(all).len(), Service::ALL.len());
    }

    #[test]
    fn older_installs_keep_cloud_speech_only_if_they_used_it() {
        let old = || -> Prefs { serde_json::from_str(r#"{"cloudSpeech": true}"#).unwrap() };
        let mut used = old();
        assert_eq!(used.speech_mode, "", "missing in an older file");
        assert!(used.settle_speech_mode(true));
        assert_eq!(used.speech_mode, SPEECH_CLOUD_FIRST);
        assert!(used.speech_mode_notice);
        let mut no_key = old();
        assert!(no_key.settle_speech_mode(false));
        assert_eq!(no_key.speech_mode, SPEECH_LOCAL);
        assert!(!no_key.cloud_speech);
        assert!(!used.settle_speech_mode(true), "only once");
        // The Dictation page's switch moves the mode with it.
        let before = used.clone();
        let mut after = used.clone();
        after.cloud_speech = false;
        after.follow_speech_switch(&before);
        assert_eq!(after.speech_mode, SPEECH_LOCAL);
    }

    #[test]
    fn his_voice_is_local_by_default_and_older_cloud_voices_stay() {
        let fresh = Prefs::default();
        assert_eq!(fresh.voice_mode, VOICE_LOCAL);
        assert!(!fresh.cloud_voice());
        let old = |json: &str| -> Prefs { serde_json::from_str(json).unwrap() };
        let mut fish = old(r#"{"voicePick": "fish", "fishVoice": "abc"}"#);
        assert_eq!(fish.voice_mode, "");
        assert!(fish.settle_voice_mode(true, false));
        assert!(fish.cloud_voice() && fish.voice_mode_notice);
        assert!(!fish.settle_voice_mode(false, false), "only once");
        let mut no_key = old(r#"{"voicePick": "fish", "fishVoice": "abc"}"#);
        no_key.settle_voice_mode(false, false);
        assert!(!no_key.cloud_voice());
        let mut eleven = old(r#"{"voiceOutput": "elevenlabs"}"#);
        eleven.settle_voice_mode(false, true);
        assert!(eleven.cloud_voice());
        let mut plain = old(r#"{"voicePick": "system"}"#);
        plain.settle_voice_mode(true, true);
        assert!(!plain.cloud_voice());
    }
}
