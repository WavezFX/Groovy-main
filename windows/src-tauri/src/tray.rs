// Notification-area icon: Open, Weekly recap, Wardrobe, Settings, Pause, Quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::i18n::{n_, t};
use crate::island::WINDOW_LABEL;

/// The menu's items, id and English label, in order. The labels are shown in
/// the interface language (i18n.rs) and follow it when it changes.
const ITEMS: [(&str, &str); 7] = [
    ("open", n_("Open Groovy")),
    ("studio", n_("Coding Studio")),
    ("recap", n_("Weekly recap")),
    ("wardrobe", n_("Wardrobe…")),
    ("settings", n_("Settings…")),
    ("pause", n_("Pause")),
    ("quit", n_("Quit")),
];

/// Kept so a language change relabels the menu without rebuilding the tray.
struct Items(Vec<(&'static str, MenuItem<Wry>)>);

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let mut items = Vec::new();
    for (id, label) in ITEMS {
        items.push((label, MenuItem::with_id(app, id, t(label), true, None::<&str>)?));
    }
    let [open, studio, recap, wardrobe, settings, pause, quit] = [0, 1, 2, 3, 4, 5, 6].map(|i| &items[i].1);
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    // "Local model loaded, using X GB: Unload", only while llama-server runs.
    let unload = MenuItem::with_id(app, "unload", t("No local model loaded"), false, None::<&str>)?;
    let menu = Menu::with_items(app, &[&unload, open, studio, recap, &sep1, wardrobe, settings, pause, &sep2, quit])?;
    app.manage(Items(items));
    app.manage(UnloadItem(unload.clone()));
    spawn_local_model_watch(app.clone());

    let mut builder = TrayIconBuilder::with_id("groovy")
        .tooltip("Groovy")
        .menu(&menu)
        .on_menu_event(|app: &AppHandle, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "settings" => crate::show_settings_window(app),
            // The Groovy window: dictation, notes and every setting.
            "open" => crate::show_hub(app, None),
            "unload" => {
                crate::models::stop();
                local_model_changed(app);
            }
            "studio" => crate::studio::show(app),
            id => {
                let _ = app.emit_to(WINDOW_LABEL, "tray", id.to_string());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

struct UnloadItem(MenuItem<Wry>);

/// Keeps the Unload item's label on what the local model takes now.
pub fn local_model_changed(app: &AppHandle) {
    let Some(item) = app.try_state::<UnloadItem>() else { return };
    match crate::models::running_memory() {
        Some(bytes) => {
            let gb = format!("{:.1}", bytes as f64 / 1_073_741_824.0);
            let _ = item.0.set_text(crate::i18n::tf("Local model loaded, using {gb} GB: Unload", &[("gb", &gb)]));
            let _ = item.0.set_enabled(true);
        }
        None => {
            let _ = item.0.set_text(t("No local model loaded"));
            let _ = item.0.set_enabled(false);
        }
    }
}

/// The tray's line about the built-in model, a job on the scheduler: every
/// 15 s while the model runs (its memory changes), poked when it starts or
/// stops, nothing otherwise.
fn spawn_local_model_watch(_app: AppHandle) {
    crate::scheduler::add(LOCAL_MODEL_JOB, std::time::Duration::ZERO, local_model_pace, local_model_changed);
}

pub const LOCAL_MODEL_JOB: &str = "tray-local-model";

fn local_model_pace(_: &AppHandle) -> crate::scheduler::Next {
    match crate::models::running_memory() {
        Some(_) => crate::scheduler::Next::Every(std::time::Duration::from_secs(15)),
        None => crate::scheduler::Next::Idle,
    }
}

/// Relabels the menu in the current language.
pub fn retitle(app: &AppHandle) {
    let Some(items) = app.try_state::<Items>() else { return };
    for (label, item) in &items.0 {
        let _ = item.set_text(t(label));
    }
}

/// The tray's tooltip names him (round 5 item 6): "Groovy", or "Pip · Groovy" once he has his own name.
pub fn set_name(app: &AppHandle, name: &str) {
    let name = name.trim();
    let tip = if name.is_empty() || name.eq_ignore_ascii_case("groovy") { "Groovy".to_string() } else { format!("{name} · Groovy") };
    if let Some(tray) = app.tray_by_id("groovy") {
        let _ = tray.set_tooltip(Some(tip));
    }
}
