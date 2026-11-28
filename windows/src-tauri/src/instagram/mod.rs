// Round 7 (FIXES_ROUND7.md): Instagram messages by voice, through what the
// user already has. New DMs come from Windows' notifications (the mail
// listener in island_tiles.rs hands them here); "message Sam on Instagram" and
// "reply" open the conversation (or the search) in the Instagram app or
// instagram.com and leave the draft on the clipboard, or type it when the user
// allowed that. No private API, no token, no password: the user presses Send.
//
// Its own module on purpose: round 6 (Discord) is built beside it, and the two
// can share code only after both merge.

pub mod command;
pub mod notes;
pub mod open;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::i18n::{t, tf};
use command::Command;
use notes::Dm;
use open::Place;

/// Settings → General → Instagram (settings.json's `instagram`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// Off by default: nothing is read or opened until the user turns it on.
    pub on: bool,
    /// "auto" (the Instagram app when Windows has shown one of its
    /// notifications, else the browser), "app", "edge", "chrome" or "browser".
    pub open_in: String,
    /// Types the name and the draft like a keyboard, only into Instagram's own
    /// window. Off: the draft goes on the clipboard. Never presses Send.
    pub type_for_me: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { on: false, open_in: "auto".into(), type_for_me: false }
    }
}

/// "reply" means the newest DM for this long after it came.
pub const REPLY_WINDOW: Duration = Duration::from_secs(15 * 60);
/// The most people remembered (name → username), in memory only.
const PEOPLE: usize = 50;

struct Recent {
    last: Option<(Dm, Instant)>,
    /// Who wrote lately: (their name in lower case, their username).
    people: Vec<(String, String)>,
    /// The Instagram app's AppUserModelID, once one of its notifications was seen.
    app_id: Option<String>,
}

static RECENT: Mutex<Recent> = Mutex::new(Recent { last: None, people: Vec::new(), app_id: None });

/// The island has an Instagram tile.
pub fn has_tile(tiles: &Value) -> bool {
    tiles["tiles"].as_array().is_some_and(|list| list.iter().any(|t| t["kind"] == "instagram"))
}

/// Notifications are read for Instagram only while it is on and has a tile.
pub fn wanted(settings: &crate::settings::Settings) -> bool {
    settings.instagram.on && has_tile(&settings.island_tiles)
}

/// Forgets every DM and name (Instagram turned off).
pub fn forget() {
    let mut r = RECENT.lock().unwrap();
    r.last = None;
    r.people.clear();
    r.app_id = None;
}

fn remember(dm: &Dm) {
    let mut r = RECENT.lock().unwrap();
    if let Some(u) = &dm.username {
        // "Sam Kay (sam.k)": the name is the part before the username.
        let name = dm.sender.rsplit_once(" (").map_or(dm.sender.as_str(), |(n, _)| n).to_lowercase();
        r.people.retain(|(n, x)| n != &name && x != u);
        r.people.insert(0, (name, u.clone()));
        r.people.truncate(PEOPLE);
    }
    if dm.source == "app" && open::app_id_ok(&dm.app_id) {
        r.app_id = Some(dm.app_id.clone());
    }
    r.last = Some((dm.clone(), Instant::now()));
}

/// A new notification from the mail listener: None when it isn't Instagram's
/// (it stays mail), Some(None) when it is Instagram's but no DM (a like, a
/// follow: dropped), Some(Some(dm)) for a DM, which is remembered.
pub fn take(id: u32, app: &str, app_id: &str, title: &str, body: &str) -> Option<Option<Dm>> {
    if !notes::is_instagram(app, app_id, title, body) {
        return None;
    }
    let dm = notes::from_notification(id, app, app_id, title, body);
    if let Some(d) = &dm {
        remember(d);
    }
    Some(dm)
}

/// Who sent the newest DM, while "reply" still means them.
pub fn last_sender() -> Option<String> {
    let r = RECENT.lock().unwrap();
    r.last.as_ref().filter(|(_, at)| at.elapsed() < REPLY_WINDOW).map(|(d, _)| d.username.clone().unwrap_or_else(|| d.sender.clone()))
}

/// Where a name said aloud goes: straight to the chat for someone who wrote
/// lately (or a name that can only be a username, like "sam.k"), else the search.
fn place_for(name: &str, people: &[(String, String)]) -> Place {
    let lower = name.trim().trim_start_matches('@').to_lowercase();
    if let Some((_, u)) = people.iter().find(|(n, u)| *n == lower || *u == lower) {
        return Place::Chat(u.clone());
    }
    match notes::username(&lower) {
        Some(u) if u.contains(['.', '_']) || u.chars().any(|c| c.is_ascii_digit()) => Place::Chat(u),
        _ => Place::NewMessage,
    }
}

fn prefs(app: &AppHandle) -> Prefs {
    app.try_state::<crate::Shared>().map(|s| s.settings.lock().unwrap().instagram.clone()).unwrap_or_default()
}

fn app_id() -> Option<String> {
    RECENT.lock().unwrap().app_id.clone()
}

fn to_clipboard(text: &str) -> bool {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).is_ok()
}

/// Opens the place and leaves the draft: on the clipboard, or typed into
/// Instagram's window when the user allowed typing. What Groovy says.
fn deliver(prefs: &Prefs, place: &Place, who: &str, text: &str) -> String {
    let id = app_id();
    if let Err(e) = open::open(&prefs.open_in, id.as_deref(), place) {
        return tf("I couldn't open Instagram: {error}", &[("error", &e)]);
    }
    let search = *place == Place::NewMessage;
    let mut said = if search {
        tf("I've opened Instagram's new message search: pick {name} there.", &[("name", who)])
    } else {
        tf("I've opened your chat with {name} on Instagram.", &[("name", who)])
    };
    if prefs.type_for_me {
        // Into the search box, the name; into an open chat, the message. Never Enter.
        let typed = if search { who.to_string() } else { text.to_string() };
        if !typed.is_empty() {
            std::thread::spawn(move || {
                if !open::type_when_ready(&typed, Duration::from_secs(8)) {
                    crate::log::line("instagram: Instagram didn't come to the front; nothing typed");
                }
            });
        }
    }
    if !text.is_empty() {
        if prefs.type_for_me && !search {
            said.push(' ');
            said.push_str(&t("I'm typing your message: check it, then press Send."));
        } else if to_clipboard(text) {
            said.push(' ');
            said.push_str(&t("Your message is on the clipboard: paste it with Ctrl+V, then press Send."));
        }
    }
    said
}

/// Does a command; what Groovy says back. Returns at once: opening, typing
/// and the AI's writing (only when asked) happen on their own threads.
pub fn run(app: &AppHandle, cmd: &Command) -> String {
    let prefs = prefs(app);
    if !prefs.on {
        return t("Instagram is off. Turn it on in Settings → General → Instagram.");
    }
    let (who, text, write) = match cmd {
        Command::Inbox => {
            return match open::open(&prefs.open_in, app_id().as_deref(), &Place::Inbox) {
                Ok(_) => t("Opening your Instagram messages."),
                Err(e) => tf("I couldn't open Instagram: {error}", &[("error", &e)]),
            };
        }
        Command::Reply { text, write } => match last_sender() {
            Some(who) => (who, text.clone(), *write),
            None => return t("There's no new Instagram message to reply to."),
        },
        Command::Message { to, text, write } => (to.clone(), text.clone(), *write),
    };
    let place = place_for(&who, &RECENT.lock().unwrap().people);
    if write && !text.is_empty() && crate::brain_ai::available() {
        let app = app.clone();
        let name = who.clone();
        tauri::async_runtime::spawn(async move {
            let system = "You write one short, friendly Instagram direct message for the user. Reply with the message only: no quotes, no greeting line unless asked, in the language the user wrote in.";
            let ask = format!("To {name}. What I want to say: {text}");
            let draft = match crate::brain_ai::complete(system, &ask, 200, Duration::from_secs(30)).await {
                Ok(d) if !d.trim().is_empty() => crate::brain_ai::strip_thinking(&d).trim().trim_matches('"').to_string(),
                _ => text.clone(),
            };
            let prefs = self::prefs(&app);
            let said = tauri::async_runtime::spawn_blocking(move || deliver(&prefs, &place, &name, &draft)).await.unwrap_or_default();
            crate::flow::live::announce(&app, "bolt", "#E1306C", said, None, 12);
        });
        return tf("Writing your message to {name}…", &[("name", &who)]);
    }
    deliver(&prefs, &place, &who, &text)
}

/// The tile: opens the chat with someone who wrote (or the DMs list when `who` is empty).
#[tauri::command]
pub async fn instagram_open(app: AppHandle, who: String) -> Result<String, String> {
    let prefs = prefs(&app);
    if !prefs.on {
        return Err(t("Instagram is off. Turn it on in Settings → General → Instagram."));
    }
    let who = who.chars().take(60).collect::<String>();
    tauri::async_runtime::spawn_blocking(move || {
        let place = if who.trim().is_empty() { Place::Inbox } else { place_for(&who, &RECENT.lock().unwrap().people) };
        open::open(&prefs.open_in, app_id().as_deref(), &place)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watch_is_wanted_only_when_on_with_a_tile() {
        let mut s = crate::settings::Settings::default();
        assert!(!s.instagram.on, "off by default");
        assert!(!wanted(&s));
        s.island_tiles = serde_json::json!({ "tiles": [{ "kind": "instagram" }] });
        assert!(!wanted(&s), "a tile alone reads nothing");
        s.instagram.on = true;
        assert!(wanted(&s));
        s.island_tiles = serde_json::json!({ "tiles": [{ "kind": "mail" }] });
        assert!(!wanted(&s), "on, but no tile");
    }

    #[test]
    fn an_old_settings_file_has_instagram_off() {
        let s: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.instagram, Prefs::default());
        let s: crate::settings::Settings = serde_json::from_str(r#"{"instagram":{"on":true}}"#).unwrap();
        assert_eq!((s.instagram.on, s.instagram.open_in.as_str(), s.instagram.type_for_me), (true, "auto", false));
    }

    #[test]
    fn names_go_to_the_chat_only_when_known() {
        let people = vec![("sam kay".to_string(), "sam.k".to_string()), ("mia_r".to_string(), "mia_r".to_string())];
        assert_eq!(place_for("Sam Kay", &people), Place::Chat("sam.k".into()));
        assert_eq!(place_for("sam.k", &people), Place::Chat("sam.k".into()));
        assert_eq!(place_for("@Mia_R", &people), Place::Chat("mia_r".into()));
        assert_eq!(place_for("jo.b", &people), Place::Chat("jo.b".into()), "only a username looks like that");
        assert_eq!(place_for("Sam", &people), Place::NewMessage, "a plain first name could be anyone: the search");
        assert_eq!(place_for("Uncle Bob", &people), Place::NewMessage);
    }

    #[test]
    fn a_fake_dm_is_remembered_for_reply_and_other_notifications_are_left_alone() {
        forget();
        assert_eq!(take(1, "Outlook", "Microsoft.Office.OUTLOOK.EXE.15", "Sam", "Lunch?"), None, "mail stays mail");
        assert_eq!(take(2, "Instagram", "Facebook.InstagramBeta_8x!App", "Instagram", "jo liked your photo."), Some(None));
        assert_eq!(last_sender(), None);
        let dm = take(3, "Instagram", "Facebook.InstagramBeta_8x!App", "Sam Kay (sam.k)", "you up?").flatten().unwrap();
        assert_eq!((dm.sender.as_str(), dm.preview.as_str()), ("Sam Kay (sam.k)", "you up?"));
        assert_eq!(last_sender().as_deref(), Some("sam.k"));
        assert_eq!(app_id().as_deref(), Some("Facebook.InstagramBeta_8x!App"));
        // A browser's notification never becomes "the app".
        take(4, "Google Chrome", "Chrome", "Instagram", "mia_r: hi · www.instagram.com");
        assert_eq!(app_id().as_deref(), Some("Facebook.InstagramBeta_8x!App"));
        assert_eq!(last_sender().as_deref(), Some("mia_r"));
        forget();
        assert_eq!(last_sender(), None);
    }
}
