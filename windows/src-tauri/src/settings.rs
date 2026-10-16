// Preferences, stored as plain JSON in settings.json under platform::config_dir().
// No secret ever lands here — API keys live in the OS keychain (see secrets.rs).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A field the file lacks takes its value from `Default`, so a settings.json
/// written by an older build still loads, whatever has been added since.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub auto_close_interval: f64,
    /// Hovering the island opens it all the way, and it folds again shortly
    /// after the pointer leaves (the Mac's "Open on hover"). Off by default.
    pub open_on_hover: bool,
    pub absence_interval: f64,
    pub active_integrations: Vec<String>,
    /// The always-on workspace pill (src/core/pills.ts checks it is one).
    pub main_pill: String,
    /// "primary" = the main display, "cursor" = whichever display the mouse is on.
    pub screen: String,
    pub autostart: bool,
    pub hooks_installed: bool,
    /// Claude model used by the chat. Changeable in the settings window.
    pub model: String,
    /// Show the Claude plan pill (5 h and weekly limits) in the island's header.
    /// Off until the user turns it on, so the header stays as it shipped.
    pub show_plan_in_notch: bool,
    /// Groovy's status line relay is the one in Claude Code's settings.json.
    /// Like `hooks_installed`, the real state wins at launch over what was stored.
    pub plan_relay_installed: bool,
    /// Show the Codex plan pill (5 h / weekly limits from `codex app-server`).
    /// Off by default; nothing is installed for it.
    pub show_codex_plan_in_notch: bool,
    /// Who the chat talks to: "anthropic", a cloud provider of
    /// openai_compat.rs ("openai", "google", "openrouter"), or a model server
    /// of local_chat.rs ("ollama", "lmstudio", "custom"). Picked in the chat view.
    pub chat_provider: String,
    /// The model picked for each provider other than Anthropic (whose model is
    /// `model`), by provider id.
    pub chat_models: BTreeMap<String, String>,
    /// Addresses of the model servers once connected; empty means not connected.
    pub ollama_url: String,
    pub lmstudio_url: String,
    /// Any other OpenAI-compatible server; its key, if any, is in the keychain.
    pub custom_url: String,
    /// Global shortcuts the user changed, by action id; the others keep their
    /// default (see shortcuts.rs).
    pub shortcuts: crate::shortcuts::Bindings,
    /// Groovy's outfit, picked in the wardrobe: "auto" (dresses for the
    /// season), "none" or an outfit id — the Mac's raw values. The island reads
    /// anything it doesn't know as "auto", so the value is stored as it comes.
    pub mochi_outfit: String,
    /// Groovy's look: "night" (the default) or another look id from
    /// src/mochi/skins.ts. Kept as it comes: the island reads anything it
    /// doesn't know as the default.
    pub groovy_look: String,
    /// When the look was last chosen (Unix ms): of two windows' saves, the
    /// newer choice wins (keep_newer_look, round 3B item 4).
    pub look_at: f64,
    /// Voice: Groovy listens for "Groovy" / "Hey Groovy" / "Yo Groovy" while on.
    pub voice_enabled: bool,
    /// "local" (Whisper on this computer) or "livekit" (a live conversation
    /// with the user's own LiveKit agent after the wake word).
    pub voice_engine: String,
    /// The Whisper model voice.rs loads: "tiny.en" or "base.en".
    pub voice_model: String,
    /// The user's LiveKit server (wss://…) and the room Groovy joins. The API
    /// key and secret are in the keychain (secrets.rs), never here.
    pub livekit_url: String,
    pub livekit_room: String,
    /// Where along the top of the screen the island sits: 0 = left edge,
    /// 0.5 = centre (the default), 1 = right edge. Set by dragging the island;
    /// owned by the Rust side (what a webview sends back is ignored).
    pub island_x: f64,
    /// After the wake word, keep listening after each answer, like a call.
    pub voice_call: bool,
    /// Read answers to spoken questions aloud (the system's voices).
    pub voice_speak: bool,
    /// The voice he speaks with, by name; empty picks an offline English one.
    pub voice_tts: String,
    /// What Whisper listens for with a multilingual model: an ISO 639-1 code, or "" to detect it.
    pub voice_language: String,
    /// The microphone's deviceId as the webview lists it; "" = the system default.
    pub voice_mic: String,
    /// Speaking over Groovy stops him and he listens (best with headphones).
    pub voice_interrupt: bool,
    /// Break reminders (src/island/breaks.ts): minutes between them, 0 = off.
    pub break_water: u32,
    /// The closed island stays on screen as a small pill instead of hiding.
    pub keep_visible: bool,
    pub break_stretch: u32,
    pub break_eyes: u32,
    /// A colour of the user's own for a pill's Groovy, by pill ID ("#RRGGBB"),
    /// picked in Settings → Active pills. Empty means the catalog's colours.
    /// Kept as it comes, like `mochi_outfit`: src/core/pill-colors.ts reads
    /// whatever is not a colour as "no choice".
    pub pill_colors: BTreeMap<String, String>,
    /// Interface language: "" follows the system, else one of i18n::LANGUAGES
    /// ("fr", "pt-BR", "zh-Hans"…). Kept as it comes, like `mochi_outfit`: a
    /// code this build doesn't know reads as "".
    pub language: String,
    /// Groovy on the desktop: whether he lives there, and his spot. Owned by
    /// the Rust side (desktop.rs) — what a webview sends back is ignored.
    pub desktop_mochi: DesktopMochiPref,
    /// The phone link (phone_link/) is on: a TLS server for the Android app on
    /// the local network. Off until the user turns it on in Settings → Phone;
    /// owned by the Rust side, so what a webview sends back is ignored.
    pub phone_link: bool,
    /// The phone may chat through this computer's API keys (phone_link/chat.rs).
    /// Off until the user turns it on in Settings → Android phone; owned by the
    /// Rust side like `phone_link`, so what a webview sends back is ignored.
    pub phone_chat: bool,
    /// Which models the phone may use, as "provider/model". Empty allows none.
    /// Also owned by the Rust side (phone_link::set_chat_models cleans it).
    pub phone_chat_models: Vec<String>,
    /// The phone may see session details (steps, last line, project folder name, colour). Off until the
    /// user turns it on in Settings → Android phone; owned by the Rust side like `phone_link`.
    pub phone_details: bool,
    /// Let the phone answer the questions Claude Code asks (cap `answers`). Off by default; owned by Rust like the other phone switches.
    pub phone_answers: bool,
    /// Show the files an agent changed on the phone, and their diffs on request (cap `diffs`). Off by default; owned by Rust like the other phone switches.
    pub phone_diffs: bool,
    /// Show the Claude and Codex plan usage on the phone (cap `usage`). Off by default; owned by Rust like the other phone switches.
    pub phone_usage: bool,
    /// The service pills whose cards the phone may see, by pill id (cap `services`). None by default; owned by Rust like the other phone switches.
    pub phone_services: Vec<String>,
    /// Reach the phone through the user's own relay when it is not on this network (phone_link/relay_client.rs).
    /// Off by default; owned by Rust like the other phone switches. The access key, the room and the pairing key live
    /// in the OS keystore, never here.
    pub phone_relay: bool,
    /// The relay's address (`wss://…`), empty until the user enters it. Not a secret. Owned by Rust.
    pub phone_relay_url: String,
    /// Personality, AI mode, memory and performance (assistant.rs).
    pub assistant: crate::assistant::AssistantPrefs,
    /// Groovy's colours, layered accessories, presets and favourites. The page
    /// owns its shape (src/mochi/appearance.ts) and validates it on every read.
    pub wardrobe: Value,
    /// "My island": the preset, the tiles and the mail apps. The page owns its
    /// shape (src/core/tiles.ts) and validates it on every read.
    pub island_tiles: Value,
    /// "Close when I leave" (the default since round 2) was offered once to a
    /// user who had another auto-close delay.
    pub close_on_leave_offered: bool,
    /// Control Centre → Appearance → Island style: "glass" (Windows' own
    /// Acrylic, the default) or "solid" (the black island).
    pub island_style: String,
    /// Settings → Voice → While listening, show: "badge" (default) or "strip".
    pub listening_view: String,
    /// How far Groovy's look has grown: active seconds, the stage reached and
    /// the stage shown (round 3 item 6). The page owns its shape
    /// (src/mochi/growth.ts) and checks it on every read; it never goes back.
    pub growth: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesktopMochiPref {
    /// He was on the desktop when the app quit: he flies back out at launch.
    pub on_desktop: bool,
    /// Top-left corner of his window where the user last left him.
    pub spot: Option<DesktopSpot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopSpot {
    pub x: f64,
    pub y: f64,
    /// What x and y are measured in (`DesktopMode::space`).
    pub space: String,
}

fn default_model() -> String {
    crate::claude::DEFAULT_MODEL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            sound_volume: 0.12,
            auto_close_interval: 0.0,
            open_on_hover: false,
            absence_interval: 180.0,
            active_integrations: vec![
                "integration_resend".into(),
                "integration_n8n".into(),
                "integration_vercel".into(),
                "integration_github".into(),
            ],
            main_pill: "integration_claude".into(),
            screen: "primary".into(),
            autostart: false,
            hooks_installed: false,
            model: default_model(),
            show_plan_in_notch: false,
            plan_relay_installed: false,
            show_codex_plan_in_notch: false,
            // Offline first: a local Ollama model, no API key needed.
            chat_provider: "llamacpp".into(),
            chat_models: BTreeMap::new(),
            ollama_url: String::new(),
            lmstudio_url: String::new(),
            custom_url: String::new(),
            shortcuts: Default::default(),
            // Round 3: no outfit any more, only Head/Face/Neck layers (the wardrobe).
            mochi_outfit: "none".into(),
            groovy_look: "night".into(),
            look_at: 0.0,
            voice_enabled: false,
            voice_engine: "local".into(),
            voice_model: "base.en".into(),
            livekit_url: String::new(),
            livekit_room: "groovy".into(),
            island_x: 0.5,
            voice_call: true,
            voice_speak: true,
            voice_tts: String::new(),
            voice_language: String::new(),
            voice_mic: String::new(),
            voice_interrupt: false,
            break_water: 45,
            keep_visible: true,
            break_stretch: 60,
            break_eyes: 0,
            pill_colors: BTreeMap::new(),
            language: String::new(),
            desktop_mochi: DesktopMochiPref::default(),
            phone_link: false,
            phone_chat: false,
            phone_chat_models: Vec::new(),
            phone_details: false,
            phone_answers: false,
            phone_diffs: false,
            phone_usage: false,
            phone_services: Vec::new(),
            phone_relay: false,
            phone_relay_url: String::new(),
            assistant: Default::default(),
            wardrobe: Value::Object(Map::new()),
            island_tiles: Value::Object(Map::new()),
            close_on_leave_offered: false,
            island_style: "glass".into(),
            listening_view: "badge".into(),
            growth: Value::Object(Map::new()),
        }
    }
}

pub use crate::platform::{config_dir, local_dir};

pub fn hook_exe_path() -> PathBuf {
    local_dir().join("bin").join(crate::platform::HOOK_EXE)
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// No settings.json yet: the very first start.
pub fn first_run() -> bool {
    !settings_path().exists()
}

pub fn load() -> Settings {
    load_from(&settings_path())
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let dir = config_dir();
    crate::platform::ensure_private_dir(&dir)?;
    save_to(&settings_path(), settings)
}

/// How many copies `set_aside` can make within one second before it gives up.
const MAX_COPIES_PER_SECOND: u32 = 100;

/// settings.json files that are there but whose contents never made it into
/// memory: they could not be read, or could not be used and could not be copied
/// aside either. What is in memory for them is the defaults, so the next save
/// must not quietly replace them — see `keep_what_was_not_loaded`.
static NOT_LOADED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn not_loaded() -> MutexGuard<'static, Vec<PathBuf>> {
    NOT_LOADED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One line in groovy.log. Tests must never write to the real one.
fn note(message: String) {
    #[cfg(not(test))]
    crate::log::line(message);
    #[cfg(test)]
    eprintln!("{message}");
}

/// Of the look already saved and the one a window sends, the newer choice:
/// a slower save of an older copy never undoes a look chosen since.
pub fn keep_newer_look(current: (&str, f64), incoming: (&str, f64)) -> (String, f64) {
    let at = |t: f64| if t.is_finite() { t } else { 0.0 };
    if at(incoming.1) >= at(current.1) {
        (incoming.0.to_string(), at(incoming.1))
    } else {
        (current.0.to_string(), at(current.1))
    }
}

/// The furthest of two growth records (round 3 item 6): the most active
/// seconds and the highest stage reached win, so a window with an older copy
/// never makes Groovy smaller. There is no chosen stage any more (round 3B
/// item 5): a saved one is dropped. Anything unreadable counts as nothing.
pub fn merge_growth(old: &Value, new: &Value) -> Value {
    let seconds = |v: &Value| v.get("activeSeconds").and_then(Value::as_f64).filter(|s| s.is_finite() && *s >= 0.0).unwrap_or(0.0);
    let stage = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).filter(|s| *s <= 5);
    let reached = stage(old, "reached").unwrap_or(0).max(stage(new, "reached").unwrap_or(0));
    // The stage the launch intro last greeted him at (-1: never).
    let seen = stage(old, "seen").max(stage(new, "seen")).map_or(-1, |s| s.min(reached) as i64);
    serde_json::json!({
        "activeSeconds": seconds(old).max(seconds(new)),
        "reached": reached,
        // No stage picking any more (round 3B item 5): a saved choice is dropped.
        "shown": null,
        "seen": seen,
    })
}

/// The settings held in the bytes of a settings.json, or `None` when there is
/// nothing usable in them. Split from `load_from` so it can be tested without a
/// file.
fn parse(bytes: &[u8]) -> Option<Settings> {
    // PowerShell 5 and Notepad write a UTF-8 BOM, and serde_json refuses it.
    let text = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if text.iter().all(u8::is_ascii_whitespace) {
        return Some(Settings::default());
    }
    let Value::Object(fields) = serde_json::from_slice(text).ok()? else {
        return None;
    };
    let whole = serde_json::from_value(Value::Object(fields.clone()));
    Some(whole.unwrap_or_else(|_| salvage(fields)))
}

/// Settings from the fields of a settings.json that does not load as a whole.
/// Each field is taken on its own: one that is not the kind of value expected
/// falls back to its default instead of costing every other setting.
fn salvage(fields: Map<String, Value>) -> Settings {
    let Ok(Value::Object(mut kept)) = serde_json::to_value(Settings::default()) else {
        return Settings::default();
    };
    for (key, value) in fields {
        let mut candidate = kept.clone();
        candidate.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(Value::Object(candidate.clone())).is_ok() {
            kept = candidate;
        } else {
            note(unusable_field(&key));
        }
    }
    serde_json::from_value(Value::Object(kept)).unwrap_or_default()
}

/// The log line for a field `salvage` had to drop. The name comes straight from
/// the file, so it is written escaped: a line break in it must not be able to
/// start what looks like another line of groovy.log.
fn unusable_field(key: &str) -> String {
    format!("settings.json: {key:?} is not usable — its default is used instead")
}

/// `load`, for the file at `path`: tests point it at a temporary directory.
///
/// Only a file that is not there means "start from the defaults" with nothing
/// more to do. One that cannot be read, or cannot be used, also gives the
/// defaults, but it still holds what the user had: it is kept, not written over.
fn load_from(path: &Path) -> Settings {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => return Settings::default(),
        Err(err) => {
            note(format!("can't read {}: {err} — using the defaults", path.display()));
            protect(path);
            return Settings::default();
        }
    };
    parse(&bytes).unwrap_or_else(|| {
        match set_aside(path, "corrupt", &bytes) {
            Ok(copy) => note(format!(
                "{} is not usable — kept as {}, using the defaults",
                path.display(),
                copy.display()
            )),
            Err(err) => {
                note(format!("{} is not usable and could not be copied: {err}", path.display()));
                protect(path);
            }
        }
        Settings::default()
    })
}

/// Marks `path` as holding something the next save must not write over.
fn protect(path: &Path) {
    let mut pending = not_loaded();
    if !pending.iter().any(|p| p == path) {
        pending.push(path.to_path_buf());
    }
}

/// Before the first save over a file `load_from` could not load: copies it
/// aside if it can be read by now, and refuses the save if it still cannot.
fn keep_what_was_not_loaded(path: &Path) -> std::io::Result<()> {
    let mut pending = not_loaded();
    let Some(index) = pending.iter().position(|p| p == path) else {
        return Ok(());
    };
    match std::fs::read(path) {
        Ok(bytes) => {
            let copy = set_aside(path, "unread", &bytes)?;
            note(format!("{} was never loaded — kept as {}", path.display(), copy.display()));
        }
        // Gone since: nothing is left to protect.
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(std::io::Error::new(
                err.kind(),
                format!("{} still can't be read, so it is left as it is: {err}", path.display()),
            ));
        }
    }
    pending.swap_remove(index);
    Ok(())
}

/// Keeps `bytes`, the contents of `path`, in a file beside it named after it
/// with `.<kind>-YYYYMMDD-HHMMSS` appended. Contents an earlier copy of that
/// kind already holds are not copied again: a file nobody repairs would
/// otherwise leave one more copy behind at every launch.
fn set_aside(path: &Path, kind: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    if let Some(copy) = same_copy(path, kind, bytes) {
        return Ok(copy);
    }
    let t = crate::platform::local_time();
    let stamp = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    );
    set_aside_as(path, kind, &stamp, bytes, write_whole)
}

/// The copy of `path` of this `kind` that already holds exactly `bytes`.
fn same_copy(path: &Path, kind: &str, bytes: &[u8]) -> Option<PathBuf> {
    let prefix = format!("{}.{kind}-", path.file_name()?.to_string_lossy());
    std::fs::read_dir(path.parent()?)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .filter(|entry| entry.metadata().is_ok_and(|m| m.len() == bytes.len() as u64))
        .map(|entry| entry.path())
        .find(|copy| std::fs::read(copy).is_ok_and(|held| held == bytes))
}

/// `set_aside` for a given time stamp, writing through `write`. `-1`, `-2`… are
/// appended while the name is taken, so a copy that is already there is never
/// written over; and a copy that could not be written whole is removed, never
/// left looking like a faithful one.
fn set_aside_as(
    path: &Path,
    kind: &str,
    stamp: &str,
    bytes: &[u8],
    write: fn(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<PathBuf> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let base = format!("{name}.{kind}-{stamp}");
    for attempt in 0..MAX_COPIES_PER_SECOND {
        let copy = match attempt {
            0 => path.with_file_name(&base),
            n => path.with_file_name(format!("{base}-{n}")),
        };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&copy) {
            Ok(mut file) => {
                let written = write(&mut file, bytes);
                drop(file);
                if written.is_err() {
                    let _ = std::fs::remove_file(&copy);
                }
                return written.map(|()| copy);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::new(ErrorKind::AlreadyExists, format!("too many copies named {base}")))
}

/// Writes `bytes` to `file` and waits until they are on the disk.
fn write_whole(file: &mut std::fs::File, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()
}

/// `save`, for the file at `path`, whose directory already exists.
fn save_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    keep_what_was_not_loaded(path)?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;

    // A dotfiles setup can make settings.json a symlink: write to the file it
    // points at, so the link survives the rename below.
    #[cfg(unix)]
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(unix)]
    let path = resolved.as_path();

    // Write beside the target and rename over it: a crash, a full disk or a
    // power cut leaves the previous settings.json intact rather than half a file.
    let temp = path.with_extension(format!("json.groovy-{}", std::process::id()));
    let written = std::fs::File::create(&temp)
        .and_then(|mut file| write_whole(&mut file, &json))
        .and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// A settings.json in which no value is the default one.
    // Two #: the colours in it are written "#RRGGBB".
    const CUSTOM: &str = r##"{
  "soundEnabled": false,
  "soundVolume": 0.5,
  "autoCloseInterval": 30.0,
  "openOnHover": true,
  "absenceInterval": 60.0,
  "activeIntegrations": ["integration_notion"],
  "mainPill": "agent_cursor",
  "screen": "cursor",
  "autostart": true,
  "hooksInstalled": true,
  "model": "some-model",
  "showPlanInNotch": true,
  "planRelayInstalled": true,
  "showCodexPlanInNotch": true,
  "chatProvider": "ollama",
  "chatModels": { "ollama": "llama3.2", "openai": "gpt-x" },
  "ollamaUrl": "http://127.0.0.1:11434",
  "lmstudioUrl": "http://127.0.0.1:1234",
  "customUrl": "https://llm.example.com",
  "shortcuts": { "openChat": { "keys": "Ctrl+Shift+K", "enabled": false } },
  "mochiOutfit": "witchHat",
  "groovyLook": "tide",
  "lookAt": 1760000000000.5,
  "voiceEnabled": true,
  "voiceEngine": "livekit",
  "voiceModel": "tiny.en",
  "livekitUrl": "wss://me.livekit.cloud",
  "livekitRoom": "den",
  "islandX": 0.25,
  "voiceCall": false,
  "voiceSpeak": false,
  "voiceTts": "Microsoft George",
  "voiceLanguage": "pa",
  "voiceMic": "abc123",
  "voiceInterrupt": true,
  "breakWater": 30,
  "keepVisible": false,
  "breakStretch": 90,
  "breakEyes": 20,
  "pillColors": { "integration_claude": "#2DD4BF" },
  "language": "pt-BR",
  "desktopMochi": { "onDesktop": true, "spot": { "x": 1500.5, "y": -300.0, "space": "screen" } },
  "phoneLink": true,
  "phoneChat": true,
  "phoneChatModels": ["openai/gpt-x"],
  "phoneDetails": true,
  "phoneAnswers": true,
  "phoneDiffs": true,
  "phoneUsage": true,
  "phoneServices": ["integration_stripe"],
  "phoneRelay": true,
  "phoneRelayUrl": "wss://relay.example.workers.dev",
  "assistant": {
    "name": "Mochi",
    "personality": "sarcastic",
    "humour": 8,
    "responseLength": "short",
    "proactivity": "high",
    "language": "Spanish",
    "customInstructions": "Call me Captain.",
    "aiMode": "local",
    "localProvider": "ollama",
    "localModel": "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    "gpuLayers": 20,
    "brain": "claude",
    "aiSetup": "local",
    "dailyOnlineLimit": 50,
    "memoryEnabled": false,
    "historyEnabled": false,
    "historyRetentionDays": 7,
    "performanceProfile": "lightweight"
  },
  "wardrobe": {
    "appearance": { "body": "", "body2": "", "glow": "#ff00aa", "eyes": "", "layers": ["sunglasses"] },
    "presets": [],
    "favourites": ["crown"]
  },
  "islandTiles": { "preset": "custom", "tiles": [{ "id": "mail", "kind": "mail", "target": "", "label": "", "size": "expanded" }], "mailApps": ["Outlook"] },
  "closeOnLeaveOffered": true,
  "islandStyle": "solid",
  "listeningView": "strip",
  "growth": { "activeSeconds": 90000, "reached": 1, "shown": null }
}"##;

    fn custom() -> Value {
        serde_json::from_str(CUSTOM).unwrap()
    }

    /// `CUSTOM` with one key replaced, or removed when `value` is `None`.
    fn custom_with(key: &str, value: Option<Value>) -> Vec<u8> {
        let mut object = custom().as_object().unwrap().clone();
        match value {
            Some(value) => object.insert(key.to_string(), value),
            None => object.remove(key),
        };
        serde_json::to_vec(&object).unwrap()
    }

    /// Settings compare through their JSON: the struct has no `PartialEq`.
    fn shown(settings: &Settings) -> Value {
        serde_json::to_value(settings).unwrap()
    }

    fn defaults() -> Value {
        shown(&Settings::default())
    }

    /// A fresh directory of our own, and the settings.json it will hold.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir()
            .join(format!("groovy-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        (dir, file)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    /// What was set aside under `settings.json.<kind>-…`.
    fn kept(dir: &Path, kind: &str) -> Vec<Vec<u8>> {
        let prefix = format!("settings.json.{kind}-");
        names(dir)
            .iter()
            .filter(|name| name.starts_with(&prefix))
            .map(|name| std::fs::read(dir.join(name)).unwrap())
            .collect()
    }

    // ── Reading ───────────────────────────────────────────────────────────────

    #[test]
    fn new_installs_close_the_island_when_the_pointer_leaves() {
        let d = Settings::default();
        assert_eq!(d.auto_close_interval, 0.0);
        assert!(!d.close_on_leave_offered);
        // An older settings file keeps its own delay and has not been offered it yet.
        let old: Settings = serde_json::from_str(r#"{"autoCloseInterval": 15}"#).unwrap();
        assert_eq!(old.auto_close_interval, 15.0);
        assert!(!old.close_on_leave_offered);
    }

    #[test]
    fn the_island_is_glass_unless_solid_was_chosen() {
        assert_eq!(Settings::default().island_style, "glass");
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.island_style, "glass");
        let solid: Settings = serde_json::from_str(r#"{"islandStyle": "solid"}"#).unwrap();
        assert_eq!(solid.island_style, "solid");
    }

    #[test]
    fn a_missing_file_is_the_defaults_and_is_not_created() {
        let (dir, file) = scratch("missing");
        assert_eq!(shown(&load_from(&file)), defaults());
        assert!(names(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_file_is_the_defaults_and_nothing_is_set_aside() {
        let (dir, file) = scratch("empty");
        for nothing in [&b""[..], &b" \r\n\t "[..]] {
            std::fs::write(&file, nothing).unwrap();
            assert_eq!(shown(&load_from(&file)), defaults());
        }
        assert_eq!(names(&dir), ["settings.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_fields_are_ignored_and_the_known_ones_kept() {
        let bytes = custom_with("somethingFromALaterVersion", Some(json!({ "a": [1, 2] })));
        assert_eq!(shown(&parse(&bytes).unwrap()), custom());
    }

    #[test]
    fn a_file_from_before_the_model_setting_keeps_everything_else() {
        let loaded = shown(&parse(&custom_with("model", None)).unwrap());
        let mut expected = custom();
        expected["model"] = json!(crate::claude::DEFAULT_MODEL);
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_slower_save_never_undoes_a_newer_look() {
        // Night → Ember → Frost → Night twenty times over, saves arriving out of order.
        let mut saved = ("night".to_string(), 0.0);
        let mut at = 1000.0;
        let mut sends = Vec::new();
        for round in 0..20 {
            for look in ["ember", "frost", "night"] {
                at += 1.0;
                sends.push((look, at));
            }
            if round % 3 == 0 {
                sends.reverse();
            }
        }
        for (look, t) in &sends {
            saved = keep_newer_look((&saved.0, saved.1), (look, *t));
        }
        assert_eq!(saved, ("night".to_string(), at));
        // An old copy without a stamp loses to any chosen look.
        assert_eq!(keep_newer_look(("frost", 5.0), ("ember", 0.0)).0, "frost");
        assert_eq!(keep_newer_look(("frost", 5.0), ("ember", f64::NAN)).0, "frost");
    }

    #[test]
    fn growth_never_goes_back_whichever_window_saves() {
        let old = serde_json::json!({ "activeSeconds": 700000.0, "reached": 3, "shown": 2 });
        let stale = serde_json::json!({ "activeSeconds": 1000.0, "reached": 0, "shown": null });
        let merged = merge_growth(&old, &stale);
        assert_eq!(merged["activeSeconds"], 700000.0);
        assert_eq!(merged["reached"], 3);
        assert_eq!(merged["shown"], Value::Null);
        // A newer record moves it on; a chosen stage, even a reached one, is dropped.
        assert_eq!(merge_growth(&old, &old)["shown"], Value::Null);
        let newer = serde_json::json!({ "activeSeconds": 800000.0, "reached": 4, "shown": 5 });
        let merged = merge_growth(&old, &newer);
        assert_eq!((merged["activeSeconds"].as_f64(), merged["reached"].as_u64()), (Some(800000.0), Some(4)));
        assert_eq!(merged["shown"], Value::Null);
        // Junk is nothing, never a crash.
        let merged = merge_growth(&serde_json::json!("x"), &serde_json::json!({ "activeSeconds": -5, "reached": 9 }));
        assert_eq!(merged, serde_json::json!({ "activeSeconds": 0.0, "reached": 0, "shown": null, "seen": -1 }));
        // The intro's stage is kept from whichever copy has it.
        let a = serde_json::json!({ "activeSeconds": 1.0, "reached": 3, "seen": 3 });
        assert_eq!(merge_growth(&a, &serde_json::json!({}))["seen"], 3);
    }

    #[test]
    fn a_file_from_before_the_wardrobe_wears_nothing() {
        let loaded = parse(&custom_with("mochiOutfit", None)).unwrap();
        assert_eq!(loaded.mochi_outfit, "none");
        assert_eq!(loaded.model, "some-model");
        assert!(!loaded.sound_enabled);
    }

    #[test]
    fn a_file_from_before_the_desktop_mochi_keeps_him_in_the_island() {
        let loaded = parse(&custom_with("desktopMochi", None)).unwrap();
        assert_eq!(loaded.desktop_mochi, DesktopMochiPref::default());
        assert!(!loaded.desktop_mochi.on_desktop);
        assert_eq!(loaded.mochi_outfit, "witchHat");
    }

    #[test]
    fn a_file_from_before_the_phone_link_keeps_it_off() {
        // The link opens a port on the local network: it is never on by default,
        // and an older file must not be read as consent.
        assert!(!Settings::default().phone_link);
        let loaded = parse(&custom_with("phoneLink", None)).unwrap();
        assert!(!loaded.phone_link);
        assert_eq!(loaded.mochi_outfit, "witchHat");
        assert!(parse(&custom_with("phoneLink", Some(serde_json::json!(true)))).unwrap().phone_link);

        // A value of the wrong type does not switch it on either, and costs nothing else.
        let (dir, file) = scratch("phone-link");
        std::fs::write(&file, custom_with("phoneLink", Some(serde_json::json!("yes")))).unwrap();
        let odd = load_from(&file);
        assert!(!odd.phone_link);
        assert_eq!(odd.mochi_outfit, "witchHat");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_from_before_phone_chat_keeps_it_off_and_empty() {
        // Chat spends the user's API credit: never on by default, and an older file is not consent.
        let d = Settings::default();
        assert!(!d.phone_chat);
        assert!(d.phone_chat_models.is_empty());
        let loaded = parse(&custom_with("phoneChat", None)).unwrap();
        assert!(!loaded.phone_chat);
        assert!(parse(&custom_with("phoneChat", Some(serde_json::json!(true)))).unwrap().phone_chat);
        let odd = parse(&custom_with("phoneChat", Some(serde_json::json!("yes")))).unwrap();
        assert!(!odd.phone_chat, "a wrong type does not switch it on");
        assert_eq!(odd.mochi_outfit, "witchHat");
        let models = parse(&custom_with("phoneChatModels", Some(serde_json::json!(["openai/gpt-x"])))).unwrap();
        assert_eq!(models.phone_chat_models, vec!["openai/gpt-x".to_string()]);
        let bad = parse(&custom_with("phoneChatModels", Some(serde_json::json!("openai/gpt-x")))).unwrap();
        assert!(bad.phone_chat_models.is_empty());
    }

    #[test]
    fn a_file_from_before_phone_answers_keeps_it_off() {
        // Answering for the agent from the phone is never on by default; an older file is not consent.
        assert!(!Settings::default().phone_answers);
        assert!(!parse(&custom_with("phoneAnswers", None)).unwrap().phone_answers);
        assert!(parse(&custom_with("phoneAnswers", Some(serde_json::json!(true)))).unwrap().phone_answers);
        assert!(!parse(&custom_with("phoneAnswers", Some(serde_json::json!("yes")))).unwrap().phone_answers, "a wrong type does not switch it on");
    }

    #[test]
    fn a_file_from_before_phone_diffs_keeps_it_off() {
        // Showing what an agent changed on the phone is never on by default; an older file is not consent.
        assert!(!Settings::default().phone_diffs);
        assert!(!parse(&custom_with("phoneDiffs", None)).unwrap().phone_diffs);
        assert!(parse(&custom_with("phoneDiffs", Some(serde_json::json!(true)))).unwrap().phone_diffs);
        assert!(!parse(&custom_with("phoneDiffs", Some(serde_json::json!("yes")))).unwrap().phone_diffs, "a wrong type does not switch it on");
    }

    #[test]
    fn a_file_from_before_phone_usage_keeps_it_off() {
        assert!(!Settings::default().phone_usage);
        assert!(!parse(&custom_with("phoneUsage", None)).unwrap().phone_usage);
        assert!(parse(&custom_with("phoneUsage", Some(serde_json::json!(true)))).unwrap().phone_usage);
        assert!(!parse(&custom_with("phoneUsage", Some(serde_json::json!("yes")))).unwrap().phone_usage, "a wrong type does not switch it on");
    }

    #[test]
    fn a_file_from_before_the_relay_keeps_it_off_and_empty() {
        assert!(!Settings::default().phone_relay);
        assert!(Settings::default().phone_relay_url.is_empty());
        let old = parse(&custom_with("phoneRelay", None)).unwrap();
        assert!(!old.phone_relay);
        assert!(parse(&custom_with("phoneRelay", Some(serde_json::json!(true)))).unwrap().phone_relay);
        assert!(!parse(&custom_with("phoneRelay", Some(serde_json::json!("yes")))).unwrap().phone_relay, "a wrong type does not switch it on");
        assert!(parse(&custom_with("phoneRelayUrl", Some(serde_json::json!(5)))).unwrap().phone_relay_url.is_empty());
    }

    #[test]
    fn a_file_from_before_phone_services_shows_none() {
        // Showing a service on the phone is never on by default; an older file is not consent.
        assert!(Settings::default().phone_services.is_empty());
        assert!(parse(&custom_with("phoneServices", None)).unwrap().phone_services.is_empty());
        assert_eq!(parse(&custom_with("phoneServices", Some(serde_json::json!(["integration_stripe"])))).unwrap().phone_services, vec!["integration_stripe".to_string()]);
        assert!(parse(&custom_with("phoneServices", Some(serde_json::json!("integration_stripe")))).unwrap().phone_services.is_empty(), "a wrong type shows none");
    }

    #[test]
    fn a_file_from_before_phone_details_keeps_them_off() {
        // Details can include commands and folder names: never on by default, an older file is not consent.
        assert!(!Settings::default().phone_details);
        assert!(!parse(&custom_with("phoneDetails", None)).unwrap().phone_details);
        assert!(parse(&custom_with("phoneDetails", Some(serde_json::json!(true)))).unwrap().phone_details);
        let odd = parse(&custom_with("phoneDetails", Some(serde_json::json!("yes")))).unwrap();
        assert!(!odd.phone_details, "a wrong type does not switch it on");
        assert_eq!(odd.mochi_outfit, "witchHat");
    }

    #[test]
    fn a_half_written_desktop_spot_costs_only_the_spot() {
        let loaded = parse(&custom_with(
            "desktopMochi",
            Some(json!({ "onDesktop": true, "spot": { "x": "left" } })),
        ))
        .unwrap();
        // The whole field falls back, and nothing else does.
        assert_eq!(loaded.desktop_mochi, DesktopMochiPref::default());
        assert_eq!(loaded.model, "some-model");

        let loaded = parse(&custom_with("desktopMochi", Some(json!({ "onDesktop": true })))).unwrap();
        assert!(loaded.desktop_mochi.on_desktop);
        assert_eq!(loaded.desktop_mochi.spot, None);
    }

    #[test]
    fn a_file_from_before_the_colours_paints_every_pill_as_the_catalog_says() {
        let loaded = parse(&custom_with("pillColors", None)).unwrap();
        assert!(loaded.pill_colors.is_empty());
        assert_eq!(loaded.mochi_outfit, "witchHat");
    }

    #[test]
    fn pill_colours_are_kept_as_written_and_cost_nothing_else_when_unusable() {
        // A pill a newer build added keeps its colour through a save by this one.
        let loaded = parse(&custom_with("pillColors", Some(json!({ "agent_new": "#abcdef" })))).unwrap();
        assert_eq!(loaded.pill_colors.get("agent_new").map(String::as_str), Some("#abcdef"));
        // Not a map of strings: the colours fall back, and nothing else does.
        let loaded = parse(&custom_with("pillColors", Some(json!(["#2DD4BF"])))).unwrap();
        assert!(loaded.pill_colors.is_empty());
        assert_eq!(loaded.model, "some-model");
    }

    #[test]
    fn an_outfit_this_build_does_not_know_is_kept_as_written() {
        // A newer build may add outfits: the island shows "auto" for it, but
        // the choice must survive a save made by this one.
        let loaded = parse(&custom_with("mochiOutfit", Some(json!("topHat")))).unwrap();
        assert_eq!(loaded.mochi_outfit, "topHat");
        // The language too: "" (follow the system) when absent, as it comes otherwise.
        assert_eq!(parse(&custom_with("language", None)).unwrap().language, "");
        assert_eq!(parse(&custom_with("language", Some(json!("xx")))).unwrap().language, "xx");
    }

    #[test]
    fn a_missing_field_takes_its_default_and_the_rest_are_kept() {
        for key in custom().as_object().unwrap().keys() {
            let loaded = parse(&custom_with(key, None))
                .unwrap_or_else(|| panic!("a file without {key} was thrown away"));
            let mut expected = custom();
            expected[key] = defaults()[key].clone();
            assert_eq!(shown(&loaded), expected, "without {key}");
        }
    }

    #[test]
    fn a_field_of_the_wrong_type_takes_its_default_and_the_rest_are_kept() {
        for (key, wrong) in [
            ("soundVolume", json!("loud")),
            ("soundEnabled", json!(1)),
            ("activeIntegrations", json!("integration_n8n")),
            ("mainPill", json!(["agent_cursor"])),
            ("screen", Value::Null),
            ("shortcuts", json!("Ctrl+Alt+A")),
        ] {
            let loaded = parse(&custom_with(key, Some(wrong)))
                .unwrap_or_else(|| panic!("a file with a bad {key} was thrown away"));
            let mut expected = custom();
            expected[key] = defaults()[key].clone();
            assert_eq!(shown(&loaded), expected, "bad {key}");
        }
    }

    #[test]
    fn a_utf8_bom_is_not_corruption() {
        // What PowerShell 5's `Set-Content -Encoding utf8` and Notepad write.
        let (dir, file) = scratch("bom");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(CUSTOM.as_bytes());
        std::fs::write(&file, &bytes).unwrap();
        assert_eq!(shown(&load_from(&file)), custom());
        assert_eq!(names(&dir), ["settings.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unusable_file_is_the_defaults_and_is_set_aside_byte_for_byte() {
        for (name, bad) in [
            ("invalid", &b"{ \"soundVolume\": 0.5, \xff oops"[..]),
            ("not-an-object", &b"[1, 2, 3]"[..]),
        ] {
            let (dir, file) = scratch(name);
            std::fs::write(&file, bad).unwrap();
            assert_eq!(shown(&load_from(&file)), defaults());
            assert_eq!(kept(&dir, "corrupt"), [bad.to_vec()], "{name}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_second_unusable_file_never_replaces_the_copy_of_the_first() {
        // Both within the same second: the names must still differ.
        let (dir, file) = scratch("twice");
        let (first, second) = (&b"{ first"[..], &b"{ second"[..]);
        std::fs::write(&file, first).unwrap();
        load_from(&file);
        std::fs::write(&file, second).unwrap();
        load_from(&file);
        let mut copies = kept(&dir, "corrupt");
        copies.sort();
        assert_eq!(copies, [first.to_vec(), second.to_vec()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn something_that_is_not_a_readable_file_is_the_defaults_and_is_left_alone() {
        // A directory in the file's place: a read error that is not "not found".
        let (dir, file) = scratch("unreadable");
        std::fs::create_dir(&file).unwrap();
        assert_eq!(shown(&load_from(&file)), defaults());
        assert_eq!(names(&dir), ["settings.json"]);
        assert!(file.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opens `path` the way a process that shares nothing does: nobody else can
    /// read or write it until the handle is dropped.
    #[cfg(windows)]
    fn locked(path: &Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).share_mode(0).open(path).unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn a_file_that_could_not_be_read_is_set_aside_before_it_is_saved_over() {
        let (dir, file) = scratch("locked-then-free");
        std::fs::write(&file, CUSTOM).unwrap();
        let lock = locked(&file);
        let loaded = load_from(&file);
        assert_eq!(shown(&loaded), defaults());
        drop(lock);

        save_to(&file, &loaded).unwrap();
        assert_eq!(kept(&dir, "unread"), [CUSTOM.as_bytes().to_vec()]);
        assert_eq!(shown(&load_from(&file)), defaults());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn a_file_that_still_cannot_be_read_is_not_saved_over() {
        let (dir, file) = scratch("locked");
        std::fs::write(&file, CUSTOM).unwrap();
        let lock = locked(&file);
        let loaded = load_from(&file);
        assert!(save_to(&file, &loaded).is_err());
        drop(lock);
        assert_eq!(std::fs::read(&file).unwrap(), CUSTOM.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_could_not_be_read_is_never_saved_over_unseen() {
        // The same as the two locked-file tests above, on every platform: a
        // directory in the file's place cannot be read either.
        let (dir, file) = scratch("unread");
        std::fs::create_dir(&file).unwrap();
        let loaded = load_from(&file);

        // Still unreadable: the save is refused and nothing is touched.
        assert!(save_to(&file, &loaded).is_err());
        assert!(file.is_dir());
        assert_eq!(names(&dir), ["settings.json"]);

        // Readable by now: kept aside first, and only then saved over.
        std::fs::remove_dir(&file).unwrap();
        std::fs::write(&file, CUSTOM).unwrap();
        save_to(&file, &loaded).unwrap();
        assert_eq!(kept(&dir, "unread"), [CUSTOM.as_bytes().to_vec()]);
        assert_eq!(shown(&load_from(&file)), defaults());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Copies set aside ──────────────────────────────────────────────────────

    #[test]
    fn an_unusable_file_is_set_aside_once_however_often_it_is_loaded() {
        let (dir, file) = scratch("once");
        std::fs::write(&file, b"{ broken").unwrap();
        for _ in 0..3 {
            load_from(&file);
        }
        assert_eq!(kept(&dir, "corrupt"), [b"{ broken".to_vec()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copies_made_within_the_same_second_are_numbered_and_none_is_written_over() {
        let (dir, file) = scratch("numbered");
        let stamp = "20260102-030405";
        let contents = [&b"one"[..], &b"two"[..], &b"three"[..]];
        let copies: Vec<PathBuf> = contents
            .iter()
            .map(|bytes| set_aside_as(&file, "corrupt", stamp, bytes, write_whole).unwrap())
            .collect();

        let named = |suffix: &str| dir.join(format!("settings.json.corrupt-{stamp}{suffix}"));
        assert_eq!(copies, [named(""), named("-1"), named("-2")]);
        for (copy, bytes) in copies.iter().zip(contents) {
            assert_eq!(std::fs::read(copy).unwrap(), bytes);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_that_could_not_be_written_whole_is_removed() {
        fn half_then_fail(file: &mut std::fs::File, bytes: &[u8]) -> std::io::Result<()> {
            use std::io::Write;
            file.write_all(&bytes[..bytes.len() / 2])?;
            Err(std::io::Error::other("the disk is full"))
        }
        let (dir, file) = scratch("half");
        let copy = set_aside_as(&file, "corrupt", "20260102-030405", b"all of it", half_then_fail);
        assert!(copy.is_err());
        assert!(names(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_field_name_cannot_start_a_line_of_its_own_in_the_log() {
        let forged = "volume\r\n2026-01-02 03:04:05 hook id=1 answered allow";
        let line = unusable_field(forged);
        assert!(!line.contains(['\r', '\n']));
        assert!(line.contains(r"volume\r\n2026-01-02"));
    }

    // ── Writing ───────────────────────────────────────────────────────────────

    /// A dotfiles setup keeps the real file elsewhere and links to it.
    #[cfg(unix)]
    #[test]
    fn a_save_through_a_symlink_keeps_the_link_and_writes_its_target() {
        let (dir, file) = scratch("symlink");
        let real = dir.join("dotfiles-settings.json");
        std::fs::write(&real, CUSTOM).unwrap();
        std::os::unix::fs::symlink(&real, &file).unwrap();

        save_to(&file, &Settings::default()).unwrap();
        assert!(std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink());
        assert_eq!(shown(&parse(&std::fs::read(&real).unwrap()).unwrap()), defaults());
        assert_eq!(names(&dir), ["dotfiles-settings.json", "settings.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_field_survives_a_save_and_a_load() {
        let (dir, file) = scratch("round-trip");
        let settings: Settings = serde_json::from_str(CUSTOM).unwrap();
        save_to(&file, &settings).unwrap();
        assert_eq!(shown(&load_from(&file)), custom());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_file_holds_the_camel_case_keys_the_front_end_uses() {
        let (dir, file) = scratch("keys");
        save_to(&file, &Settings::default()).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let keys: Vec<&str> = written.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "soundEnabled",
                "soundVolume",
                "autoCloseInterval",
                "openOnHover",
                "absenceInterval",
                "activeIntegrations",
                "mainPill",
                "screen",
                "autostart",
                "hooksInstalled",
                "model",
                "showPlanInNotch",
                "planRelayInstalled",
                "showCodexPlanInNotch",
                "chatProvider",
                "chatModels",
                "ollamaUrl",
                "lmstudioUrl",
                "customUrl",
                "shortcuts",
                "mochiOutfit",
                "groovyLook",
                "lookAt",
                "voiceEnabled",
                "voiceEngine",
                "voiceModel",
                "livekitUrl",
                "livekitRoom",
                "islandX",
                "voiceCall",
                "voiceSpeak",
                "voiceTts",
                "voiceLanguage",
                "voiceMic",
                "voiceInterrupt",
                "breakWater",
                "keepVisible",
                "breakStretch",
                "breakEyes",
                "pillColors",
                "language",
                "desktopMochi",
                "phoneLink",
                "phoneChat",
                "phoneChatModels",
                "phoneDetails",
                "phoneAnswers",
                "phoneDiffs",
                "phoneUsage",
                "phoneServices",
                "phoneRelay",
                "phoneRelayUrl",
                "assistant",
                "wardrobe",
                "islandTiles",
                "closeOnLeaveOffered",
                "islandStyle",
                "listeningView",
                "growth",
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_replaces_the_file_that_was_there() {
        let (dir, file) = scratch("replace");
        std::fs::write(&file, CUSTOM).unwrap();
        save_to(&file, &Settings::default()).unwrap();
        assert_eq!(shown(&load_from(&file)), defaults());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_leaves_nothing_but_settings_json_behind() {
        let (dir, file) = scratch("tidy");
        save_to(&file, &Settings::default()).unwrap();
        save_to(&file, &Settings::default()).unwrap();
        assert_eq!(names(&dir), ["settings.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_temporary_file_that_cannot_be_written_leaves_the_original_alone() {
        // The new contents go to a file beside the target first. With a
        // directory squatting on that name, the write cannot even start.
        let (dir, file) = scratch("blocked");
        std::fs::write(&file, CUSTOM).unwrap();
        let temp = dir.join(format!("settings.json.groovy-{}", std::process::id()));
        std::fs::create_dir(&temp).unwrap();

        assert!(save_to(&file, &Settings::default()).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), CUSTOM.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
