// Real mail accounts (round 3C item 5): Gmail (Google sign-in, Gmail API),
// Outlook / Hotmail (Microsoft sign-in, Graph), iCloud Mail and any other
// IMAP server (address + app password). Groovy reads only the newest
// messages' sender, subject and snippet, counts the unread, and can mark one
// read or archive it. It checks every few minutes while an account is
// connected, and never otherwise. Round 3D item 8: a message's text is read
// only when the user opens it in the island (never cached or logged), and a
// reply goes out only when they press Send (Gmail API, Graph, or SMTP).
//
// Privacy: the only network calls are to the mail providers. Passwords and
// refresh tokens live in Windows Credential Manager (`mail-account-<id>`);
// the accounts list (kind, address, server) is in groovy.db; what was read
// stays in memory. Disconnecting deletes all three.

pub mod api;
pub mod body;
pub mod imap;
pub mod oauth;
pub mod smtp;
pub mod text;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db;
use crate::i18n::{t, tf};
use oauth::Provider;

/// How often connected accounts are checked (the brief: every 2–5 minutes).
pub const CHECK_EVERY: Duration = Duration::from_secs(180);
/// Messages kept per account: what the panel shows.
const NEWEST: u32 = 8;
pub const ICLOUD_HOST: &str = "imap.mail.me.com";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: i64,
    /// "gmail" | "outlook" | "icloud" | "imap"
    pub kind: String,
    pub email: String,
    pub host: String,
    pub port: u16,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub account: i64,
    pub id: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    pub at: i64,
    pub unread: bool,
    /// Where "open" goes: the web mail, when there is one.
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inbox {
    pub account: Account,
    pub unread: u32,
    pub messages: Vec<Message>,
    pub error: Option<String>,
    pub checked_at: i64,
    /// Mark read and Archive work on every kind of account here.
    pub can_modify: bool,
}

static INBOXES: Mutex<Vec<Inbox>> = Mutex::new(Vec::new());
static TOKENS: Mutex<Option<HashMap<i64, oauth::Tokens>>> = Mutex::new(None);

fn secret_key(id: i64) -> String {
    format!("mail-account-{id}")
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

pub fn accounts(conn: &Connection) -> Result<Vec<Account>, String> {
    let mut stmt = conn.prepare("SELECT id, kind, email, host, port, username FROM mail_accounts ORDER BY id").map_err(sql)?;
    let rows = stmt
        .query_map([], |r| Ok(Account { id: r.get(0)?, kind: r.get(1)?, email: r.get(2)?, host: r.get(3)?, port: r.get::<_, i64>(4)? as u16, username: r.get(5)? }))
        .map_err(sql)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(sql)
}

fn insert(conn: &Connection, kind: &str, email: &str, host: &str, port: u16, username: &str) -> Result<Account, String> {
    conn.execute(
        "INSERT INTO mail_accounts (kind, email, host, port, username, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![kind, email, host, port as i64, username, db::now()],
    )
    .map_err(sql)?;
    let id = conn.last_insert_rowid();
    Ok(Account { id, kind: kind.into(), email: email.into(), host: host.into(), port, username: username.into() })
}

/// The web mail for an IMAP account, when the server is a known one.
pub fn webmail(kind: &str, host: &str) -> Option<&'static str> {
    let host = host.to_ascii_lowercase();
    match () {
        _ if kind == "icloud" || host.ends_with("mail.me.com") => Some("https://www.icloud.com/mail/"),
        _ if host == "imap.gmail.com" => Some("https://mail.google.com/"),
        _ if host.ends_with("outlook.com") || host.ends_with("office365.com") => Some("https://outlook.live.com/mail/"),
        _ if host.ends_with("mail.yahoo.com") => Some("https://mail.yahoo.com/"),
        _ => None,
    }
}

/// A host name for IMAP: letters, digits, dots and hyphens.
pub fn valid_host(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253 && host.split('.').all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) && host.contains('.')
}

fn valid_email(email: &str) -> bool {
    let e = email.trim();
    e.len() <= 254 && e.split_once('@').is_some_and(|(a, d)| !a.is_empty() && d.contains('.')) && !e.contains(char::is_whitespace)
}

// ── Reading ───────────────────────────────────────────────────────────────────

async fn access_token(a: &Account) -> Result<String, String> {
    let now = db::now();
    if let Some(tok) = TOKENS.lock().unwrap().as_ref().and_then(|m| m.get(&a.id)).filter(|t| t.expires_at > now) {
        return Ok(tok.access.clone());
    }
    let refresh = crate::secrets::get_internal(&secret_key(a.id)).ok_or_else(|| t("The account needs to be connected again."))?;
    let tokens = oauth::refresh(provider(&a.kind)?, &refresh).await?;
    if let Some(r) = tokens.refresh.as_deref().filter(|r| *r != refresh) {
        crate::secrets::set_internal(&secret_key(a.id), r)?;
    }
    let access = tokens.access.clone();
    TOKENS.lock().unwrap().get_or_insert_with(HashMap::new).insert(a.id, tokens);
    Ok(access)
}

fn provider(kind: &str) -> Result<Provider, String> {
    match kind {
        "gmail" => Ok(Provider::Google),
        "outlook" => Ok(Provider::Microsoft),
        _ => Err("not an OAuth account".into()),
    }
}

async fn imap_session(a: &Account) -> Result<imap::Session, String> {
    let password = crate::secrets::get_internal(&secret_key(a.id)).ok_or_else(|| t("The account needs to be connected again."))?;
    let mut s = imap::Session::connect(&a.host, a.port).await?;
    s.login(&a.username, &password).await?;
    Ok(s)
}

async fn fetch(a: &Account) -> Result<(u32, Vec<Message>), String> {
    match a.kind.as_str() {
        "gmail" => api::gmail_inbox(&access_token(a).await?, a.id, &a.email, NEWEST).await,
        "outlook" => api::graph_inbox(&access_token(a).await?, a.id, NEWEST).await,
        _ => {
            let mut s = imap_session(a).await?;
            let exists = s.inbox().await?;
            let unread = s.unread().await?;
            let newest = s.newest(exists, NEWEST).await?;
            s.logout().await;
            let link = webmail(&a.kind, &a.host).map(str::to_string);
            Ok((
                unread,
                newest
                    .into_iter()
                    .map(|h| Message { account: a.id, id: h.uid.to_string(), from: h.from, subject: h.subject, snippet: String::new(), at: h.at, unread: !h.seen, link: link.clone() })
                    .collect(),
            ))
        }
    }
}

/// New unread messages: in `now` but not among what was seen before.
pub fn new_unread(before: &[Inbox], now: &[Inbox]) -> Vec<Message> {
    let known: HashSet<(i64, &str)> = before.iter().flat_map(|i| i.messages.iter().map(|m| (m.account, m.id.as_str()))).collect();
    let checked: HashSet<i64> = before.iter().filter(|i| i.error.is_none()).map(|i| i.account.id).collect();
    now.iter()
        // An account's first check is its backlog, not news.
        .filter(|i| checked.contains(&i.account.id))
        .flat_map(|i| i.messages.iter())
        .filter(|m| m.unread && !known.contains(&(m.account, m.id.as_str())))
        .cloned()
        .collect()
}

/// Checks every account now; tells the island what changed.
pub async fn refresh_all(app: Option<&AppHandle>) -> Vec<Inbox> {
    let list = db::conn().and_then(|c| accounts(&c)).unwrap_or_default();
    let mut fresh = Vec::new();
    for a in list {
        let (unread, messages, error) = match fetch(&a).await {
            Ok((u, m)) => (u, m, None),
            Err(e) => {
                crate::log::line(format!("mail: account {} ({}) not read: {e}", a.id, a.kind));
                let old = INBOXES.lock().unwrap().iter().find(|i| i.account.id == a.id).map(|i| (i.unread, i.messages.clone()));
                let (u, m) = old.unwrap_or_default();
                (u, m, Some(e))
            }
        };
        fresh.push(Inbox { account: a, unread, messages, error, checked_at: db::now(), can_modify: true });
    }
    let news = {
        let mut cache = INBOXES.lock().unwrap();
        let news = new_unread(&cache, &fresh);
        *cache = fresh.clone();
        news
    };
    if let Some(app) = app {
        let _ = app.emit("mail-inboxes", &fresh);
        if !news.is_empty() {
            let _ = app.emit("mail-new", &news);
        }
    }
    fresh
}

/// Starts the check: every CHECK_EVERY while an account is connected.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            if db::conn().and_then(|c| accounts(&c)).is_ok_and(|l| !l.is_empty()) {
                refresh_all(Some(&app)).await;
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// "OK Groovy, any new emails?": the count and the top senders, from the last check.
pub fn summary() -> String {
    let connected = db::conn().and_then(|c| accounts(&c)).map(|l| l.len()).unwrap_or(0);
    if connected == 0 {
        return t("No mail account is connected yet. Connect one in Settings → Mail.");
    }
    let inboxes = INBOXES.lock().unwrap().clone();
    if inboxes.is_empty() {
        return t("I haven't checked your mail yet. Ask me again in a minute.");
    }
    summary_of(&inboxes)
}

pub fn summary_of(inboxes: &[Inbox]) -> String {
    let unread: u32 = inboxes.iter().map(|i| i.unread).sum();
    if unread == 0 {
        return t("No new emails.");
    }
    let mut senders: Vec<&str> = Vec::new();
    let mut newest: Vec<&Message> = inboxes.iter().flat_map(|i| i.messages.iter()).filter(|m| m.unread).collect();
    newest.sort_by(|a, b| b.at.cmp(&a.at));
    for m in newest {
        if !m.from.is_empty() && !senders.contains(&m.from.as_str()) && senders.len() < 3 {
            senders.push(&m.from);
        }
    }
    let count = if unread == 1 { t("You have 1 unread email") } else { tf("You have {n} unread emails", &[("n", &unread.to_string())]) };
    if senders.is_empty() {
        format!("{count}.")
    } else {
        tf("{count}, from {senders}.", &[("count", &count), ("senders", &senders.join(", "))])
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn mail_accounts() -> Result<Vec<Account>, String> {
    accounts(&*db::conn()?)
}

#[tauri::command]
pub fn mail_inboxes() -> Vec<Inbox> {
    INBOXES.lock().unwrap().clone()
}

#[tauri::command]
pub async fn mail_refresh(app: AppHandle) -> Vec<Inbox> {
    refresh_all(Some(&app)).await
}

/// iCloud ("icloud": the Apple ID and an app-specific password) or any IMAP server.
#[tauri::command]
pub async fn mail_connect_imap(app: AppHandle, kind: String, email: String, host: String, port: u16, username: String, password: String) -> Result<Account, String> {
    let email = email.trim().to_string();
    if !valid_email(&email) {
        return Err(t("Enter the whole email address."));
    }
    let (host, port, username) = if kind == "icloud" {
        (ICLOUD_HOST.to_string(), 993, email.clone())
    } else if kind == "imap" {
        let host = host.trim().to_ascii_lowercase();
        if !valid_host(&host) || port == 0 {
            return Err(t("Enter the IMAP server, like imap.example.com, and its port (usually 993)."));
        }
        let user = username.trim();
        (host, port, if user.is_empty() { email.clone() } else { user.to_string() })
    } else {
        return Err("unknown account kind".into());
    };
    let password = password.trim().to_string();
    if password.is_empty() {
        return Err(t("Enter the password (an app-specific password for iCloud)."));
    }
    // It has to work before anything is saved.
    let mut s = imap::Session::connect(&host, port).await?;
    s.login(&username, &password).await?;
    s.logout().await;
    let account = insert(&*db::conn()?, &kind, &email, &host, port, &username)?;
    if let Err(e) = crate::secrets::set_internal(&secret_key(account.id), &password) {
        let _ = db::conn().map(|c| c.execute("DELETE FROM mail_accounts WHERE id = ?1", [account.id]));
        return Err(e);
    }
    crate::log::line(format!("mail: account {} connected ({kind})", account.id));
    tauri::async_runtime::spawn(async move {
        refresh_all(Some(&app)).await;
    });
    Ok(account)
}

/// Gmail ("gmail") or Outlook ("outlook"): the provider's sign-in in the browser.
#[tauri::command]
pub async fn mail_connect_oauth(app: AppHandle, provider: String) -> Result<Account, String> {
    let p = self::provider(&provider)?;
    let tokens = oauth::authorize(p).await?;
    let refresh = tokens.refresh.clone().ok_or_else(|| t("The account didn't allow Groovy to stay signed in. Try again."))?;
    let email = match p {
        Provider::Google => api::gmail_email(&tokens.access).await?,
        Provider::Microsoft => api::graph_email(&tokens.access).await?,
    };
    let account = insert(&*db::conn()?, &provider, &email, "", 0, &email)?;
    if let Err(e) = crate::secrets::set_internal(&secret_key(account.id), &refresh) {
        let _ = db::conn().map(|c| c.execute("DELETE FROM mail_accounts WHERE id = ?1", [account.id]));
        return Err(e);
    }
    TOKENS.lock().unwrap().get_or_insert_with(HashMap::new).insert(account.id, tokens);
    crate::log::line(format!("mail: account {} connected ({provider})", account.id));
    tauri::async_runtime::spawn(async move {
        refresh_all(Some(&app)).await;
    });
    Ok(account)
}

/// Deletes the account, its password or token, and what was read from it.
#[tauri::command]
pub fn mail_disconnect(app: AppHandle, id: i64) -> Result<(), String> {
    crate::secrets::set_internal(&secret_key(id), "")?;
    db::conn()?.execute("DELETE FROM mail_accounts WHERE id = ?1", [id]).map_err(sql)?;
    if let Some(m) = TOKENS.lock().unwrap().as_mut() {
        m.remove(&id);
    }
    let left = {
        let mut cache = INBOXES.lock().unwrap();
        cache.retain(|i| i.account.id != id);
        cache.clone()
    };
    let _ = app.emit("mail-inboxes", &left);
    crate::log::line(format!("mail: account {id} disconnected"));
    Ok(())
}

fn find(account: i64) -> Result<Account, String> {
    accounts(&*db::conn()?)?.into_iter().find(|a| a.id == account).ok_or_else(|| t("That account isn't connected any more."))
}

/// "read" (mark read) or "archive".
#[tauri::command]
pub async fn mail_act(app: AppHandle, account: i64, id: String, action: String) -> Result<(), String> {
    let a = find(account)?;
    match (a.kind.as_str(), action.as_str()) {
        ("gmail", "read") => api::gmail_modify(&access_token(&a).await?, &id, "UNREAD").await?,
        ("gmail", "archive") => api::gmail_modify(&access_token(&a).await?, &id, "INBOX").await?,
        ("outlook", "read") => api::graph_mark_read(&access_token(&a).await?, &id).await?,
        ("outlook", "archive") => api::graph_archive(&access_token(&a).await?, &id).await?,
        (_, "read" | "archive") => {
            let uid: u32 = id.parse().map_err(|_| "bad message id".to_string())?;
            let mut s = imap_session(&a).await?;
            s.inbox().await?;
            if action == "read" {
                s.mark_read(uid).await?;
            } else {
                s.archive(uid).await?;
            }
            s.logout().await;
        }
        _ => return Err("unknown action".into()),
    }
    let changed = {
        let mut cache = INBOXES.lock().unwrap();
        if let Some(inbox) = cache.iter_mut().find(|i| i.account.id == account) {
            let was_unread = inbox.messages.iter().any(|m| m.id == id && m.unread);
            if action == "archive" {
                inbox.messages.retain(|m| m.id != id);
            } else if let Some(m) = inbox.messages.iter_mut().find(|m| m.id == id) {
                m.unread = false;
            }
            if was_unread {
                inbox.unread = inbox.unread.saturating_sub(1);
            }
        }
        cache.clone()
    };
    let _ = app.emit("mail-inboxes", &changed);
    Ok(())
}

/// One message opened in the island's Mail tile (round 3D item 8).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub from: String,
    pub subject: String,
    pub text: String,
}

/// Reads one message's text, for the user who opened it. It isn't kept or
/// logged, and the message isn't marked read by reading it here.
#[tauri::command]
pub async fn mail_read(account: i64, id: String) -> Result<Opened, String> {
    let a = find(account)?;
    let (from, subject, text) = match a.kind.as_str() {
        "gmail" => api::gmail_read(&access_token(&a).await?, &id).await?,
        "outlook" => api::graph_read(&access_token(&a).await?, &id).await?,
        _ => {
            let uid: u32 = id.parse().map_err(|_| "bad message id".to_string())?;
            let mut s = imap_session(&a).await?;
            s.inbox().await?;
            let raw = s.whole(uid).await;
            s.logout().await;
            let raw = raw?;
            let headers = raw.split("\r\n\r\n").next().unwrap_or("");
            (
                text::sender_name(&text::header(headers, "From").unwrap_or_default()),
                text::header(headers, "Subject").unwrap_or_default(),
                body::readable(raw.as_bytes()),
            )
        }
    };
    Ok(Opened { from, subject, text })
}

/// Sends the reply the user wrote (and pressed Send on), from the account it came to.
#[tauri::command]
pub async fn mail_reply(account: i64, id: String, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(t("Write the reply first."));
    }
    let a = find(account)?;
    match a.kind.as_str() {
        "gmail" => api::gmail_reply(&access_token(&a).await?, &a.email, &id, &text).await?,
        "outlook" => api::graph_reply(&access_token(&a).await?, &id, &text).await?,
        _ => {
            let uid: u32 = id.parse().map_err(|_| "bad message id".to_string())?;
            let mut s = imap_session(&a).await?;
            s.inbox().await?;
            let headers = s.headers(uid).await;
            s.logout().await;
            let original = body::original(&headers?);
            if original.to.is_empty() {
                return Err(t("That message has no address to answer."));
            }
            let message = body::reply_message(&a.email, &original, &text, &body::date_now(), &body::new_id(&a.email));
            let password = crate::secrets::get_internal(&secret_key(a.id)).ok_or_else(|| t("The account needs to be connected again."))?;
            let (host, port) = smtp::server_for(&a.kind, &a.host);
            smtp::send(&host, port, &a.username, &password, &a.email, &original.to, &message).await?;
        }
    }
    crate::log::line(format!("mail: account {account} sent a reply"));
    Ok(())
}

/// "Groovy, polish it": the reply rewritten by the brain doing the writing job
/// (Brains → Model per job), in the language it was written in.
#[tauri::command]
pub async fn mail_polish(text: String) -> Result<String, String> {
    let draft = text.trim();
    if draft.is_empty() {
        return Err(t("Write the reply first."));
    }
    let system = "You polish a short email reply. Keep its meaning, its language and its tone; fix grammar and spelling; make it clear and friendly. Keep it about as long. Answer with the reply text only: no subject, no greeting or signature that wasn't there, no notes.";
    let out = crate::brain_ai::complete(system, draft, 600, Duration::from_secs(30)).await?;
    let out = out.trim().trim_matches('"').trim().to_string();
    Ok(if out.is_empty() { draft.to_string() } else { out })
}

/// Opens it in the account's web mail (the browser chosen in Settings → Browser).
#[tauri::command]
pub fn mail_open_message(account: i64, id: String) -> Result<(), String> {
    let link = INBOXES
        .lock()
        .unwrap()
        .iter()
        .flat_map(|i| i.messages.iter())
        .find(|m| m.account == account && m.id == id)
        .and_then(|m| m.link.clone());
    match link {
        Some(l) if l.starts_with("https://") => crate::browser::open(&l).map(|_| ()),
        _ => Err(t("Open it in your mail app.")),
    }
}

#[derive(Serialize)]
pub struct OauthReady {
    google: bool,
    microsoft: bool,
}

/// Whether Gmail and Outlook can sign in: an app registration is set.
#[tauri::command]
pub fn mail_oauth_ready() -> OauthReady {
    OauthReady { google: Provider::Google.client_id().is_some(), microsoft: Provider::Microsoft.client_id().is_some() }
}

#[tauri::command]
pub fn mail_oauth_configure(provider: String, client_id: String, client_secret: String) -> Result<(), String> {
    self::provider(&provider)?.configure(&client_id, &client_secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbox(id: i64, unread: u32, msgs: &[(&str, &str, bool, i64)], error: Option<&str>) -> Inbox {
        Inbox {
            account: Account { id, kind: "imap".into(), email: "me@x.com".into(), host: "imap.x.com".into(), port: 993, username: "me".into() },
            unread,
            messages: msgs
                .iter()
                .map(|(mid, from, unread, at)| Message { account: id, id: mid.to_string(), from: from.to_string(), subject: "s".into(), snippet: String::new(), at: *at, unread: *unread, link: None })
                .collect(),
            error: error.map(str::to_string),
            checked_at: 0,
            can_modify: true,
        }
    }

    #[test]
    fn accounts_are_kept_in_the_database_without_secrets() {
        let conn = db::open_in_memory().unwrap();
        let a = insert(&conn, "icloud", "me@icloud.com", ICLOUD_HOST, 993, "me@icloud.com").unwrap();
        assert_eq!(accounts(&conn).unwrap(), vec![a.clone()]);
        let cols: Vec<String> = conn.prepare("SELECT name FROM pragma_table_info('mail_accounts')").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert!(!cols.iter().any(|c| c.contains("pass") || c.contains("token")), "{cols:?}");
        assert_eq!(secret_key(a.id), format!("mail-account-{}", a.id));
        assert!(crate::secrets::is_internal(&secret_key(a.id)));
        assert!(!crate::secrets::is_internal("mail-account-1;x"));
    }

    #[test]
    fn only_mail_that_arrived_since_the_last_check_is_news() {
        let before = [inbox(1, 1, &[("1", "Anna", true, 10)], None)];
        let now = [inbox(1, 2, &[("2", "Bob", true, 20), ("1", "Anna", true, 10)], None), inbox(2, 5, &[("9", "Old", true, 5)], None)];
        let news = new_unread(&before, &now);
        assert_eq!(news.iter().map(|m| m.from.as_str()).collect::<Vec<_>>(), ["Bob"], "account 2's first check is its backlog");
    }

    #[test]
    fn any_new_emails_reads_the_count_and_the_top_senders() {
        assert_eq!(summary_of(&[inbox(1, 0, &[], None)]), "No new emails.");
        let two = [inbox(1, 3, &[("1", "Anna", true, 30), ("2", "Anna", true, 20), ("3", "GitHub", true, 10), ("4", "Read", false, 40)], None), inbox(2, 1, &[("5", "Stripe", true, 15)], None)];
        assert_eq!(summary_of(&two), "You have 4 unread emails, from Anna, Stripe, GitHub.");
        assert_eq!(summary_of(&[inbox(1, 1, &[], None)]), "You have 1 unread email.");
    }

    #[test]
    fn servers_and_web_mail() {
        assert!(valid_host("imap.mail.me.com") && valid_host("mail.example.co.uk"));
        assert!(!valid_host("imap example.com") && !valid_host("localhost") && !valid_host("a..b") && !valid_host("x.com\r\n"));
        assert_eq!(webmail("icloud", ICLOUD_HOST), Some("https://www.icloud.com/mail/"));
        assert_eq!(webmail("imap", "outlook.office365.com"), Some("https://outlook.live.com/mail/"));
        assert_eq!(webmail("imap", "imap.fastmail.com"), None);
        assert!(valid_email("me@icloud.com") && !valid_email("me") && !valid_email("a b@x.com"));
    }
}
