// "My island" (FIXES_ROUND1 item 4), the backend side: the apps the user can
// put on the island (the Start menu on Windows, .desktop files on Linux),
// opening what a tile points to, and new-mail pop-ups read from Windows' own
// notification feed — no account, no password: whatever shows a Windows
// notification (Outlook, Mail, Gmail in a browser…) can show on the island.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppEntry {
    pub name: String,
    /// The Start-menu shortcut (Windows) or "desktop:<id>" (Linux).
    pub target: String,
}

#[cfg(windows)]
fn start_menus() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["APPDATA", "ProgramData"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
    }
    roots
}

/// The apps that can go on the island, by name.
#[tauri::command]
pub async fn island_apps() -> Vec<AppEntry> {
    tauri::async_runtime::spawn_blocking(list_apps).await.unwrap_or_default()
}

#[cfg(windows)]
fn list_apps() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut stack = start_menus();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if lower.contains("uninstall") || lower.contains("readme") || lower.contains("help") {
                    continue;
                }
                if !out.iter().any(|a| a.name == name) {
                    out.push(AppEntry { name, target: path.to_string_lossy().into_owned() });
                }
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

#[cfg(not(windows))]
fn list_apps() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut dirs = vec![PathBuf::from("/usr/share/applications"), PathBuf::from("/var/lib/flatpak/exports/share/applications")];
    dirs.push(crate::platform::home_dir().join(".local/share/applications"));
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
            let Some(name) = text.lines().find_map(|l| l.strip_prefix("Name=")).map(str::trim) else { continue };
            let id = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if !out.iter().any(|a| a.name == name) {
                out.push(AppEntry { name: name.to_string(), target: format!("desktop:{id}") });
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Whether `target` is something an app tile may launch: a shortcut or
/// program that exists (Windows), or a .desktop id (Linux).
pub fn launchable(target: &str) -> bool {
    if let Some(id) = target.strip_prefix("desktop:") {
        return !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    }
    let p = Path::new(target);
    p.is_absolute() && p.is_file() && p.extension().is_some_and(|x| ["lnk", "exe", "url", "appref-ms"].iter().any(|e| x.eq_ignore_ascii_case(e)))
}

/// Opens what a tile points to. Only what fits the tile's kind is opened.
#[tauri::command]
pub fn island_launch(kind: String, target: String) -> Result<(), String> {
    match kind.as_str() {
        "site" => {
            if !(target.starts_with("https://") || target.starts_with("http://")) || target.chars().any(char::is_whitespace) {
                return Err("Only web addresses (https://…) open from a website tile.".into());
            }
            crate::platform::open_url(&target);
        }
        "folder" => {
            if !Path::new(&target).is_dir() {
                return Err(crate::i18n::tf("{0} is not a folder any more.", &[("0", &target)]));
            }
            crate::platform::reveal_folder(&target);
        }
        "app" => {
            if !launchable(&target) {
                return Err(crate::i18n::t("This app is not installed any more. Remove the tile, or add it again."));
            }
            if let Some(id) = target.strip_prefix("desktop:") {
                std::process::Command::new("gtk-launch").arg(id).spawn().map_err(|e| e.to_string())?;
            } else {
                // Explorer opens shortcuts and programs the way the Start menu does.
                crate::platform::reveal_folder(&target);
            }
        }
        _ => return Err("Nothing to open.".into()),
    }
    Ok(())
}

/// A folder for a folder tile, picked in the system's dialog.
#[tauri::command]
pub async fn island_pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(crate::i18n::t("Pick a folder")).pick_folder(move |p| {
        let _ = tx.send(p);
    });
    match rx.await.map_err(|e| e.to_string())? {
        Some(p) => Ok(Some(p.into_path().map_err(|e| e.to_string())?.to_string_lossy().into_owned())),
        None => Ok(None),
    }
}

// ── Mail from Windows' notifications ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailNote {
    pub id: u32,
    /// The app that showed it ("Outlook", "Google Chrome"…).
    pub app: String,
    /// Usually the sender.
    pub title: String,
    /// Usually the subject.
    pub body: String,
    /// What opens the app: its AppUserModelID, when Windows gives one.
    pub app_id: String,
}

/// Asks Windows (once; it remembers) to let Groovy read notifications.
/// "allowed", "denied", "unspecified" or "unsupported".
#[tauri::command]
pub async fn mail_access() -> String {
    tauri::async_runtime::spawn_blocking(request_access).await.unwrap_or_else(|_| "unsupported".into())
}

#[cfg(windows)]
fn request_access() -> String {
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    let Ok(listener) = UserNotificationListener::Current() else { return "unsupported".into() };
    match listener.RequestAccessAsync().and_then(|op| op.get()) {
        Ok(s) if s == UserNotificationListenerAccessStatus::Allowed => "allowed".into(),
        Ok(s) if s == UserNotificationListenerAccessStatus::Denied => "denied".into(),
        Ok(_) => "unspecified".into(),
        Err(e) => {
            crate::log::line(format!("notifications: access request failed: {e}"));
            "unsupported".into()
        }
    }
}

#[cfg(not(windows))]
fn request_access() -> String {
    "unsupported".into()
}

#[cfg(windows)]
fn read_notifications() -> Result<Vec<MailNote>, String> {
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};
    let listener = UserNotificationListener::Current().map_err(|e| e.to_string())?;
    if listener.GetAccessStatus().map_err(|e| e.to_string())? != UserNotificationListenerAccessStatus::Allowed {
        return Err("not allowed".into());
    }
    let list = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.get()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for n in list {
        let id = n.Id().unwrap_or(0);
        let (app, app_id) = n
            .AppInfo()
            .map(|a| {
                let name = a.DisplayInfo().and_then(|d| d.DisplayName()).map(|s| s.to_string()).unwrap_or_default();
                let aumid = a.AppUserModelId().map(|s| s.to_string()).unwrap_or_default();
                (name, aumid)
            })
            .unwrap_or_default();
        let texts: Vec<String> = n
            .Notification()
            .and_then(|x| x.Visual())
            .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
            .and_then(|b| b.GetTextElements())
            .map(|els| els.into_iter().filter_map(|t| t.Text().ok()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
        out.push(MailNote {
            id,
            app,
            title: texts.first().cloned().unwrap_or_default(),
            body: texts.get(1..).map(|r| r.join(" · ")).unwrap_or_default(),
            app_id,
        });
    }
    Ok(out)
}

/// Watches the notification feed and hands new ones to the island
/// ("mail-notification"); the island keeps those from mail apps the user chose.
#[cfg(windows)]
pub fn spawn_mail_watch(app: AppHandle) {
    use tauri::Emitter;
    std::thread::Builder::new()
        .name("groovy-notifications".into())
        .spawn(move || {
            let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
            let mut first = true;
            loop {
                if let Ok(list) = read_notifications() {
                    for n in list {
                        if seen.insert(n.id) && !first {
                            let _ = app.emit_to(crate::island::WINDOW_LABEL, "mail-notification", &n);
                        }
                    }
                    if seen.len() > 2000 {
                        seen.clear();
                        first = true;
                        continue;
                    }
                    first = false;
                }
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
        })
        .ok();
}

#[cfg(not(windows))]
pub fn spawn_mail_watch(_app: AppHandle) {}

/// Opens the app a notification came from.
#[tauri::command]
pub fn mail_open(app_id: String) -> Result<(), String> {
    if app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '!' | '-' | '{' | '}')) {
        return Err("Open the mail app from the Start menu.".into());
    }
    #[cfg(windows)]
    {
        crate::platform::reveal_folder(&format!("shell:AppsFolder\\{app_id}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_shortcuts_and_desktop_ids_launch() {
        assert!(launchable("desktop:org.gnome.Nautilus"));
        assert!(!launchable("desktop:../../etc"));
        assert!(!launchable("desktop:"));
        assert!(!launchable("relative/app.exe"));
        assert!(!launchable("/does/not/exist.lnk"));
        let dir = std::env::temp_dir().join(format!("groovy-tiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lnk = dir.join("App.lnk");
        std::fs::write(&lnk, b"x").unwrap();
        let txt = dir.join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(launchable(&lnk.to_string_lossy()));
        assert!(!launchable(&txt.to_string_lossy()), "only shortcuts and programs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn websites_only_open_as_web_addresses() {
        assert!(island_launch("site".into(), "file:///C:/Windows/system32/calc.exe".into()).is_err());
        assert!(island_launch("site".into(), "https://a.b c".into()).is_err());
        assert!(island_launch("folder".into(), "/definitely/not/here".into()).is_err());
        assert!(island_launch("weird".into(), "x".into()).is_err());
        assert!(mail_open("x; rm -rf".into()).is_err());
    }
}
