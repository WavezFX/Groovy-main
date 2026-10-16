// Groovy for Windows — app wiring and the commands the island calls.

mod agent_hooks;
mod agents;
mod assistant;
mod brain_ai;
mod chat;
mod claude;
mod codex_plan;
mod config_file;
mod db;
mod desktop;
mod files;
mod flow;
mod glass;
mod github;
mod hooks;
mod i18n;
mod idle;
mod browser;
mod calls;
mod worktest;
mod identity;
mod integrations;
mod island;
mod island_tiles;
mod livekit;
mod local_chat;
mod log;
mod memory;
mod models;
mod music;
mod notes;
mod okgroovy;
mod net;
mod ollama;
mod openai_compat;
mod phone_link;
mod pipe;
mod platform;
#[cfg(target_os = "linux")]
mod portal;
mod recap;
mod secrets;
mod session_window;
mod settings;
mod shortcuts;
mod sounds;
mod skills;
mod speech;
mod spotify;
mod studio;
mod system;
mod tasks;
mod tools;
mod tray;
mod voice;
#[cfg(windows)]
mod webview_drop;
mod webview_memory;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

use chat::{Chat, ChatContext, ChatReply, ModelInfo};
use files::DroppedFile;
use hooks::{HookPreview, HookStatus};
use island::{PollGate, ScreenInfo};
use pipe::Pending;
use settings::Settings;

pub struct Shared {
    pub settings: Mutex<Settings>,
    pub gate: Arc<PollGate>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootInfo {
    settings: Settings,
    screen: ScreenInfo,
    version: String,
    hook_path: String,
    /// False where the OS has no global cursor (Wayland): the page then reports
    /// the cursor from its own mouse events.
    cursor_poll: bool,
    /// CI's smoke test (GROOVY_SMOKE_VIEW): hold the island open on "home"
    /// (F) or "player" (E) so its corners can be checked on screen.
    smoke_view: Option<&'static str>,
}

/// The view the smoke test asked for, if it is one it may ask for.
fn smoke_view(value: Option<&str>) -> Option<&'static str> {
    match value? {
        "home" => Some("home"),
        "player" => Some("player"),
        "frames" => Some("frames"),
        _ => None,
    }
}

/// CI's frame-time test (round 3B item 8b): the island's frame times, written
/// to frame-test.json for the workflow. Only while that test runs.
#[tauri::command]
fn frame_report(report: serde_json::Value) -> Result<(), String> {
    if smoke_view(std::env::var("GROOVY_SMOKE_VIEW").ok().as_deref()) != Some("frames") {
        return Err("not in the frame test".into());
    }
    log::line(format!("frame test: {}", report["stats"]));
    let path = platform::local_dir().join("frame-test.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
fn boot(app: AppHandle, shared: State<Shared>) -> BootInfo {
    let mut settings = shared.settings.lock().unwrap().clone();
    // The real state of ~/.claude/settings.json wins over whatever we stored.
    let hooks_status = hooks::status();
    settings.hooks_installed = hooks_status.installed;
    settings.plan_relay_installed = hooks_status.plan_relay_installed;
    let screen = island::screen_info(&app, &settings.screen);
    // The Windows smoke test waits for this line: it proves a webview loaded
    // the interface and reached the backend.
    log::line(format!("boot: interface loaded ({})", env!("CARGO_PKG_VERSION")));
    BootInfo {
        settings,
        screen,
        version: env!("CARGO_PKG_VERSION").to_string(),
        hook_path: settings::hook_exe_path().to_string_lossy().to_string(),
        cursor_poll: platform::CURSOR_POLL,
        smoke_view: smoke_view(std::env::var("GROOVY_SMOKE_VIEW").ok().as_deref()),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, shared: State<Shared>, settings: Settings) {
    let (screen_changed, autostart_changed, shortcuts_changed) = {
        let mut current = shared.settings.lock().unwrap();
        let screen_changed = current.screen != settings.screen;
        let autostart_changed = current.autostart != settings.autostart;
        let shortcuts_changed = current.shortcuts != settings.shortcuts;
        // Where Groovy sits on the desktop is desktop.rs's to say, not a webview's.
        let mut settings = settings.clone();
        settings.assistant = settings.assistant.sanitised();
        settings.desktop_mochi = current.desktop_mochi.clone();
        // So is where the island sits along the top (island_move).
        settings.island_x = current.island_x;
        // Same for the phone link: only its own commands turn it on or off.
        settings.phone_link = current.phone_link;
        settings.phone_chat = current.phone_chat;
        settings.phone_details = current.phone_details;
        settings.phone_answers = current.phone_answers;
        settings.phone_diffs = current.phone_diffs;
        settings.phone_usage = current.phone_usage;
        settings.phone_services = current.phone_services.clone();
        settings.phone_relay = current.phone_relay;
        settings.phone_relay_url = current.phone_relay_url.clone();
        settings.phone_chat_models = current.phone_chat_models.clone();
        // Two windows hold a copy: growth only ever moves forward, and the
        // stage shown is save_growth's to change.
        settings.growth = settings::merge_growth(&settings.growth, &current.growth);
        let (look, look_at) = settings::keep_newer_look((&current.groovy_look, current.look_at), (&settings.groovy_look, settings.look_at));
        settings.groovy_look = look;
        settings.look_at = look_at;
        *current = settings;
        (screen_changed, autostart_changed, shortcuts_changed)
    };
    let settings = shared.settings.lock().unwrap().clone();
    if let Err(err) = settings::save(&settings) {
        log::line(format!("could not save settings: {err}"));
    }
    if autostart_changed {
        let manager = app.autolaunch();
        let result = if settings.autostart { manager.enable() } else { manager.disable() };
        if let Err(err) = result {
            eprintln!("[groovy] autostart: {err}");
        }
    }
    if screen_changed {
        let collapsed = shared.gate.collapsed.load(Ordering::Relaxed);
        island::apply_geometry(&app, &settings.screen, collapsed);
    }
    integrations::settings_saved(&app, &settings.active_integrations);
    // The voice model or language may be the dictation engine's too.
    flow::sync_engine(&app);
    spotify::sync(&app, &settings.active_integrations);
    glass::apply(&app);
    idle::set(&settings.unload_after);
    browser::configure(&settings.browser, &settings.search_engine);
    if shortcuts_changed {
        shortcuts::apply(&app, &settings.shortcuts);
    }
    if i18n::set_picked(&settings.language) {
        language_changed(&app);
    }
    // Keep the other window in step (island ⇄ settings window).
    let _ = app.emit("settings-changed", settings);
}

/// The island reports the system's languages at launch, for "System" in
/// Settings → Language (WebView2 and WebKitGTK know them best).
#[tauri::command]
fn set_system_languages(app: AppHandle, languages: Vec<String>) {
    if i18n::set_system(languages) {
        language_changed(&app);
    }
}

/// What Rust labels itself follows the new language: the tray menu and the
/// settings window's title. The webviews switch on their own.
fn language_changed(app: &AppHandle) {
    tray::retitle(app);
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(&i18n::t("Settings — Groovy"));
    }
}

/// Hidden island → shrink the window to the invisible wake strip and park the
/// cursor poll; anything else → full panel and 60 Hz polling.
#[tauri::command]
fn set_collapsed(app: AppHandle, shared: State<Shared>, collapsed: bool) {
    let pref = shared.settings.lock().unwrap().screen.clone();
    shared.gate.collapsed.store(collapsed, Ordering::Relaxed);
    island::apply_geometry(&app, &pref, collapsed);
    // The wake strip must always take the mouse, and a resize invalidates the flag.
    island::refresh_click_through(&app, &shared.gate);
    shared.gate.set_active(!collapsed);
    platform::set_pointer_watch(!collapsed);
    // Folded away: its webview keeps as little memory as it can until it opens.
    if let Some(win) = island::window(&app) {
        webview_memory::set_low(&win, collapsed);
    }
}

/// The front end pushes the island shape; Rust decides click-through from it.
#[tauri::command]
fn set_island_rect(app: AppHandle, shared: State<Shared>, x: f64, y: f64, width: f64, height: f64, radius: Option<f64>) {
    shared.gate.set_rect(island::IslandRect { x, y, w: width, h: height });
    glass::shape(&app, x, y, width, height, radius.unwrap_or(0.0));
    // Without the cursor poll the input region is the click-through: it follows the island.
    if !platform::CURSOR_POLL {
        island::refresh_click_through(&app, &shared.gate);
    }
}

/// The island's growth clock (round 3 item 6): only this field, merged so it
/// never goes back, without redrawing every window (no settings-changed). The
/// Control Centre hears it as growth-changed for its look cards.
#[tauri::command]
fn save_growth(app: AppHandle, shared: State<Shared>, growth: serde_json::Value) {
    let settings = {
        let mut s = shared.settings.lock().unwrap();
        s.growth = settings::merge_growth(&s.growth, &growth);
        s.clone()
    };
    if let Err(err) = settings::save(&settings) {
        log::line(format!("could not save settings: {err}"));
    }
    let _ = app.emit("growth-changed", settings.growth);
}

/// The island page asks for the window to be the island's size (round 3 §8 B).
#[tauri::command]
fn fit_island(app: AppHandle, width: f64, height: f64) {
    island::fit(&app, width, height);
}

#[tauri::command]
fn focus_window(app: AppHandle, focused: bool) {
    let Some(win) = island::window(&app) else { return };
    platform::set_activating(&win, focused);
    if focused {
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn reposition(app: AppHandle, shared: State<Shared>) {
    let pref = shared.settings.lock().unwrap().screen.clone();
    let collapsed = shared.gate.collapsed.load(Ordering::Relaxed);
    island::apply_geometry(&app, &pref, collapsed);
}

/// The displays the island can be pinned to, for Settings.
#[tauri::command]
fn list_monitors(app: AppHandle) -> Vec<island::MonitorChoice> {
    island::monitor_choices(&app)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(i18n::t("Only web addresses (https://…) open from here."));
    }
    // The browser chosen in Settings → Browser (round 4 item 4).
    browser::open(&url).map(|_| ())
}

/// "Open terminal" opens the working folder in VS Code when `code` is on PATH,
/// and falls back to the file manager otherwise.
#[tauri::command]
fn open_in_vscode(path: Option<String>) -> bool {
    // No shell anywhere near this. The path is a project folder chosen by
    // whoever is using Claude Code, and a shell would happily read `&`, `^`, `%`
    // or `$` in a folder name as syntax. Finding the launcher ourselves and
    // handing the path over as a separate argument keeps it a path.
    let path = path.filter(|p| !p.is_empty());
    // It arrives in a hook payload: only an existing folder, given by its full
    // path, goes any further. `code` would read `--something` as an option, and
    // xdg-open would launch a file with whatever handles its type.
    if let Some(p) = path.as_deref() {
        let p = std::path::Path::new(p);
        if !(p.is_absolute() && p.is_dir()) {
            return false;
        }
    }
    if let Some(code) = platform::find_on_path("code") {
        let mut cmd = Command::new(code);
        if let Some(p) = path.as_deref() {
            cmd.arg(p);
        }
        if platform::no_console(&mut cmd).spawn().is_ok() {
            return true;
        }
    }
    if let Some(p) = path.as_deref() {
        platform::reveal_folder(p);
    }
    false
}

/// "Open terminal": brings forward the terminal or editor window the session
/// runs in, when it was found (see session_window.rs); otherwise opens the
/// folder in VS Code, as before.
#[tauri::command]
fn open_session(session_id: Option<String>, path: Option<String>) -> bool {
    let Some(owners) = session_id.as_deref().and_then(session_window::lookup) else {
        return open_in_vscode(path);
    };
    let folder = path.as_deref().map(session_window::folder_name).unwrap_or_default().to_string();
    #[cfg(windows)]
    {
        if platform::focus_session_window(&owners, &folder) {
            return true;
        }
        open_in_vscode(path)
    }
    // Linux asks the display server, KWin or the terminal, which can take a
    // moment: never on the UI thread a sync command runs on. VS Code still
    // opens when none of them could bring the window forward.
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            if !platform::focus_session_window(&owners, &folder) {
                open_in_vscode(path);
            }
        });
        true
    }
}

/// The Claude Desktop pill's target: the Claude app (Windows only — it has no
/// Linux build).
#[tauri::command]
fn open_claude_desktop() -> bool {
    platform::open_claude_desktop()
}

/// The file behind a live diff, if it may be handed to the editor: an existing
/// regular file given by its full path. Anything else — a relative path, a
/// folder, a path `code` could read as an option — goes no further.
fn diff_file(path: &str) -> Option<&std::path::Path> {
    let p = std::path::Path::new(path);
    (p.is_absolute() && p.is_file()).then_some(p)
}

/// The diff card's ↗: opens the edited file in VS Code when `code` is on PATH,
/// otherwise shows its folder. The file itself is never opened by its type —
/// xdg-open or Explorer would run a script that Claude just wrote.
#[tauri::command]
fn open_file_in_vscode(path: String) -> bool {
    let Some(file) = diff_file(&path) else { return false };
    if let Some(code) = platform::find_on_path("code") {
        let mut cmd = Command::new(code);
        cmd.arg(file);
        if platform::no_console(&mut cmd).spawn().is_ok() {
            return true;
        }
    }
    if let Some(folder) = file.parent().filter(|d| d.is_dir()) {
        platform::reveal_folder(&folder.to_string_lossy());
    }
    false
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Tray → Pause. Paused means paused: the pollers stop talking to the network,
/// not just the island stopping showing things.
#[tauri::command]
fn set_paused(paused: bool) {
    integrations::set_paused(paused);
}

// ── Claude Code hooks ─────────────────────────────────────────────────────────

#[tauri::command]
fn hooks_status() -> HookStatus {
    hooks::status()
}

/// Pill ID → whether that agent's hooks reach Groovy. Read-only.
#[tauri::command]
fn agent_hooks_status() -> std::collections::HashMap<String, bool> {
    agent_hooks::status()
}

/// Returns the diff the user has to look at before anything is written.
#[tauri::command]
fn hooks_preview(install: bool) -> Result<HookPreview, String> {
    hooks::preview(install)
}

/// Only ever called from an explicit click in the settings window.
#[tauri::command]
fn hooks_apply(
    app: AppHandle,
    shared: State<Shared>,
    install: bool,
    fingerprint: String,
) -> Result<String, String> {
    // The fingerprint comes from the preview the user actually looked at, so a
    // settings.json that changed in between is refused rather than overwritten.
    let backup = hooks::write(install, &fingerprint)?;
    let updated = {
        let mut current = shared.settings.lock().unwrap();
        current.hooks_installed = install;
        if let Err(err) = settings::save(&current) {
            log::line(format!("could not save settings: {err}"));
        }
        current.clone()
    };
    let _ = app.emit("settings-changed", updated);
    Ok(backup)
}

// ── Other agents' hooks and plugins ──────────────────────────────────────────

#[tauri::command]
fn agent_hooks_list() -> Vec<agents::AgentStatus> {
    agents::list()
}

/// The diff the user has to look at before anything is written.
#[tauri::command]
fn agent_hooks_preview(agent: String, install: bool) -> Result<config_file::Plan, String> {
    agents::preview(&agent, install)
}

/// Only ever called from an explicit click in the settings window, with the
/// fingerprint of the preview the user looked at.
#[tauri::command]
fn agent_hooks_apply(agent: String, install: bool, fingerprint: String) -> Result<String, String> {
    let backups = agents::apply(&agent, install, &fingerprint)?;
    let done = if install { "installed" } else { "removed" };
    log::line(format!("agent hooks {done} for {agent}"));
    Ok(backups)
}

// ── Plan usage ────────────────────────────────────────────────────────────────

/// The diff of putting the plan usage relay into (or taking it out of) the
/// status line, before anything is written.
#[tauri::command]
fn status_line_preview(install: bool) -> Result<HookPreview, String> {
    hooks::status_line_preview(install)
}

/// Only ever called from an explicit click in the settings window.
#[tauri::command]
fn status_line_apply(
    app: AppHandle,
    shared: State<Shared>,
    install: bool,
    fingerprint: String,
) -> Result<String, String> {
    let backup = hooks::status_line_write(install, &fingerprint)?;
    let updated = {
        let mut current = shared.settings.lock().unwrap();
        current.plan_relay_installed = install;
        // As on the Mac: taking the relay out turns the pill off with it.
        if !install {
            current.show_plan_in_notch = false;
        }
        if let Err(err) = settings::save(&current) {
            log::line(format!("could not save settings: {err}"));
        }
        current.clone()
    };
    let _ = app.emit("settings-changed", updated);
    Ok(backup)
}

/// Codex plan usage, asked of the Codex CLI (`codex app-server`) when its pill
/// shows. Off the main thread: it can take a few seconds.
#[tauri::command]
async fn codex_plan_usage() -> Option<serde_json::Value> {
    tauri::async_runtime::spawn_blocking(codex_plan::read).await.ok().flatten()
}

#[tauri::command]
fn approval_decision(app: AppHandle, request_id: String, decision: String) {
    recap::record_decision(&app, &request_id, &decision);
    pipe::answer(&app, &request_id, &decision);
}

/// An option picked on the island for a question Claude Code asked.
#[tauri::command]
fn approval_answer(
    app: AppHandle,
    request_id: String,
    answers: std::collections::HashMap<String, serde_json::Value>,
) {
    // An answered question is not an Allow / Deny: nothing for the recap.
    recap::forget_request(&app, &request_id);
    pipe::answer_question(&app, &request_id, &answers);
}

/// The island has the card on screen, so the long wait for a human may begin.
/// Until this arrives the relay only waits a few hundred milliseconds, which is
/// what stops a paused or unresponsive island from freezing Claude Code.
#[tauri::command]
fn approval_ack(app: AppHandle, request_id: String) {
    pipe::acknowledge(&app, &request_id);
}

/// Nobody can act on this request — the island is paused, or another card is
/// already up. Claude Code falls back to asking in the terminal immediately.
#[tauri::command]
fn approval_decline(app: AppHandle, request_id: String) {
    recap::forget_request(&app, &request_id);
    pipe::decline(&app, &request_id);
}

// ── Chat, files and secrets ───────────────────────────────────────────────────

/// One chat turn with the provider picked in the chat view. API keys and any
/// file bytes stay on the Rust side.
#[tauri::command]
async fn chat_send(
    app: AppHandle,
    shared: State<'_, Shared>,
    chat: State<'_, Chat>,
    query: String,
    context: Option<ChatContext>,
    spoken: Option<bool>,
) -> Result<ChatReply, String> {
    let settings = shared.settings.lock().unwrap().clone();
    chat.set_spoken(spoken.unwrap_or(false));
    chat::send(&app, &chat, &settings, query, context).await
}

/// "Test my voice setup", step 3: "say hello" to the brain the calls use.
#[tauri::command]
async fn voice_probe(app: AppHandle, shared: State<'_, Shared>) -> Result<serde_json::Value, String> {
    let settings = shared.settings.lock().unwrap().clone();
    let started = std::time::Instant::now();
    let r = chat::probe(&app, &settings, "Say hello in one short sentence.").await;
    match &r {
        Ok((answer, who)) => log::voice(format!("test: brain {who} answered in {:.1}s ({} chars)", started.elapsed().as_secs_f64(), answer.len())),
        Err(e) => log::voice(format!("test: brain failed: {e}")),
    }
    let (answer, who) = r?;
    Ok(serde_json::json!({ "answer": answer, "brain": who }))
}

/// One line for voice.log from the island (the call's stages).
#[tauri::command]
fn voice_log(line: String) {
    log::voice(line.chars().take(400).collect::<String>());
}

/// Dragging the island along the top of the screen: `fraction` 0 = left edge,
/// 1 = right edge. Saved when the drag ends.
#[tauri::command]
fn island_move(app: AppHandle, shared: State<'_, Shared>, fraction: f64, save: bool) {
    let fraction = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.5 };
    let (screen, collapsed, settings) = {
        let mut s = shared.settings.lock().unwrap();
        s.island_x = fraction;
        (s.screen.clone(), shared.gate.collapsed.load(Ordering::Relaxed), s.clone())
    };
    island::apply_geometry(&app, &screen, collapsed);
    if save {
        if let Err(err) = settings::save(&settings) {
            log::line(format!("could not save settings: {err}"));
        }
    }
}

/// Where the island sits along the top, and the display's free width in
/// logical pixels, so a drag can turn pixels into a fraction.
#[tauri::command]
fn island_track(app: AppHandle, shared: State<'_, Shared>) -> (f64, f64) {
    let (screen, fraction) = {
        let s = shared.settings.lock().unwrap();
        (s.screen.clone(), s.island_x)
    };
    let info = island::screen_info(&app, &screen);
    (fraction, (info.width - island::PANEL_W).max(1.0))
}

// ── The offline brain (ollama.rs) ─────────────────────────────────────────────

#[tauri::command]
async fn ollama_status() -> ollama::OllamaStatus {
    ollama::status().await
}

/// One click: install Ollama if needed, start it, download a model, and make
/// it the chat's brain.
#[tauri::command]
async fn ollama_setup(app: AppHandle, shared: State<'_, Shared>) -> Result<String, String> {
    let model = ollama::setup(&app).await?;
    let settings = {
        let mut s = shared.settings.lock().unwrap();
        s.chat_provider = "ollama".into();
        s.chat_models.insert("ollama".into(), model.clone());
        s.clone()
    };
    if let Err(err) = settings::save(&settings) {
        log::line(format!("could not save settings: {err}"));
    }
    let _ = app.emit("settings-changed", &settings);
    Ok(model)
}

// ── Voice (voice.rs, livekit.rs) ──────────────────────────────────────────────

/// One spoken segment to text, with the Whisper model chosen in Settings.
#[tauri::command]
async fn voice_transcribe(app: AppHandle, shared: State<'_, Shared>, samples: Vec<i16>, wake: bool) -> Result<String, String> {
    let (model, language) = {
        let s = shared.settings.lock().unwrap();
        (s.voice_model.clone(), s.voice_language.clone())
    };
    let language = voice::language_for(&model, &language);
    // Round 3B item 10: the small spotter listens for "Hey Groovy"; the full
    // engine is loaded and asked only once it has heard it.
    let spotted = if wake {
        let pcm: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
        match tauri::async_runtime::spawn_blocking(move || speech::kws::heard_shared(&pcm)).await.map_err(|e| e.to_string())? {
            Some(false) => return Ok(String::new()),
            Some(true) => true,
            None => false,
        }
    } else {
        false
    };
    let text = voice_words(&app, model, language, samples, wake).await?;
    // Heard by the spotter, written oddly by the engine: the wake phrase is put back.
    if spotted && okgroovy::wake::after_wake(&text).is_none() {
        return Ok(format!("Hey Groovy, {}", okgroovy::wake::drop_wake_words(&text)).trim_end_matches([',', ' ']).to_string());
    }
    Ok(text)
}

async fn voice_words(app: &AppHandle, model: String, language: Option<String>, samples: Vec<i16>, wake: bool) -> Result<String, String> {
    let app = app.clone();
    // One speech engine for everything (round 2): calls use what dictation,
    // the Notetaker and « OK Groovy » use — Parakeet, or Whisper when chosen.
    let whisper = flow::uses_whisper(&app);
    if whisper {
        return voice::transcribe(model, language, samples, wake).await;
    }
    let started = std::time::Instant::now();
    let n = samples.len();
    let text = tauri::async_runtime::spawn_blocking(move || {
        let pcm: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
        speech::transcribe(&pcm).map(|t| t.text)
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| i18n::t("The speech model isn't downloaded yet: set it up in Groovy → Voice and dictation."))?;
    if !wake {
        log::voice(format!("heard {:.1}s of speech in {:.1}s: {} chars", n as f64 / 16000.0, started.elapsed().as_secs_f64(), text.len()));
    }
    Ok(text)
}

/// Quick settings (round 4 item 2): the AI, « OK Groovy » and the voice, in one call.
#[tauri::command]
fn quick_status(app: AppHandle) -> serde_json::Value {
    let ai: Vec<serde_json::Value> = brain_ai::states().into_iter().map(|(name, state)| serde_json::json!({ "name": name, "state": state })).collect();
    serde_json::json!({
        "ai": ai,
        "ok": okgroovy::okgroovy_status(app.clone()),
        "voice": speech::tts::tts_status(app).speaking,
    })
}

#[tauri::command]
fn voice_status(app: AppHandle, shared: State<'_, Shared>) -> voice::VoiceStatus {
    let model = shared.settings.lock().unwrap().voice_model.clone();
    let mut status = voice::status(&model);
    if !flow::uses_whisper(&app) {
        // Parakeet: the shared engine, always built in.
        status.available = true;
        status.ready = speech::is_ready();
    }
    status
}

/// Settings → Voice → Download: only on the user's click.
#[tauri::command]
async fn voice_download_model(app: AppHandle, model: String) -> Result<(), String> {
    voice::download(&app, &model).await
}

#[tauri::command]
fn voice_delete_model(model: String) -> Result<(), String> {
    voice::delete(&model)
}

/// A short-lived token for the user's LiveKit room, signed here with the
/// secret from the keychain; the secret itself never reaches the island.
#[tauri::command]
fn livekit_token(shared: State<'_, Shared>) -> Result<livekit::Access, String> {
    let (url, room) = {
        let s = shared.settings.lock().unwrap();
        (s.livekit_url.clone(), s.livekit_room.clone())
    };
    livekit::token(&url, &room, secrets::get("livekit-api-key"), secrets::get("livekit-api-secret"))
}

/// The models a provider offers, for the picker in the chat view. Only asked
/// once the user picked that provider, and only with its key or address.
#[tauri::command]
async fn chat_models(shared: State<'_, Shared>, provider: String) -> Result<Vec<ModelInfo>, String> {
    let settings = shared.settings.lock().unwrap().clone();
    chat::models(&settings, &provider).await
}

/// Settings → Local models → Connect: does the server answer, and with which models?
#[tauri::command]
async fn local_connect(provider: String, url: String) -> Result<local_chat::Connected, String> {
    local_chat::connect(&provider, &url).await
}

/// Stores the custom server's key for the address typed next to it; it is only
/// ever sent to that address.
#[tauri::command]
fn local_set_key(url: String, key: String) -> Result<(), String> {
    local_chat::set_custom_key(&url, &key)
}

#[tauri::command]
fn chat_reset(chat: State<Chat>) {
    chat.reset();
}

/// Copies a dropped file into the inbox and reports its name back.
#[tauri::command]
fn ingest_file(path: String) -> Result<DroppedFile, String> {
    files::ingest(&path)
}

/// The island may only ask whether a key exists — never read it.
#[tauri::command]
fn secret_present(key: String) -> bool {
    secrets::present(&key)
}

#[tauri::command]
fn secret_set(app: AppHandle, key: String, value: String) -> Result<(), String> {
    // Bound to its server's address: only local_set_key may store it.
    if key == local_chat::CUSTOM_KEY {
        return Err("use local_set_key".into());
    }
    let before = (key == "github-token").then(|| secrets::get(&key));
    secrets::set(&key, &value)?;
    if let Some(before) = before {
        if secrets::get(&key) != before {
            integrations::github_token_changed(&app);
        }
    }
    // A Claude key can make « OK Groovy » available (brain_ai.rs).
    if key == claude::KEY {
        okgroovy::sync(&app);
    }
    Ok(())
}

#[tauri::command]
fn secret_clear(app: AppHandle, key: String) -> Result<(), String> {
    let had = key == "github-token" && secrets::present(&key);
    secrets::clear(&key)?;
    if had {
        integrations::github_token_changed(&app);
    }
    if key == claude::KEY {
        okgroovy::sync(&app);
    }
    Ok(())
}

/// Changes one setting from the backend, saves it and tells the windows.
pub(crate) fn set_setting(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    let Some(shared) = app.try_state::<Shared>() else { return };
    let updated = {
        let mut current = shared.settings.lock().unwrap();
        change(&mut current);
        if let Err(err) = settings::save(&current) {
            log::line(format!("could not save settings: {err}"));
        }
        current.clone()
    };
    let _ = app.emit("settings-changed", updated);
}

/// Opens the configured n8n instance — the URL lives in the Credential Manager.
#[tauri::command]
fn open_n8n() -> Result<(), String> {
    match secrets::get("n8n-url") {
        Some(url) => open_url(url),
        None => Ok(()),
    }
}

/// Refresh buttons in the integration cards.
#[tauri::command]
async fn refresh_integration(app: AppHandle, id: String) {
    integrations::poll_once(app, &id).await;
}

/// The GitHub card was opened: refetch its `section` ("pulse" or "activity")
/// if it is stale. The pollers still decline while the pill is off or paused.
#[tauri::command]
fn github_refresh(section: String) {
    integrations::github_refresh_if_stale(&section);
}

/// Lets the island write to the same log as the Rust side.
#[tauri::command]
fn log_line(message: String) {
    log::line(format!("ui  {message}"));
}

// ── Global shortcuts ──────────────────────────────────────────────────────────

/// How each global shortcut went: registered, taken by another app, and so on.
#[tauri::command]
fn shortcuts_status(app: AppHandle) -> shortcuts::Report {
    shortcuts::status(&app)
}

/// Settings is recording a new combination: let go of ours meanwhile, so the
/// keys reach the recorder instead of running an action. `false` takes them back.
#[tauri::command]
fn shortcuts_suspend(app: AppHandle, shared: State<Shared>, suspended: bool) {
    if suspended {
        shortcuts::suspend(&app);
    } else {
        let stored = shared.settings.lock().unwrap().shortcuts.clone();
        shortcuts::apply(&app, &stored);
    }
}

// ── Settings window ───────────────────────────────────────────────────────────

/// WebView2 allows exactly one browser environment per app, and its options are
/// fixed by whichever webview is created first. Every window must therefore ask
/// for the *same* arguments as the island (see `additionalBrowserArgs` in
/// tauri.conf.json) — a mismatch makes the second window come up blank, with no
/// error anywhere. No spare renderer is kept warm for a window that may never
/// open (SpareRendererForSitePerProcess): one process less (round 3 §9).
pub(crate) const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,SpareRendererForSitePerProcess --autoplay-policy=no-user-gesture-required";

/// A page of the app (`settings.html`, `studio.html`…), with an optional query.
/// In a dev build the pages are served by Vite, so a second window needs the
/// absolute dev URL; a bundled build resolves it inside the app bundle.
pub(crate) fn page_url(app: &AppHandle, page: &str, query: Option<&str>) -> WebviewUrl {
    #[cfg(dev)]
    if let Some(mut base) = app.config().build.dev_url.clone() {
        base.set_path(&format!("/{page}"));
        base.set_query(query);
        return WebviewUrl::External(base);
    }
    let _ = app;
    WebviewUrl::App(match query {
        Some(q) => format!("{page}?{q}").into(),
        None => page.into(),
    })
}

/// Only a known section id goes into the Control Centre's address.
fn hub_query(page: Option<&str>) -> Option<String> {
    let page = page?;
    (!page.is_empty() && page.len() <= 32 && page.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| format!("page={page}"))
}

/// The Control Centre exists only while it is open (round 3 §9: memory): made
/// when asked for, destroyed when closed. Built from its own thread: a WebView2
/// window built inside a synchronous command, on the thread the command blocks,
/// never finishes loading (it came up blank); from another thread Tauri hands
/// the work to the event loop. It shares the island's browser arguments, so
/// all windows share one WebView2 browser (see BROWSER_ARGS).
fn create_hub_window(app: &AppHandle, page: Option<&str>) {
    static OPENING: AtomicBool = AtomicBool::new(false);
    if OPENING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let url = page_url(&app, "settings.html", hub_query(page).as_deref());
    let _ = std::thread::Builder::new().name("groovy-hub-window".into()).spawn(move || {
        let built = WebviewWindowBuilder::new(&app, HUB_LABEL, url)
            .additional_browser_args(BROWSER_ARGS)
            .title(i18n::t("Settings — Groovy"))
            .inner_size(920.0, 740.0)
            .min_inner_size(460.0, 480.0)
            .resizable(true)
            .center()
            .focused(true)
            // Shown where the island is, then grown to its place (grow_out_of_island).
            .visible(false)
            .build();
        match built {
            Ok(win) => {
                log::line("control centre opened");
                grow_out_of_island(&app, &win);
                let closing = win.clone();
                let app2 = app.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Folds back into the island, then goes (round 3B item 8).
                        if CLOSING.swap(true, Ordering::SeqCst) {
                            return;
                        }
                        api.prevent_close();
                        let win = closing.clone();
                        let app = app2.clone();
                        let _ = std::thread::Builder::new().name("groovy-hub-close".into()).spawn(move || {
                            let _ = win.emit("hub-closing", ());
                            if let (Some(from), Some(to)) = (window_rect(&win), island_spot(&app)) {
                                animate_window(&win, from, to, 200);
                            }
                            let _ = win.destroy();
                            CLOSING.store(false, Ordering::SeqCst);
                        });
                    }
                });
            }
            Err(err) => log::line(format!("settings window failed: {err}")),
        }
        OPENING.store(false, Ordering::SeqCst);
    });
}

static CLOSING: AtomicBool = AtomicBool::new(false);

/// A window's outer rectangle, physical pixels.
fn window_rect(win: &tauri::WebviewWindow) -> Option<(f64, f64, f64, f64)> {
    let p = win.outer_position().ok()?;
    let s = win.outer_size().ok()?;
    Some((p.x as f64, p.y as f64, s.width as f64, s.height as f64))
}

/// A small rectangle where the island is, physical pixels: where a window
/// grows out of and folds back into.
fn island_spot(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let (x, y, w, _) = window_rect(&island::window(app)?)?;
    let (sw, sh) = (w.min(360.0).max(200.0), 160.0);
    Some((x + (w - sw) / 2.0, y, sw, sh))
}

/// Windows' Settings → Accessibility → "Animation effects" is on.
#[cfg(windows)]
fn animation_effects_on() -> bool {
    use windows::core::BOOL;
    use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};
    let mut on = BOOL(1);
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut BOOL as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    ok.is_err() || on.as_bool()
}
#[cfg(not(windows))]
fn animation_effects_on() -> bool {
    true
}

/// Moves and sizes a window from one rectangle to another over `ms`, eased
/// out (the shared motion: src/core/motion.ts). With Windows' animation
/// effects off it goes straight there; the page only fades.
fn animate_window(win: &tauri::WebviewWindow, from: (f64, f64, f64, f64), to: (f64, f64, f64, f64), ms: u64) {
    const STEP: u64 = 16;
    let ms = if animation_effects_on() { ms } else { 0 };
    let steps = (ms / STEP).max(1);
    for i in 1..=steps {
        let k = 1.0 - (1.0 - i as f64 / steps as f64).powi(3);
        let lerp = |a: f64, b: f64| a + (b - a) * k;
        platform::set_bounds(
            win,
            lerp(from.0, to.0).round() as i32,
            lerp(from.1, to.1).round() as i32,
            lerp(from.2, to.2).round().max(1.0) as u32,
            lerp(from.3, to.3).round().max(1.0) as u32,
        );
        std::thread::sleep(std::time::Duration::from_millis(STEP));
    }
}

/// The Control Centre opens out of the island: it starts small where the
/// island is and grows to its place while its page fades in (round 3B item 8).
fn grow_out_of_island(app: &AppHandle, win: &tauri::WebviewWindow) {
    let Some(to) = window_rect(win) else {
        let _ = win.show();
        return;
    };
    let Some(from) = island_spot(app) else {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    };
    platform::set_bounds(win, from.0 as i32, from.1 as i32, from.2 as u32, from.3 as u32);
    let _ = win.show();
    let _ = win.set_focus();
    animate_window(win, from, to, 300);
}

/// The Control Centre window (every setting, Flow dictation, the Notetaker…),
/// labelled "settings" since it grew out of the settings window. The ported
/// Coucou 0.5.0 code calls it the hub.
pub const HUB_LABEL: &str = "settings";

pub fn show_settings_window(app: &AppHandle) {
    show_hub(app, None);
}

/// Shows the Control Centre, on `page` when given ("dictation", "notes",
/// "assistant"…: the id of one of its sections).
pub fn show_hub(app: &AppHandle, page: Option<&str>) {
    let Some(win) = app.get_webview_window(HUB_LABEL) else {
        create_hub_window(app, page);
        return;
    };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
    if let Some(page) = page {
        let _ = win.emit("hub-page", page.to_string());
    }
}

/// The argument the login item starts Groovy with.
const AUTOSTART_ARG: &str = "--autostart";

/// Groovy started with the computer, not opened by the user: the window stays hidden.
pub fn launched_at_login() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}

/// The panel is laid out in CSS px against the window's own scale. If the webview
/// reports a different devicePixelRatio (it ends up bigger than the window and gets
/// cut), correct its zoom so one CSS px is one logical px again.
#[tauri::command]
fn fit_zoom(app: AppHandle, dpr: f64) -> bool {
    let Some(win) = island::window(&app) else { return false };
    let scale = win.scale_factor().unwrap_or(1.0);
    static ZOOM: std::sync::Mutex<f64> = std::sync::Mutex::new(1.0);
    let mut zoom = ZOOM.lock().unwrap();
    if let Some(new_zoom) = island::calculate_zoom_correction(*zoom, scale, dpr) {
        *zoom = new_zoom;
        log::line(format!("webview dpr {dpr} != window scale {scale} — setting zoom {new_zoom:.3}"));
        let _ = win.set_zoom(new_zoom);
        true
    } else {
        false
    }
}

/// Groovy's right-click menu on the island and on the desktop: the Control
/// Centre, settings, the wardrobe and the Coding Studio, a right-click away.
#[tauri::command]
fn context_menu(window: tauri::WebviewWindow) -> Result<(), String> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    let app = window.app_handle();
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, i18n::t(label), true, None::<&str>);
    let open = item("ctx-open", i18n::n_("Open Groovy")).map_err(|e| e.to_string())?;
    let settings = item("ctx-settings", i18n::n_("Settings…")).map_err(|e| e.to_string())?;
    let wardrobe = item("ctx-wardrobe", i18n::n_("Wardrobe…")).map_err(|e| e.to_string())?;
    let studio = item("ctx-studio", i18n::n_("Coding Studio")).map_err(|e| e.to_string())?;
    let sep = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&open, &settings, &sep, &wardrobe, &studio]).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// What the right-click menu's items do (menu events reach the app, not the window).
fn on_context_menu(app: &AppHandle, id: &str) {
    match id {
        "ctx-open" => show_hub(app, None),
        "ctx-settings" => show_hub(app, Some("general")),
        "ctx-wardrobe" => {
            let _ = app.emit_to(island::WINDOW_LABEL, "open-wardrobe", ());
        }
        "ctx-studio" => studio::show(app),
        _ => {}
    }
}

/// A user action failed in a window too small to show it (the island, Groovy
/// on the desktop): the island shows it as a notice for a few seconds.
#[tauri::command]
fn notify_error(app: AppHandle, text: String) {
    let text: String = text.chars().take(200).collect();
    log::line(format!("shown error: {text}"));
    flow::live::announce(&app, "error", "#F05252", text, None, 8);
}

#[tauri::command]
fn open_settings_window(app: AppHandle, page: Option<String>) {
    show_hub(&app, page.as_deref());
}

pub fn run() {
    platform::prepare_environment();
    log::install_crash_handler();
    let first_run = settings::first_run();
    let mut loaded = settings::load();
    loaded.assistant = loaded.assistant.clone().sanitised();
    // A Claude chat with no key: the first cloud provider that has one, else
    // the local model only if local AI was chosen (otherwise the chat says what
    // to set up instead of quietly loading a model).
    // Ollama was the default chat before round 2: without Ollama on the PC
    // it is Groovy's own engine (one local engine, never both).
    if loaded.chat_provider == "ollama" && !ollama::installed() {
        loaded.chat_provider = "llamacpp".into();
    }
    if loaded.chat_provider == chat::ANTHROPIC && !secrets::present("anthropic-api-key") {
        if let Some(p) = openai_compat::PROVIDERS.iter().find(|p| secrets::present(p.key)) {
            loaded.chat_provider = p.id.to_string();
        } else if loaded.assistant.local_allowed() {
            loaded.chat_provider = loaded.assistant.local_provider.clone();
        }
    }
    // A local chat with no local AI chosen goes to a cloud key when there is one.
    if loaded.chat_provider == "llamacpp" && !loaded.assistant.local_allowed() {
        if let Some(p) = openai_compat::PROVIDERS.iter().find(|p| secrets::present(p.key)) {
            loaded.chat_provider = p.id.to_string();
        }
    }
    // On a small PC the light profile is the default (llama-server's context and
    // threads), unless the user picked another one.
    if first_run {
        loaded.assistant.performance_profile = models::recommended_profile(models::memory_info().total).to_string();
    }
    // Round 3B item 7: My island's presets and mail apps used to switch every
    // integration off. Once, any service whose key is still stored is back on.
    if !first_run && !loaded.integrations_restored {
        let back = settings::restore_integrations(&mut loaded, secrets::present);
        if !back.is_empty() {
            log::line(format!("integrations turned back on: {}", back.join(", ")));
        }
        if let Err(e) = settings::save(&loaded) {
            log::line(format!("could not save settings: {e}"));
        }
    }
    // Round 4 item 3: talking over Groovy stops him, by default. Installs that
    // had the old default (off) get it once.
    if !first_run && !loaded.barge_in_default {
        loaded.voice_interrupt = true;
        loaded.barge_in_default = true;
        if let Err(e) = settings::save(&loaded) {
            log::line(format!("could not save settings: {e}"));
        }
    }
    if let Err(e) = db::init() {
        log::line(format!("local database unavailable: {e}"));
    }
    // First start: the defaults are written once, so they are there to edit
    // and back up, and every later start reads the same values.
    if first_run {
        match settings::save(&loaded) {
            Ok(()) => log::line("first run: settings created"),
            Err(e) => log::line(format!("first run: could not save settings: {e}")),
        }
    }
    i18n::set_picked(&loaded.language);
    idle::set(&loaded.unload_after);
    browser::configure(&loaded.browser, &loaded.search_engine);
    let gate = Arc::new(PollGate::new());

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // `groovy --shortcut <action>`: what a desktop's own keyboard
            // settings run where we can't listen for keys ourselves (Wayland).
            match shortcuts::from_args(&argv) {
                Some(action) => shortcuts::dispatch(app, action),
                None => {
                    let _ = app.emit_to(island::WINDOW_LABEL, "tray", "open".to_string());
                }
            }
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_ARG])))
        .plugin(tauri_plugin_dialog::init());
    // Where no global shortcut can work, the plugin isn't even started.
    if platform::global_shortcuts_blocked().is_none() {
        builder = builder.plugin(shortcuts::plugin());
    }

    builder
        .manage(Shared {
            settings: Mutex::new(loaded.clone()),
            gate: gate.clone(),
        })
        .manage(Pending::default())
        .manage(Chat::default())
        .manage(shortcuts::Registry::default())
        .manage(recap::load())
        .invoke_handler(tauri::generate_handler![
            boot,
            save_settings,
            set_system_languages,
            set_collapsed,
            set_island_rect,
            fit_island,
            save_growth,
            glass::island_glass,
            focus_window,
            reposition,
            list_monitors,
            open_url,
            open_in_vscode,
            open_session,
            open_claude_desktop,
            open_file_in_vscode,
            quit_app,
            hooks_status,
            agent_hooks_status,
            hooks_preview,
            hooks_apply,
            agent_hooks_list,
            agent_hooks_preview,
            agent_hooks_apply,
            status_line_preview,
            status_line_apply,
            codex_plan_usage,
            approval_decision,
            approval_answer,
            approval_ack,
            approval_decline,
            log_line,
            chat_send,
            chat_models,
            local_connect,
            local_set_key,
            chat_reset,
            ollama_status,
            ollama_setup,
            island_move,
            island_track,
            voice_transcribe,
            voice_status,
            voice_download_model,
            voice_delete_model,
            livekit_token,
            ingest_file,
            secret_present,
            secret_set,
            secret_clear,
            refresh_integration,
            github_refresh,
            open_n8n,
            open_settings_window,
            set_paused,
            shortcuts_status,
            shortcuts_suspend,
            phone_link::phone_link_status,
            phone_link::phone_link_set_enabled,
            phone_link::phone_link_pairing,
            phone_link::phone_link_new_pairing,
            phone_link::phone_link_publish,
            phone_link::phone_link_publish_question,
            phone_link::phone_link_publish_prefs,
            phone_link::phone_answers_status,
            phone_link::phone_answers_set_enabled,
            phone_link::phone_diffs_status,
            phone_link::phone_diffs_set_enabled,
            phone_link::phone_link_send_diff,
            phone_link::phone_usage_status,
            phone_link::phone_usage_set_enabled,
            phone_link::phone_relay_status,
            phone_link::phone_relay_set,
            phone_link::phone_relay_set_access,
            phone_link::phone_link_publish_usage,
            phone_link::phone_services_status,
            phone_link::phone_services_set,
            phone_link::phone_link_publish_services,
            phone_link::phone_details_status,
            phone_link::phone_details_set_enabled,
            phone_link::phone_chat_status,
            phone_link::phone_chat_set_enabled,
            phone_link::phone_chat_set_models,
            recap::recap_history,
            recap::recap_prefs,
            recap::recap_set_enabled,
            recap::recap_set_hide_projects,
            recap::recap_mark_shown,
            recap::recap_clear,
            recap::recap_save_png,
            recap::recap_reveal_saved,
            desktop::desktop_mochi_info,
            desktop::desktop_mochi_pick_up,
            desktop::desktop_mochi_carry,
            desktop::desktop_mochi_carry_end,
            desktop::desktop_mochi_drag_begin,
            desktop::desktop_mochi_drag_move,
            desktop::desktop_mochi_drag_end,
            desktop::desktop_mochi_fly_out,
            desktop::desktop_mochi_fly_home,
            desktop::desktop_mochi_set_asleep,
            sounds::custom_sounds,
            sounds::custom_sound,
            sounds::reveal_sounds_folder,
            sounds::reload_sounds,
            spotify::spotify_refresh,
            spotify::spotify_control,
            spotify::spotify_open,
            spotify::spotify_installed,
            memory::memory_list,
            memory::memory_add,
            memory::memory_update,
            memory::memory_delete,
            memory::memory_clear,
            memory::memory_export,
            memory::wardrobe_export,
            memory::history_list,
            memory::history_messages,
            memory::history_delete,
            memory::history_clear,
            tasks::task_list,
            tasks::task_add,
            tasks::task_set_done,
            tasks::task_delete,
            tasks::task_clear_done,
            models::models_status,
            models::memory_hogs,
            models::models_load_anyway,
            voice_probe,
            voice_log,
            models::models_install_runtime,
            models::models_download,
            models::models_cancel_download,
            models::models_delete,
            models::models_start,
            models::models_stop,
            models::models_reveal_folder,
            system::system_status,
            music::music_add_folder,
            music::music_rescan,
            music::music_folders,
            music::music_remove_folder,
            music::music_search,
            music::music_set_favourite,
            music::music_art,
            music::music_playlists,
            music::music_playlist_create,
            music::music_playlist_rename,
            music::music_playlist_delete,
            music::music_playlist_tracks,
            music::music_playlist_add,
            music::music_playlist_remove,
            music::music_state,
            music::music_play,
            music::music_control,
            music::music_seek,
            music::music_volume,
            music::music_set_mode,
            db::usage_today,
            fit_zoom,
            notify_error,
            context_menu,
            island_tiles::island_apps,
            island_tiles::island_launch,
            island_tiles::island_pick_folder,
            island_tiles::mail_access,
            island_tiles::mail_open,
            speech::check::speech_check_start,
            speech::check::speech_check_reset,
            speech::check::speech_check_cancel,
            speech::check::speech_check_stop,
            brain_ai::ai_clis,
            flow::flow_status,
            flow::flow_set_prefs,
            flow::flow_history,
            flow::flow_delete,
            flow::flow_clear,
            flow::flow_stats,
            flow::flow_words,
            flow::flow_set_words,
            flow::speech_cloud_services,
            flow::flow_snippets,
            flow::flow_set_snippets,
            flow::flow_styles,
            flow::flow_set_styles,
            flow::flow_running_apps,
            flow::flow_copy,
            flow::flow_paste_again,
            flow::flow_models_download,
            flow::flow_models_remove,
            flow::live_state,
            flow::live_notices,
            flow::live_action,
            notes::notes_state,
            notes::notes_start,
            notes::notes_stop,
            notes::notes_rename,
            notes::notes_rename_speaker,
            notes::notes_toggle_todo,
            notes::notes_delete,
            notes::notes_summarize,
            notes::notes_retry,
            notes::notes_markdown,
            notes::notes_export,
            okgroovy::voice_done,
            okgroovy::okgroovy_status,
            quick_status,
            browser::browsers,
            calls::voice_call_active,
            calls::voice_basic,
            calls::call_summary,
            calls::instant_voice_options,
            calls::instant_openai_answer,
            calls::instant_gemini_token,
            frame_report,
            speech::tts::tts_audio,
            speech::tts::tts_status,
            speech::tts::fish_test,
            speech::tts::kokoro_download,
            speech::tts::kokoro_remove,
            okgroovy::okgroovy_test,
            skills::manage::skills_status,
            skills::manage::skills_take_confirm,
            skills::manage::skills_install_node,
            skills::manage::skills_projects,
            skills::manage::skills_pick_project,
            skills::manage::skills_list,
            skills::manage::skills_search,
            skills::manage::skills_preview,
            skills::manage::skills_install,
            skills::manage::skills_remove,
            skills::manage::skills_update,
            skills::manage::skills_undo,
            skills::manage::skills_init,
            skills::manage::skills_fork,
            skills::manage::skills_read,
            skills::manage::skills_save,
            skills::manage::skills_analyze,
            skills::manage::skills_recommendations,
            skills::manage::skills_dismiss,
            skills::manage::skills_session_started,
            okgroovy::voice_say_native,
            studio::studio_workspaces,
            studio::studio_add_workspace,
            studio::studio_new_project,
            studio::studio_remove_workspace,
            studio::studio_list,
            studio::studio_read,
            studio::studio_write,
            studio::studio_delete,
            studio::studio_history,
            studio::studio_revert,
            studio::studio_search,
            studio::studio_run,
            studio::studio_stop,
            studio::studio_git,
            studio::studio_git_action,
            studio::studio_ai_target,
            studio::studio_ask,
            studio::open_studio_window,
            tools::tools_list,
            tools::tools_set_permission,
            tools::tools_decide,
            tools::tools_log,
            tools::tools_log_clear,
        ])
        .on_menu_event(|app, event| on_context_menu(app, event.id().as_ref()))
        .setup(move |app| {
            let handle = app.handle().clone();
            app.manage(phone_link::PhoneLink::new(handle.clone()));
            tray::build(&handle)?;
            // Groovy's desktop window, when he is out on the desktop. The
            // Control Centre and the Studio are made only when opened.
            desktop::setup(&handle);

            if let Some(win) = island::window(&handle) {
                // Where Tauri takes the drop itself (Linux), its paths are the
                // ones ingest_file may copy (files.rs).
                win.on_window_event(|event| {
                    if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                        files::allow_dropped(paths.iter().map(|p| p.to_string_lossy().to_string()));
                    }
                });
                platform::make_non_activating(&win);
                #[cfg(windows)]
                webview_drop::install(&handle);
                island::apply_geometry(&handle, &loaded.screen, false);
                let _ = win.show();
            }
            gate.collapsed.store(false, Ordering::Relaxed);
            // Nothing drawn yet, so nothing takes the mouse until the page
            // reports the island's shape.
            if !platform::CURSOR_POLL {
                island::refresh_click_through(&handle, &gate);
            }
            gate.set_active(true);
            island::spawn_cursor_poll(handle.clone(), gate.clone());

            log::line(format!("--- Groovy {} started ---", env!("CARGO_PKG_VERSION")));
            if let Ok(conn) = db::conn() {
                match memory::prune(&conn, loaded.assistant.history_retention_days, db::now()) {
                    Ok(n) if n > 0 => log::line(format!("chat history: {n} old conversation(s) removed")),
                    Err(e) => log::line(format!("chat history: {e}")),
                    _ => {}
                }
            }
            tasks::spawn(handle.clone());
            brain_ai::init(&handle);
            island_tiles::spawn_mail_watch(handle.clone());
            flow::setup(&handle);
            notes::setup(&handle);
            if loaded.autostart {
                // Login items written before this version start Groovy without
                // --autostart: writing it again adds it, so the window stays hidden at login.
                let _ = handle.autolaunch().enable();
            }
            let open_window = flow::get(&handle).map(|f| f.store.lock().unwrap().prefs.open_at_launch).unwrap_or(false);
            // No AI chosen yet: the first start opens the choice; later starts
            // remind once in the pill. Nothing loads a model meanwhile.
            // Asked in the island, after the intro, with a button to the choice:
            // the Control Centre is a whole second webview, opened only when
            // the user wants it (round 3 §9, under 300 MB at idle).
            if loaded.assistant.ai_setup.is_empty() && !launched_at_login() {
                let h = handle.clone();
                let wait = if first_run { 5 } else { 4 };
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    flow::live::announce(&h, "bolt", "#7C5CFF", i18n::t("Choose Groovy's AI to start chatting"), Some((i18n::t("Choose"), "openAssistant")), 30);
                });
            } else if open_window && !launched_at_login() && !first_run {
                show_hub(&handle, None);
            }
            // Round 2: the island closes when the pointer leaves by default.
            // Someone who had another delay is offered it once.
            if !first_run && !loaded.close_on_leave_offered && loaded.auto_close_interval > 0.0 {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                    flow::live::announce(&h, "bolt", "#7C5CFF", i18n::t("The island can now close as soon as the mouse leaves it."), Some((i18n::t("Use it"), "closeOnLeave")), 20);
                });
            }
            if !loaded.close_on_leave_offered {
                set_setting(&handle, |s| s.close_on_leave_offered = true);
            }
            music::init(&handle);
            models::spawn_idle_reaper();
            speech::spawn_idle_reaper();
            worktest::start(&handle);
            hooks::ensure_hook_exe(&handle);
            pipe::start(handle.clone());
            phone_link::start_if_enabled(&handle);
            integrations::start(handle.clone());
            spotify::sync(&handle, &loaded.active_integrations);
            shortcuts::apply(&handle, &loaded.shortcuts);
            glass::spawn_watch(handle.clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Groovy")
        .run(|app, event| {
            // Withdraw the phone link's announcement on the local network before the process ends.
            if let tauri::RunEvent::Exit = event {
                phone_link::stop_on_exit(app);
                // The local model server is ours: it never outlives Groovy.
                models::stop();
                music::stop_on_exit();
                notes::shutdown(app);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::diff_file;

    #[test]
    fn every_window_asks_webview2_for_the_islands_arguments() {
        // A window with other arguments comes up blank (see BROWSER_ARGS).
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["windows"][0]["additionalBrowserArgs"], super::BROWSER_ARGS);
        for (file, src) in [("studio.rs", include_str!("studio.rs")), ("desktop.rs", include_str!("desktop.rs")), ("lib.rs", include_str!("lib.rs"))] {
            let built = src.matches("WebviewWindowBuilder::new(").count();
            let with_args = src.matches(".additional_browser_args(").count();
            assert_eq!(built, with_args, "{file}: a window without the shared arguments");
        }
    }

    #[test]
    fn the_smoke_test_can_only_ask_for_the_two_layouts() {
        assert_eq!(super::smoke_view(Some("home")), Some("home"));
        assert_eq!(super::smoke_view(Some("player")), Some("player"));
        assert_eq!(super::smoke_view(Some("frames")), Some("frames"));
        for other in [None, Some(""), Some("settings"), Some("HOME")] {
            assert_eq!(super::smoke_view(other), None);
        }
    }

    #[test]
    fn the_control_centre_opens_only_on_a_plain_page_name() {
        assert_eq!(super::hub_query(Some("skills")).as_deref(), Some("page=skills"));
        assert_eq!(super::hub_query(Some("flow-settings")).as_deref(), Some("page=flow-settings"));
        assert_eq!(super::hub_query(None), None);
        for bad in ["", "a b", "x&y=1", "../x", "é", &"a".repeat(40)] {
            assert_eq!(super::hub_query(Some(bad)), None, "{bad}");
        }
    }

    #[test]
    fn only_an_existing_file_by_its_full_path_reaches_the_editor() {
        let dir = std::env::temp_dir().join(format!("groovy-diff-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("edited.ts");
        std::fs::write(&file, "x").unwrap();

        assert!(diff_file(&file.to_string_lossy()).is_some());
        // A folder, a missing file, a relative path or an option never pass.
        assert!(diff_file(&dir.to_string_lossy()).is_none());
        assert!(diff_file(&dir.join("missing.ts").to_string_lossy()).is_none());
        assert!(diff_file("edited.ts").is_none());
        assert!(diff_file("--help").is_none());
        assert!(diff_file("").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
