// Groovy's desktop tools: things he can do on this computer when asked in the
// chat or by voice — open a website, an app or a folder, find files, set a
// reminder, add a to-do, report the system status, read the clipboard.
//
// Every tool has a permission the user sets in Settings → Permissions:
//   allow  runs when asked
//   ask    the answer carries "Allow once / Always / Don't" buttons, and
//          nothing happens until one is clicked (5 minutes, then it lapses)
//   deny   never runs; the answer says it is turned off
// Every run, refusal and approval is written to the audit log (tool_log),
// which the user can read and clear. Nothing here runs a shell command or
// takes a path from the page: apps are found among the Start-menu shortcuts,
// folders among the user's own, websites must be http(s).
//
// Requests are recognised by plain patterns (parse()), not by a model, so a
// tool never runs because a model imagined it was asked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

use crate::i18n::{n_, t, tf};
use crate::{db, log};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Perm {
    Allow,
    Ask,
    Deny,
}

impl Perm {
    fn parse(s: &str) -> Option<Perm> {
        match s {
            "allow" => Some(Perm::Allow),
            "ask" => Some(Perm::Ask),
            "deny" => Some(Perm::Deny),
            _ => None,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Perm::Allow => "allow",
            Perm::Ask => "ask",
            Perm::Deny => "deny",
        }
    }
}

pub struct ToolDef {
    pub id: &'static str,
    /// English, the key of its translation.
    pub label: &'static str,
    pub description: &'static str,
    pub default: Perm,
}

pub const TOOLS: &[ToolDef] = &[
    ToolDef { id: "open_url", label: n_("Open websites"), description: n_("“Open youtube.com” — http and https only."), default: Perm::Allow },
    ToolDef { id: "web_search", label: n_("Search the web"), description: n_("“Search for pizza near me” — in your browser, with your search engine."), default: Perm::Allow },
    ToolDef { id: "browser", label: n_("Control the browser"), description: n_("“New tab”, “go back”, “what page is this?” — in the browser in front."), default: Perm::Allow },
    ToolDef { id: "browser_click", label: n_("Press buttons on web pages"), description: n_("“Click Sign in” — a visible button or link, by its name."), default: Perm::Ask },
    ToolDef { id: "windows", label: n_("Switch and arrange windows"), description: n_("“Switch to Outlook”, “minimise”, “snap this left”, “move it to the other screen”."), default: Perm::Allow },
    ToolDef { id: "close_app", label: n_("Close apps"), description: n_("“Close Spotify” — always asks first: anything unsaved could be lost."), default: Perm::Ask },
    ToolDef { id: "sound", label: n_("Volume and sound"), description: n_("“Set the volume to 30%”, “turn Discord down”, “mute Spotify”."), default: Perm::Allow },
    ToolDef { id: "discord", label: n_("Discord"), description: n_("“Mute me on Discord”, “deafen me” — with Discord's own shortcuts — and discord:// links."), default: Perm::Allow },
    ToolDef { id: "spotify_play", label: n_("Play on Spotify"), description: n_("“Play Daft Punk on Spotify” — found with Spotify's search."), default: Perm::Allow },
    ToolDef { id: "recent_files", label: n_("Open recent files"), description: n_("“Open my last file”, “open recent budget”."), default: Perm::Allow },
    ToolDef { id: "system_settings", label: n_("Windows settings"), description: n_("Dark or light mode, Wi-Fi, Bluetooth, brightness, Do not disturb, Night light."), default: Perm::Ask },
    ToolDef { id: "set_clipboard", label: n_("Set the clipboard"), description: n_("“Copy this to the clipboard”."), default: Perm::Ask },
    ToolDef { id: "type_text", label: n_("Type for you"), description: n_("“Type Dear Sam” — where the cursor is."), default: Perm::Ask },
    ToolDef { id: "open_app", label: n_("Open apps"), description: n_("“Open Notepad” — apps from the Start menu only."), default: Perm::Ask },
    ToolDef { id: "open_folder", label: n_("Open folders"), description: n_("“Open my downloads” — your own folders only."), default: Perm::Allow },
    ToolDef { id: "search_files", label: n_("Search your files"), description: n_("“Find files named invoice” — by name, in your own folders."), default: Perm::Ask },
    ToolDef { id: "reminders", label: n_("Reminders and to-dos"), description: n_("“Remind me to call Sam at 5pm”, “add milk to my to-do list”."), default: Perm::Allow },
    ToolDef { id: "mail", label: n_("New emails"), description: n_("“Any new emails?”: the count and who they're from, from your connected accounts."), default: Perm::Allow },
    ToolDef { id: "system_status", label: n_("System status"), description: n_("“How's my battery?”, “how much memory is free?”"), default: Perm::Allow },
    ToolDef { id: "clipboard", label: n_("Read the clipboard"), description: n_("“What's on my clipboard?” — shown in the chat, sent nowhere."), default: Perm::Ask },
];

pub fn def(id: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.id == id)
}

pub fn permission(conn: &Connection, id: &str) -> Perm {
    let perm = crate::music::kv_get(conn, &format!("perm.{id}"))
        .and_then(|v| Perm::parse(&v))
        .or_else(|| def(id).map(|d| d.default))
        .unwrap_or(Perm::Deny);
    // Closing an app always asks (round 4 item 5): its unsaved work is at stake.
    if id == "close_app" && perm == Perm::Allow {
        Perm::Ask
    } else {
        perm
    }
}

pub fn set_permission(conn: &Connection, id: &str, perm: Perm) -> Result<(), String> {
    def(id).ok_or("Unknown tool.")?;
    crate::music::kv_set(conn, &format!("perm.{id}"), perm.as_str());
    audit(conn, id, &json!({ "permission": perm.as_str() }), "permission changed");
    Ok(())
}

pub fn audit(conn: &Connection, tool: &str, args: &Value, outcome: &str) {
    let _ = conn.execute(
        "INSERT INTO tool_log (tool, args, outcome, at) VALUES (?1, ?2, ?3, ?4)",
        params![tool, args.to_string(), outcome, db::now()],
    );
    // The log keeps the last 1000 entries.
    let _ = conn.execute("DELETE FROM tool_log WHERE id NOT IN (SELECT id FROM tool_log ORDER BY id DESC LIMIT 1000)", []);
}

// ── What was asked ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    OpenUrl(String),
    WebSearch(String),
    Browser(crate::browser::Action),
    Pc(crate::pc::Action),
    OpenApp(String),
    OpenFolder(KnownFolder),
    SearchFiles(String),
    Remind { text: String, when: When },
    Todo(String),
    SystemStatus,
    Clipboard,
    Mail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownFolder {
    Home,
    Documents,
    Downloads,
    Desktop,
    Pictures,
    Music,
    Videos,
}

/// A time as said: relative, or a clock time today/tomorrow (local).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum When {
    In(i64),
    At { tomorrow: bool, hour: u32, minute: u32 },
}

impl Call {
    pub fn tool(&self) -> &'static str {
        match self {
            Call::OpenUrl(_) => "open_url",
            Call::WebSearch(_) => "web_search",
            Call::Browser(crate::browser::Action::Press(_)) => "browser_click",
            Call::Browser(_) => "browser",
            Call::Pc(a) => a.tool(),
            Call::OpenApp(_) => "open_app",
            Call::OpenFolder(_) => "open_folder",
            Call::SearchFiles(_) => "search_files",
            Call::Remind { .. } | Call::Todo(_) => "reminders",
            Call::SystemStatus => "system_status",
            Call::Clipboard => "clipboard",
            Call::Mail => "mail",
        }
    }

    fn args(&self) -> Value {
        match self {
            Call::OpenUrl(u) => json!({ "url": u }),
            Call::WebSearch(q) => json!({ "query": q }),
            Call::Browser(a) => json!({ "action": format!("{a:?}") }),
            Call::Pc(a) => json!({ "action": format!("{a:?}") }),
            Call::OpenApp(a) => json!({ "app": a }),
            Call::OpenFolder(f) => json!({ "folder": f }),
            Call::SearchFiles(q) => json!({ "query": q }),
            Call::Remind { text, when } => json!({ "text": text, "when": format!("{when:?}") }),
            Call::Todo(text) => json!({ "text": text }),
            Call::SystemStatus | Call::Clipboard | Call::Mail => json!({}),
        }
    }

    /// What will happen, for the approval question.
    fn describe(&self) -> String {
        match self {
            Call::OpenUrl(u) => tf("Open {url} in your browser?", &[("url", u)]),
            Call::WebSearch(q) => tf("Search the web for “{query}”?", &[("query", q)]),
            Call::Browser(crate::browser::Action::Press(name)) => tf("Press “{name}” on the page in front?", &[("name", name)]),
            Call::Browser(_) => t("Use the browser in front?"),
            Call::Pc(a) => a.describe(),
            Call::OpenApp(a) => tf("Open {app}?", &[("app", a)]),
            Call::OpenFolder(f) => tf("Open your {folder} folder?", &[("folder", &format!("{f:?}"))]),
            Call::SearchFiles(q) => tf("Search your folders for files named “{query}”?", &[("query", q)]),
            Call::Remind { text, .. } => tf("Set a reminder: {text}?", &[("text", text)]),
            Call::Todo(text) => tf("Add to your to-do list: {text}?", &[("text", text)]),
            Call::SystemStatus => t("Read the system status?"),
            Call::Clipboard => t("Read what's on your clipboard? It is shown here and sent nowhere."),
            Call::Mail => t("Check how many new emails you have?"),
        }
    }
}

/// The request without "hey Groovy", "please"… and end punctuation, in its
/// own case. Matching is done on its ASCII-lowercase copy, which has the same
/// byte offsets, so what the user typed can be cut out of it unchanged.
fn strip_leads(text: &str) -> String {
    let mut t: String = text.trim().chars().filter(|c| *c != '?' && *c != '!').collect();
    t = t.trim_end_matches('.').trim().to_string();
    for lead in ["ok groovy,", "ok groovy", "hey groovy,", "hey groovy", "groovy,", "groovy", "please", "can you", "could you", "would you"] {
        if t.to_ascii_lowercase().starts_with(lead) {
            t = t[lead.len()..].trim().trim_start_matches(',').trim().to_string();
        }
    }
    if t.to_ascii_lowercase().ends_with(" please") {
        t.truncate(t.len() - " please".len());
    }
    t.trim().to_string()
}

/// The part of `cased` at the place `part` (a slice of `lower`) is in `lower`.
fn as_typed<'a>(cased: &'a str, lower: &str, part: &str) -> &'a str {
    let start = part.as_ptr() as usize - lower.as_ptr() as usize;
    &cased[start..start + part.len()]
}

/// "in 10 minutes", "in an hour", "at 5pm", "at 17:30", "tomorrow at 9", "tomorrow".
pub fn parse_when(s: &str) -> Option<When> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("in ") {
        let mut words = rest.split_whitespace();
        let n = words.next()?;
        let n: i64 = match n {
            "a" | "an" | "one" => 1,
            "half" => return (words.next() == Some("an") && words.next()?.starts_with("hour")).then_some(When::In(1800)),
            "two" => 2,
            "three" => 3,
            "five" => 5,
            "ten" => 10,
            "fifteen" => 15,
            "twenty" => 20,
            "thirty" => 30,
            n => n.parse().ok()?,
        };
        let unit = words.next()?;
        let secs = if unit.starts_with("sec") {
            1
        } else if unit.starts_with("min") {
            60
        } else if unit.starts_with("hour") || unit == "hr" || unit == "hrs" {
            3600
        } else if unit.starts_with("day") {
            86_400
        } else {
            return None;
        };
        return (n > 0 && n <= 365 * 86_400 / secs).then_some(When::In(n * secs));
    }
    let (tomorrow, rest) = match s.strip_prefix("tomorrow") {
        Some(r) => (true, r.trim()),
        None => (false, s),
    };
    if tomorrow && rest.is_empty() {
        return Some(When::At { tomorrow: true, hour: 9, minute: 0 });
    }
    let rest = rest.strip_prefix("at ")?.trim();
    let (clock, suffix) = match rest.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (rest[..i].trim(), rest[i..].trim().replace('.', "")),
        None => (rest, String::new()),
    };
    let (h, m) = match clock.split_once([':', '.']) {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (clock.parse::<u32>().ok()?, 0),
    };
    let hour = match suffix.as_str() {
        "pm" | "p" if h < 12 => h + 12,
        "am" | "a" if h == 12 => 0,
        "" | "am" | "pm" | "a" | "p" | "oclock" | "o'clock" => h,
        _ => return None,
    };
    (hour < 24 && m < 60).then_some(When::At { tomorrow, hour, minute: m })
}

/// Unix seconds for `when`, from the local clock `now` (unix) and the local
/// time it is (y, m, d, h, min, s).
pub fn due_at(when: When, now: i64, local: (i64, u32, u32, u32, u32, u32)) -> i64 {
    match when {
        When::In(secs) => now + secs,
        When::At { tomorrow, hour, minute } => {
            let (y, mo, d, h, mi, s) = local;
            let today_secs = (h * 3600 + mi * 60 + s) as i64;
            let target = (hour * 3600 + minute * 60) as i64;
            let mut due = now - today_secs + target;
            if tomorrow {
                due += 86_400;
            } else if due <= now {
                // "at 9" when it is already 10: tomorrow at 9.
                due += 86_400;
            }
            let _ = (y, mo, d);
            due
        }
    }
}

fn known_folder(word: &str) -> Option<KnownFolder> {
    Some(match word.trim().trim_start_matches("my ").trim_start_matches("the ").trim_end_matches(" folder") {
        "home" | "user" => KnownFolder::Home,
        "documents" | "docs" | "document" => KnownFolder::Documents,
        "downloads" | "download" => KnownFolder::Downloads,
        "desktop" => KnownFolder::Desktop,
        "pictures" | "photos" | "images" => KnownFolder::Pictures,
        "music" => KnownFolder::Music,
        "videos" | "movies" => KnownFolder::Videos,
        _ => return None,
    })
}

fn looks_like_site(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(|c| c == '"' || c == '“' || c == '”');
    if s.contains(' ') || s.len() > 200 {
        return None;
    }
    let url = if s.starts_with("http://") || s.starts_with("https://") { s.to_string() } else { format!("https://{s}") };
    let parsed = reqwest::Url::parse(&url).ok()?;
    let host = parsed.host_str()?;
    let tlds = host.rsplit('.').next()?;
    (matches!(parsed.scheme(), "http" | "https") && host.contains('.') && tlds.len() >= 2 && tlds.chars().all(|c| c.is_ascii_alphabetic()))
        .then_some(url)
}

/// A tool request, or None for anything else (which goes to the assistant).
pub fn parse(text: &str) -> Option<Call> {
    let original = text.trim();
    let cased = strip_leads(original);
    let t = cased.to_ascii_lowercase();
    let t = t.as_str();

    // Reminders: "remind me to X in 10 minutes", "remind me at 5pm to X", "set a reminder to X at 9".
    let reminder = t.strip_prefix("remind me ").or_else(|| t.strip_prefix("set a reminder ")).or_else(|| t.strip_prefix("set a reminder for me "));
    if let Some(rest) = reminder {
        let rest = rest.trim_start_matches("for me ").trim();
        // "at 5pm to X" / "in an hour to X"
        if let Some(i) = rest.find(" to ") {
            if let Some(when) = parse_when(&rest[..i]) {
                let text = rest[i + 4..].trim();
                if !text.is_empty() {
                    return Some(Call::Remind { text: capitalise(as_typed(&cased, t, text)), when });
                }
            }
        }
        let body = rest.strip_prefix("to ").or_else(|| rest.strip_prefix("that ")).or_else(|| rest.strip_prefix("about "))?;
        // The earliest place where the rest of the sentence is a time.
        let best = [" in ", " at ", " tomorrow"]
            .iter()
            .flat_map(|sep| body.match_indices(sep).map(|(i, _)| i).collect::<Vec<_>>())
            .filter_map(|i| parse_when(body[i..].trim()).map(|w| (i, w)))
            .min_by_key(|(i, _)| *i);
        let (i, when) = best?;
        let text = body[..i].trim();
        return (!text.is_empty()).then(|| Call::Remind { text: capitalise(as_typed(&cased, t, text)), when });
    }
    // To-dos: "add milk to my to-do list", "add a to-do: call mum".
    for prefix in ["add a to-do", "add a todo", "add to-do", "add todo", "new to-do", "new todo"] {
        if let Some(rest) = t.strip_prefix(prefix).filter(|r| r.starts_with([' ', ':'])) {
            let item = rest.trim_start_matches([' ', ':']).trim_start_matches("to ").trim();
            if !item.is_empty() {
                return Some(Call::Todo(capitalise(as_typed(&cased, t, item))));
            }
        }
    }
    if let Some(rest) = t.strip_prefix("add ") {
        for list in [" to my to-do list", " to my todo list", " to my to do list", " to the to-do list", " to my list", " to my todos"] {
            if let Some(item) = rest.strip_suffix(list) {
                if !item.trim().is_empty() {
                    return Some(Call::Todo(capitalise(as_typed(&cased, t, item.trim()))));
                }
            }
        }
    }
    // New mail.
    let mail_words = ["new email", "new emails", "new mail", "unread email", "unread emails", "unread mail", "my email", "my emails", "my mail", "my inbox", "any email", "any emails", "any mail"];
    if (t.starts_with("any") || t.starts_with("do i have") || t.starts_with("have i got") || t.starts_with("check") || t.starts_with("how many") || t.starts_with("what") || t.starts_with("read"))
        && mail_words.iter().any(|w| t.contains(w))
        && t.split_whitespace().count() <= 10
    {
        return Some(Call::Mail);
    }
    // System status.
    let status_words = ["battery", "cpu", "processor", "memory usage", "ram", "free memory", "disk space", "storage space", "system status"];
    if (t.starts_with("how") || t.starts_with("what") || t.starts_with("show") || t.starts_with("check") || t.starts_with("tell me"))
        && status_words.iter().any(|w| t.contains(w))
        && t.split_whitespace().count() <= 10
    {
        return Some(Call::SystemStatus);
    }
    // Clipboard.
    if ["what's on my clipboard", "whats on my clipboard", "what is on my clipboard", "read my clipboard", "read the clipboard", "show my clipboard", "show the clipboard"]
        .iter()
        .any(|p| t.starts_with(p))
    {
        return Some(Call::Clipboard);
    }
    // Files: "find files named X", "search my files for X".
    for prefix in ["find files named ", "find files called ", "find the file ", "find my file ", "search my files for ", "search files for ", "look for files named "] {
        if let Some(q) = t.strip_prefix(prefix) {
            let q = q.trim().trim_matches(|c| c == '"' || c == '“' || c == '”').trim();
            if !q.is_empty() && q.len() <= 100 {
                return Some(Call::SearchFiles(q.to_string()));
            }
        }
    }
    // The web: "search for X", "look up X", "google X" (round 4 item 4).
    for prefix in ["search the web for ", "search the internet for ", "search online for ", "search for ", "look up ", "google ", "web search "] {
        if let Some(q) = t.strip_prefix(prefix) {
            let q = as_typed(&cased, t, q.trim()).trim_matches(|c| c == '"' || c == '“' || c == '”').trim();
            if !q.is_empty() && q.len() <= 200 {
                return Some(Call::WebSearch(q.to_string()));
            }
        }
    }
    // The browser in front: new tab, back, reload, "what page is this?", "click Sign in".
    if let Some(action) = crate::browser::Action::parse(t) {
        return Some(Call::Browser(match action {
            // The button's name as typed.
            crate::browser::Action::Press(name) => {
                let i = t.rfind(&name).unwrap_or(0);
                crate::browser::Action::Press(cased.get(i..i + name.len()).unwrap_or(&name).to_string())
            }
            other => other,
        }));
    }
    // The PC: windows, sound, Discord, Spotify, recent files, Windows settings,
    // the clipboard and typing (round 4 item 5).
    if let Some(action) = crate::pc::parse(t, &cased) {
        return Some(Call::Pc(action));
    }
    // Open: a website, a folder or an app.
    let what = t.strip_prefix("open ").or_else(|| t.strip_prefix("launch ")).or_else(|| t.strip_prefix("start ")).or_else(|| t.strip_prefix("go to "))?;
    let what = what.trim();
    if what.is_empty() || what.split_whitespace().count() > 5 {
        return None;
    }
    // The site keeps its case and path as typed.
    let raw_tail = original.split_whitespace().last().unwrap_or("");
    if let Some(url) = looks_like_site(what).and_then(|_| looks_like_site(raw_tail.trim_end_matches(['.', '!', '?']))) {
        return Some(Call::OpenUrl(url));
    }
    if t.starts_with("go to ") {
        return None;
    }
    if let Some(f) = known_folder(what) {
        if what.ends_with("folder") || what.starts_with("my ") || ["downloads", "documents", "desktop", "pictures"].contains(&what) {
            return Some(Call::OpenFolder(f));
        }
    }
    // An app: "open notepad", "launch the calculator". Not "open the door".
    let app = what.trim_start_matches("the ").trim_start_matches("my ").trim_end_matches(" app").trim();
    if app.is_empty() || app.len() > 40 || ["door", "window", "file", "a file", "chat", "it", "this", "that", "up"].contains(&app) {
        return None;
    }
    Some(Call::OpenApp(app.to_string()))
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ── Pending approvals ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingAction {
    pub id: u64,
    pub tool: String,
    pub label: String,
}

struct Pending {
    call: Call,
    at: Instant,
}

static PENDING: Mutex<Option<HashMap<u64, Pending>>> = Mutex::new(None);
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
const PENDING_FOR: Duration = Duration::from_secs(300);

/// The answer to a recognised request: the result, or the question with buttons.
pub fn handle(call: Call) -> (String, Option<PendingAction>) {
    let Ok(conn) = db::conn() else { return (t("The local database is not available."), None) };
    let tool = call.tool();
    match permission(&conn, tool) {
        Perm::Deny => {
            audit(&conn, tool, &call.args(), "denied by setting");
            (tf("“{tool}” is turned off in Settings → Permissions.", &[("tool", &t(def(tool).map(|d| d.label).unwrap_or(tool)))]), None)
        }
        Perm::Allow => {
            drop(conn);
            (run(&call), None)
        }
        Perm::Ask => {
            let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let question = call.describe();
            audit(&conn, tool, &call.args(), "asked");
            let mut p = PENDING.lock().unwrap();
            let map = p.get_or_insert_with(HashMap::new);
            map.retain(|_, v| v.at.elapsed() < PENDING_FOR);
            map.insert(id, Pending { call, at: Instant::now() });
            (question, Some(PendingAction { id, tool: tool.to_string(), label: def(tool).map(|d| d.label.to_string()).unwrap_or_default() }))
        }
    }
}

/// "once" | "always" | "deny" for a pending request.
pub fn decide(id: u64, decision: &str) -> Result<String, String> {
    let pending = PENDING.lock().unwrap().as_mut().and_then(|m| m.remove(&id));
    let Some(p) = pending.filter(|p| p.at.elapsed() < PENDING_FOR) else {
        return Err(t("That request has expired: ask again."));
    };
    let conn = db::conn()?;
    let tool = p.call.tool();
    match decision {
        "deny" => {
            audit(&conn, tool, &p.call.args(), "declined");
            Ok(t("OK, I won't."))
        }
        "always" | "once" => {
            if decision == "always" {
                set_permission(&conn, tool, Perm::Allow)?;
            }
            audit(&conn, tool, &p.call.args(), "approved");
            drop(conn);
            Ok(run(&p.call))
        }
        _ => Err("Unknown decision.".into()),
    }
}

// ── Running a tool ────────────────────────────────────────────────────────────

fn run(call: &Call) -> String {
    let result = match call {
        Call::OpenUrl(url) => crate::browser::open(url).map(|browser| tf("Opened {url} in {browser}.", &[("url", url), ("browser", &browser)])),
        Call::WebSearch(q) => crate::browser::search(q),
        Call::Browser(action) => crate::browser::act(action),
        Call::Pc(action) => crate::pc::run(action),
        Call::OpenFolder(f) => match folder_path(*f) {
            Some(p) if p.is_dir() => {
                crate::platform::reveal_folder(&p.to_string_lossy());
                Ok(tf("Opened {path}.", &[("path", &p.to_string_lossy())]))
            }
            _ => Err(t("I couldn't find that folder.")),
        },
        Call::OpenApp(name) => open_app(name),
        Call::SearchFiles(q) => Ok(search_files(q)),
        Call::Remind { text, when } => {
            let lt = crate::platform::local_time();
            let local = (lt.year as i64, lt.month, lt.day, lt.hour, lt.minute, lt.second);
            let due = due_at(*when, db::now(), local);
            db::conn()
                .and_then(|c| crate::tasks::add(&c, text, Some(due)))
                .map(|_| tf("Reminder set for {when}: {text}", &[("when", &describe_due(due - db::now())), ("text", text)]))
        }
        Call::Todo(text) => db::conn().and_then(|c| crate::tasks::add(&c, text, None)).map(|_| tf("Added to your to-do list: {text}", &[("text", text)])),
        Call::SystemStatus => Ok(status_text()),
        Call::Clipboard => read_clipboard(),
        Call::Mail => Ok(crate::mail::summary()),
    };
    let outcome = if result.is_ok() { "done" } else { "failed" };
    if let Ok(conn) = db::conn() {
        audit(&conn, call.tool(), &call.args(), outcome);
    }
    log::line(format!("tool {}: {outcome}", call.tool()));
    result.unwrap_or_else(|e| e)
}

fn describe_due(secs: i64) -> String {
    if secs < 3600 {
        tf("in {n} min", &[("n", &((secs + 59) / 60).max(1).to_string())])
    } else if secs < 86_400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        tf("in {h} h {m} min", &[("h", &h.to_string()), ("m", &m.to_string())])
    } else {
        tf("in {n} days", &[("n", &(secs / 86_400).to_string())])
    }
}

pub fn folder_path(f: KnownFolder) -> Option<PathBuf> {
    let home = crate::platform::home_dir();
    let sub = match f {
        KnownFolder::Home => return Some(home),
        KnownFolder::Documents => "Documents",
        KnownFolder::Downloads => "Downloads",
        KnownFolder::Desktop => "Desktop",
        KnownFolder::Pictures => "Pictures",
        KnownFolder::Music => "Music",
        KnownFolder::Videos => "Videos",
    };
    Some(home.join(sub))
}

/// File names containing `query` (case-insensitive) in the user's own folders.
pub fn find_files(roots: &[PathBuf], query: &str, limit: usize) -> Vec<PathBuf> {
    let q = query.to_lowercase();
    let mut out = Vec::new();
    let mut visited = 0usize;
    for root in roots {
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                visited += 1;
                if visited > 50_000 || out.len() >= limit {
                    return out;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" || name == "AppData" {
                    continue;
                }
                let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
                if meta.is_dir() {
                    if depth < 6 {
                        stack.push((e.path(), depth + 1));
                    }
                } else if name.to_lowercase().contains(&q) {
                    out.push(e.path());
                }
            }
        }
    }
    out
}

fn search_files(q: &str) -> String {
    let roots: Vec<PathBuf> = [KnownFolder::Desktop, KnownFolder::Documents, KnownFolder::Downloads, KnownFolder::Pictures, KnownFolder::Music, KnownFolder::Videos]
        .iter()
        .filter_map(|f| folder_path(*f))
        .filter(|p| p.is_dir())
        .collect();
    let found = find_files(&roots, q, 25);
    if found.is_empty() {
        return tf("No files named like “{query}” in Desktop, Documents, Downloads, Pictures, Music or Videos.", &[("query", q)]);
    }
    let mut out = tf("Files named like “{query}”:", &[("query", q)]);
    for p in &found {
        out.push_str(&format!("\n- `{}`", p.to_string_lossy()));
    }
    if found.len() >= 25 {
        out.push_str(&format!("\n\n{}", t("Showing the first 25.")));
    }
    out
}

fn status_text() -> String {
    let s = crate::system::read_now();
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let mut lines = vec![
        tf("CPU: {pct}% ({cpus} threads)", &[("pct", &format!("{:.0}", s.cpu_usage)), ("cpus", &s.cpus.to_string())]),
        tf("Memory: {used} GB used of {total} GB", &[("used", &format!("{:.1}", (s.memory_total - s.memory_available) as f64 / GB)), ("total", &format!("{:.1}", s.memory_total as f64 / GB))]),
    ];
    for d in s.disks.iter().take(4) {
        lines.push(tf("Disk {name}: {free} GB free of {total} GB", &[("name", &d.mount), ("free", &format!("{:.0}", d.available as f64 / GB)), ("total", &format!("{:.0}", d.total as f64 / GB))]));
    }
    lines.push(match &s.battery {
        Some(b) if b.charging => tf("Battery: {pct}%, charging", &[("pct", &b.percent.to_string())]),
        Some(b) => tf("Battery: {pct}%", &[("pct", &b.percent.to_string())]),
        None => t("No battery"),
    });
    lines.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
}

fn read_clipboard() -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let text = cb.get_text().map_err(|_| t("There's no text on the clipboard."))?;
    let shown: String = text.chars().take(2000).collect();
    let more = if text.chars().count() > 2000 { format!("\n\n{}", t("(cut to the first 2000 characters)")) } else { String::new() };
    Ok(format!("{}\n\n```\n{}\n```{more}", t("On your clipboard:"), shown.replace("```", "ʼʼʼ")))
}

// ── Apps ──────────────────────────────────────────────────────────────────────

/// Start-menu shortcuts (Windows) or .desktop entries (Linux), by display name.
pub fn app_shortcuts() -> Vec<(String, PathBuf)> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    {
        for var in ["APPDATA", "PROGRAMDATA"] {
            if let Some(base) = std::env::var_os(var) {
                dirs.push(PathBuf::from(base).join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
            }
        }
    }
    #[cfg(not(windows))]
    {
        dirs.push(PathBuf::from("/usr/share/applications"));
        dirs.push(crate::platform::home_dir().join(".local/share/applications"));
    }
    let mut out = Vec::new();
    for d in dirs {
        let mut stack = vec![(d, 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && depth < 3 {
                    stack.push((p, depth + 1));
                    continue;
                }
                let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                if ext == "lnk" || ext == "url" || ext == "desktop" {
                    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    out.push((name, p));
                }
            }
        }
    }
    out
}

/// The best match for `wanted` among `apps`: exact name, then starts-with, then
/// contains; uninstallers and help files never.
pub fn best_app<'a>(apps: &'a [(String, PathBuf)], wanted: &str) -> Option<&'a (String, PathBuf)> {
    let w = wanted.to_lowercase();
    let ok = |n: &str| {
        let n = n.to_lowercase();
        !n.contains("uninstall") && !n.contains("readme") && !n.contains("help") && !n.contains("documentation")
    };
    let candidates: Vec<&(String, PathBuf)> = apps.iter().filter(|(n, _)| ok(n)).collect();
    candidates
        .iter()
        .find(|(n, _)| n.to_lowercase() == w)
        .or_else(|| candidates.iter().filter(|(n, _)| n.to_lowercase().starts_with(&w)).min_by_key(|(n, _)| n.len()))
        .or_else(|| candidates.iter().filter(|(n, _)| n.to_lowercase().contains(&w)).min_by_key(|(n, _)| n.len()))
        .copied()
}

fn open_app(name: &str) -> Result<String, String> {
    let apps = app_shortcuts();
    let Some((found, path)) = best_app(&apps, name) else {
        return Err(tf("I couldn't find an app called “{app}” in the Start menu.", &[("app", name)]));
    };
    launch(path)?;
    Ok(tf("Opened {app}.", &[("app", found)]))
}

#[cfg(windows)]
fn launch(path: &Path) -> Result<(), String> {
    // Explorer opens a shortcut exactly as a double-click would.
    let mut cmd = std::process::Command::new("explorer.exe");
    cmd.arg(path);
    crate::platform::no_console(&mut cmd).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn launch(path: &Path) -> Result<(), String> {
    let id = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    std::process::Command::new("gtk-launch").arg(id).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub permission: Perm,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub tool: String,
    pub args: String,
    pub outcome: String,
    pub at: i64,
}

#[tauri::command]
pub fn tools_list() -> Result<Vec<ToolInfo>, String> {
    let conn = db::conn()?;
    Ok(TOOLS.iter().map(|d| ToolInfo { id: d.id, label: d.label, description: d.description, permission: permission(&conn, d.id) }).collect())
}

#[tauri::command]
pub fn tools_set_permission(id: String, permission: String) -> Result<(), String> {
    let perm = Perm::parse(&permission).ok_or("Unknown permission.")?;
    set_permission(&*db::conn()?, &id, perm)
}

#[tauri::command]
pub fn tools_decide(id: u64, decision: String) -> Result<String, String> {
    decide(id, &decision)
}

#[tauri::command]
pub fn tools_log() -> Result<Vec<LogEntry>, String> {
    let conn = db::conn()?;
    let mut stmt = conn.prepare("SELECT tool, args, outcome, at FROM tool_log ORDER BY id DESC LIMIT 200").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok(LogEntry { tool: r.get(0)?, args: r.get(1)?, outcome: r.get(2)?, at: r.get(3)? }))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tools_log_clear() -> Result<(), String> {
    db::conn()?.execute("DELETE FROM tool_log", []).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_reminders() {
        assert_eq!(
            parse("Remind me to call Sam in 10 minutes"),
            Some(Call::Remind { text: "Call Sam".into(), when: When::In(600) })
        );
        assert_eq!(parse("remind me in an hour to stretch"), Some(Call::Remind { text: "Stretch".into(), when: When::In(3600) }));
        assert_eq!(
            parse("Hey Groovy, remind me to take the bins out at 7pm."),
            Some(Call::Remind { text: "Take the bins out".into(), when: When::At { tomorrow: false, hour: 19, minute: 0 } })
        );
        assert_eq!(
            parse("remind me at 17:30 to leave"),
            Some(Call::Remind { text: "Leave".into(), when: When::At { tomorrow: false, hour: 17, minute: 30 } })
        );
        assert_eq!(
            parse("remind me to pay rent tomorrow at 9am"),
            Some(Call::Remind { text: "Pay rent".into(), when: When::At { tomorrow: true, hour: 9, minute: 0 } })
        );
        assert_eq!(parse("remind me to pay rent tomorrow"), Some(Call::Remind { text: "Pay rent".into(), when: When::At { tomorrow: true, hour: 9, minute: 0 } }));
        assert_eq!(parse("remind me about it"), None, "no time: the assistant answers");
    }

    #[test]
    fn times() {
        assert_eq!(parse_when("in half an hour"), Some(When::In(1800)));
        assert_eq!(parse_when("in 2 hours"), Some(When::In(7200)));
        assert_eq!(parse_when("at 12am"), Some(When::At { tomorrow: false, hour: 0, minute: 0 }));
        assert_eq!(parse_when("at 12pm"), Some(When::At { tomorrow: false, hour: 12, minute: 0 }));
        assert_eq!(parse_when("at 5:15 p.m."), Some(When::At { tomorrow: false, hour: 17, minute: 15 }));
        assert_eq!(parse_when("at 25"), None);
        assert_eq!(parse_when("in 0 minutes"), None);
        assert_eq!(parse_when("at noonish"), None);
        // 10:00 now: "at 9" is tomorrow, "at 11" today.
        let now = 1_000_000;
        let local = (2026, 10, 15, 10, 0, 0);
        assert_eq!(due_at(When::At { tomorrow: false, hour: 11, minute: 0 }, now, local), now + 3600);
        assert_eq!(due_at(When::At { tomorrow: false, hour: 9, minute: 0 }, now, local), now + 23 * 3600);
        assert_eq!(due_at(When::At { tomorrow: true, hour: 10, minute: 30 }, now, local), now + 86_400 + 1800);
        assert_eq!(due_at(When::In(60), now, local), now + 60);
    }

    #[test]
    fn understands_todos_status_clipboard_and_files() {
        assert_eq!(parse("Add milk to my to-do list"), Some(Call::Todo("Milk".into())));
        assert_eq!(parse("add a to-do: call mum"), Some(Call::Todo("Call mum".into())));
        assert_eq!(parse("How's my battery?"), Some(Call::SystemStatus));
        assert_eq!(parse("OK Groovy, any new emails?"), Some(Call::Mail));
        assert_eq!(parse("do I have any new mail"), Some(Call::Mail));
        assert_eq!(parse("check my inbox"), Some(Call::Mail));
        assert_eq!(parse("how much ram is free"), Some(Call::SystemStatus));
        assert_eq!(parse("What's on my clipboard?"), Some(Call::Clipboard));
        assert_eq!(parse("find files named invoice"), Some(Call::SearchFiles("invoice".into())));
        assert_eq!(parse("How does a CPU cache work in modern processors and why does it matter for performance tuning?"), None);
    }

    #[test]
    fn understands_open() {
        assert_eq!(parse("Search for Best Pizza near me"), Some(Call::WebSearch("Best Pizza near me".into())));
        assert_eq!(parse("hey groovy, look up the Eiffel Tower"), Some(Call::WebSearch("the Eiffel Tower".into())));
        assert_eq!(parse("search my files for invoice"), Some(Call::SearchFiles("invoice".into())));
        assert_eq!(parse("new tab"), Some(Call::Browser(crate::browser::Action::NewTab)));
        assert_eq!(parse("Click the Sign In button"), Some(Call::Browser(crate::browser::Action::Press("Sign In".into()))));
        assert_eq!(Call::Browser(crate::browser::Action::Press("x".into())).tool(), "browser_click");
        assert_eq!(Call::Browser(crate::browser::Action::Back).tool(), "browser");
        assert_eq!(parse("open youtube.com"), Some(Call::OpenUrl("https://youtube.com".into())));
        assert_eq!(parse("Open https://github.com/WavezFX"), Some(Call::OpenUrl("https://github.com/WavezFX".into())));
        assert_eq!(parse("open my downloads"), Some(Call::OpenFolder(KnownFolder::Downloads)));
        assert_eq!(parse("open the documents folder"), Some(Call::OpenFolder(KnownFolder::Documents)));
        assert_eq!(parse("open notepad"), Some(Call::OpenApp("notepad".into())));
        assert_eq!(parse("launch the calculator app"), Some(Call::OpenApp("calculator".into())));
        assert_eq!(parse("open the door"), None);
        assert_eq!(parse("open javascript:alert(1)"), Some(Call::OpenApp("javascript:alert(1)".into())), "never a URL: only a Start-menu name match");
        assert_eq!(looks_like_site("javascript:alert(1)"), None);
        assert_eq!(looks_like_site("file:///c:/windows"), None);
        assert_eq!(parse("What is the weather like today?"), None);
    }

    #[test]
    fn apps_are_matched_by_name_never_uninstallers() {
        let apps = vec![
            ("Uninstall Notepad++".to_string(), PathBuf::from("a")),
            ("Notepad++".to_string(), PathBuf::from("b")),
            ("Notepad".to_string(), PathBuf::from("c")),
            ("Calculator".to_string(), PathBuf::from("d")),
        ];
        assert_eq!(best_app(&apps, "notepad").unwrap().1, PathBuf::from("c"));
        assert_eq!(best_app(&apps, "notepad++").unwrap().1, PathBuf::from("b"));
        assert_eq!(best_app(&apps, "calc").unwrap().1, PathBuf::from("d"));
        assert!(best_app(&apps, "photoshop").is_none());
    }

    #[test]
    fn permissions_default_and_persist() {
        let c = db::open_in_memory().unwrap();
        assert_eq!(permission(&c, "open_app"), Perm::Ask);
        assert_eq!(permission(&c, "open_url"), Perm::Allow);
        assert_eq!(permission(&c, "nonsense"), Perm::Deny);
        set_permission(&c, "open_app", Perm::Deny).unwrap();
        assert_eq!(permission(&c, "open_app"), Perm::Deny);
        assert!(set_permission(&c, "nonsense", Perm::Allow).is_err());
        let n: i64 = c.query_row("SELECT count(*) FROM tool_log", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "the change is audited");
    }

    #[test]
    fn finds_files_by_name_skipping_hidden_and_builds() {
        let d = std::env::temp_dir().join(format!("groovy-tools-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a/node_modules")).unwrap();
        std::fs::create_dir_all(d.join(".hidden")).unwrap();
        std::fs::write(d.join("a/Invoice-2026.pdf"), b"").unwrap();
        std::fs::write(d.join("a/node_modules/invoice.js"), b"").unwrap();
        std::fs::write(d.join(".hidden/invoice.txt"), b"").unwrap();
        let found = find_files(std::slice::from_ref(&d), "invoice", 10);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("Invoice-2026.pdf"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
