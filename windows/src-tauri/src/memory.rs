// What Groovy remembers, in the local database (db.rs), in two separate stores:
//
// - Long-term memory: facts, preferences, project and task context the user
//   asked Groovy to keep ("remember that…", or added in Settings → Memory).
//   The newest entries (pinned first) go into the system prompt, within a
//   budget, so the assistant's answers actually use them.
// - Chat history: each conversation and its turns, kept for the retention the
//   user picked (0 = keep forever).
//
// Clearing one never touches the other. Either can be turned off; with memory
// off nothing is added to the prompt and nothing new is remembered, with
// history off no turn is written. Everything stays on this computer: an
// export is a JSON file the user saves themselves.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db;

pub const KINDS: &[&str] = &["preference", "fact", "project", "task"];
const MAX_CONTENT: usize = 2000;
/// How much of the system prompt memories may take, in characters.
pub const PROMPT_BUDGET: usize = 1500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: i64,
    pub kind: String,
    pub content: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String,
    pub content: String,
    pub provider: String,
    pub created_at: i64,
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Things that look like a credential are never stored: a memory goes into
/// every prompt, possibly to an online provider.
pub fn looks_sensitive(text: &str) -> bool {
    let lower = text.to_lowercase();
    const WORDS: &[&str] = &["password", "passwd", "api key", "api_key", "secret key", "private key", "pin code"];
    const PREFIXES: &[&str] = &["sk-", "sk_live_", "ghp_", "gho_", "github_pat_", "xox", "aiza", "-----begin"];
    WORDS.iter().any(|w| lower.contains(w))
        || lower.split_whitespace().any(|tok| PREFIXES.iter().any(|p| tok.starts_with(p) && tok.len() > 12))
}

fn check(kind: &str, content: &str) -> Result<String, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("Unknown memory kind: {kind}"));
    }
    let content = content.trim();
    if content.is_empty() {
        return Err(crate::i18n::t("A memory cannot be empty."));
    }
    if content.chars().count() > MAX_CONTENT {
        return Err(format!("A memory is at most {MAX_CONTENT} characters."));
    }
    if looks_sensitive(content) {
        return Err(crate::i18n::t("That looks like a password or key, so Groovy won't remember it."));
    }
    Ok(content.to_string())
}

fn row_memory(r: &rusqlite::Row) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: r.get(0)?,
        kind: r.get(1)?,
        content: r.get(2)?,
        pinned: r.get::<_, i64>(3)? != 0,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

const MEMORY_COLS: &str = "id, kind, content, pinned, created_at, updated_at";

pub fn list(conn: &Connection, kind: Option<&str>) -> Result<Vec<Memory>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {MEMORY_COLS} FROM memories WHERE ?1 IS NULL OR kind = ?1 ORDER BY pinned DESC, updated_at DESC, id DESC"
        ))
        .map_err(sql)?;
    let rows = stmt.query_map(params![kind], row_memory).map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn add(conn: &Connection, kind: &str, content: &str) -> Result<Memory, String> {
    let content = check(kind, content)?;
    // The same thing remembered twice is one memory, freshened.
    if let Some(id) = conn
        .query_row("SELECT id FROM memories WHERE kind = ?1 AND content = ?2", params![kind, content], |r| r.get(0))
        .optional()
        .map_err(sql)?
    {
        conn.execute("UPDATE memories SET updated_at = ?2 WHERE id = ?1", params![id, db::now()]).map_err(sql)?;
        return get(conn, id);
    }
    let now = db::now();
    conn.execute(
        "INSERT INTO memories (kind, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![kind, content, now],
    )
    .map_err(sql)?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Memory, String> {
    conn.query_row(&format!("SELECT {MEMORY_COLS} FROM memories WHERE id = ?1"), [id], row_memory)
        .map_err(sql)
}

pub fn update(conn: &Connection, id: i64, kind: &str, content: &str, pinned: bool) -> Result<Memory, String> {
    let content = check(kind, content)?;
    let n = conn
        .execute(
            "UPDATE memories SET kind = ?2, content = ?3, pinned = ?4, updated_at = ?5 WHERE id = ?1",
            params![id, kind, content, pinned as i64, db::now()],
        )
        .map_err(sql)?;
    if n == 0 {
        return Err("No such memory.".into());
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM memories WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

/// Clears long-term memory (one kind, or all). Chat history is not touched.
pub fn clear(conn: &Connection, kind: Option<&str>) -> Result<usize, String> {
    conn.execute("DELETE FROM memories WHERE ?1 IS NULL OR kind = ?1", params![kind]).map_err(sql)
}

/// The memories for the system prompt: pinned first, then the newest, as long
/// as they fit in `budget` characters. Empty when there are none.
pub fn prompt_section(conn: &Connection, budget: usize) -> String {
    let Ok(all) = list(conn, None) else { return String::new() };
    let mut lines = Vec::new();
    let mut used = 0;
    for m in all {
        let line = format!("- ({}) {}", m.kind, m.content);
        let len = line.chars().count() + 1;
        if used + len > budget {
            continue;
        }
        used += len;
        lines.push(line);
    }
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "What you remember about the user (they asked you to; use it when relevant, never recite it unprompted):\n{}",
        lines.join("\n")
    )
}

/// "Remember that I prefer tabs" → Some(("preference", "I prefer tabs")).
/// Only an explicit request at the start of the message counts.
pub fn remember_request(query: &str) -> Option<(&'static str, String)> {
    let q = query.trim();
    let lower = q.to_lowercase();
    const LEADS: &[&str] = &["remember that ", "remember: ", "please remember that ", "remember i ", "remember my "];
    let lead = LEADS.iter().find(|l| lower.starts_with(*l))?;
    // "remember i …"/"remember my …" keep their first word.
    let keep_word = lead.ends_with("i ") || lead.ends_with("my ");
    let start = if keep_word { lead.len() - lead.split_whitespace().last().unwrap().len() - 1 } else { lead.len() };
    let fact = q[start..].trim().trim_end_matches(['.', '!']).trim();
    if fact.is_empty() {
        return None;
    }
    let f = fact.to_lowercase();
    let kind = if ["i prefer", "i like", "i love", "i hate", "i don't like", "i dislike", "call me", "my favourite", "my favorite"]
        .iter()
        .any(|p| f.starts_with(p))
    {
        "preference"
    } else {
        "fact"
    };
    Some((kind, fact.to_string()))
}

// ── Chat history ──────────────────────────────────────────────────────────────

fn title_of(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let mut t: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        t.push('…');
    }
    if t.is_empty() { "Conversation".into() } else { t }
}

/// Appends one finished turn; starts a conversation when `conversation` is None
/// or no longer exists. Returns the conversation's id.
pub fn record_turn(
    conn: &Connection,
    conversation: Option<i64>,
    user: &str,
    assistant: &str,
    provider: &str,
) -> Result<i64, String> {
    let now = db::now();
    let existing = match conversation {
        Some(id) => conn
            .query_row("SELECT id FROM conversations WHERE id = ?1", [id], |r| r.get::<_, i64>(0))
            .optional()
            .map_err(sql)?,
        None => None,
    };
    let id = match existing {
        Some(id) => {
            conn.execute("UPDATE conversations SET updated_at = ?2 WHERE id = ?1", params![id, now]).map_err(sql)?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO conversations (title, started_at, updated_at) VALUES (?1, ?2, ?2)",
                params![title_of(user), now],
            )
            .map_err(sql)?;
            conn.last_insert_rowid()
        }
    };
    for (role, content) in [("user", user), ("assistant", assistant)] {
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, provider, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, role, content, provider, now],
        )
        .map_err(sql)?;
    }
    Ok(id)
}

pub fn conversations(conn: &Connection) -> Result<Vec<Conversation>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.title, c.started_at, c.updated_at, (SELECT count(*) FROM messages m WHERE m.conversation_id = c.id)
             FROM conversations c ORDER BY c.updated_at DESC, c.id DESC",
        )
        .map_err(sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Conversation { id: r.get(0)?, title: r.get(1)?, started_at: r.get(2)?, updated_at: r.get(3)?, messages: r.get(4)? })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn messages(conn: &Connection, conversation: i64) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare("SELECT role, content, provider, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id")
        .map_err(sql)?;
    let rows = stmt
        .query_map([conversation], |r| {
            Ok(Message { role: r.get(0)?, content: r.get(1)?, provider: r.get(2)?, created_at: r.get(3)? })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn delete_conversation(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM conversations WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

/// Clears chat history only. Long-term memory is not touched.
pub fn clear_conversations(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM conversations", []).map_err(sql)
}

/// Drops conversations untouched for more than `days` days; 0 keeps everything.
pub fn prune(conn: &Connection, days: u32, now: i64) -> Result<usize, String> {
    if days == 0 {
        return Ok(0);
    }
    let cutoff = now - i64::from(days) * 86_400;
    conn.execute("DELETE FROM conversations WHERE updated_at < ?1", [cutoff]).map_err(sql)
}

/// Everything in the database, as one JSON document the user can keep.
pub fn export(conn: &Connection) -> Result<serde_json::Value, String> {
    let convs = conversations(conn)?;
    let mut with_messages = Vec::new();
    for c in convs {
        let msgs = messages(conn, c.id)?;
        with_messages.push(serde_json::json!({ "conversation": c, "messages": msgs }));
    }
    Ok(serde_json::json!({
        "app": "Groovy",
        "version": env!("CARGO_PKG_VERSION"),
        "exportedAt": db::now(),
        "memories": list(conn, None)?,
        "conversations": with_messages,
        "tasks": crate::tasks::list(conn, true)?,
    }))
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_list(kind: Option<String>) -> Result<Vec<Memory>, String> {
    list(&*db::conn()?, kind.as_deref())
}

#[tauri::command]
pub fn memory_add(kind: String, content: String) -> Result<Memory, String> {
    add(&*db::conn()?, &kind, &content)
}

#[tauri::command]
pub fn memory_update(id: i64, kind: String, content: String, pinned: bool) -> Result<Memory, String> {
    update(&*db::conn()?, id, &kind, &content, pinned)
}

#[tauri::command]
pub fn memory_delete(id: i64) -> Result<(), String> {
    delete(&*db::conn()?, id)
}

#[tauri::command]
pub fn memory_clear(kind: Option<String>) -> Result<usize, String> {
    clear(&*db::conn()?, kind.as_deref())
}

#[tauri::command]
pub fn history_list() -> Result<Vec<Conversation>, String> {
    conversations(&*db::conn()?)
}

#[tauri::command]
pub fn history_messages(id: i64) -> Result<Vec<Message>, String> {
    messages(&*db::conn()?, id)
}

#[tauri::command]
pub fn history_delete(id: i64) -> Result<(), String> {
    delete_conversation(&*db::conn()?, id)
}

#[tauri::command]
pub fn history_clear() -> Result<usize, String> {
    clear_conversations(&*db::conn()?)
}

/// Writes the export next to the log, in an `exports` folder, shows it, and
/// returns its path.
#[tauri::command]
pub fn memory_export() -> Result<String, String> {
    let doc = export(&*db::conn()?)?;
    let dir = crate::settings::local_dir().join("exports");
    crate::platform::ensure_private_dir(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("groovy-export-{}.json", db::now()));
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    let path = file.to_string_lossy().to_string();
    crate::platform::reveal_folder(&dir.to_string_lossy());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        db::open_in_memory().unwrap()
    }

    #[test]
    fn add_list_update_delete() {
        let c = conn();
        let a = add(&c, "fact", "  My dog is called Biscuit ").unwrap();
        assert_eq!(a.content, "My dog is called Biscuit");
        let b = add(&c, "preference", "I prefer short answers").unwrap();
        assert_eq!(list(&c, None).unwrap().len(), 2);
        assert_eq!(list(&c, Some("fact")).unwrap(), vec![a.clone()]);
        let a2 = update(&c, a.id, "fact", "My dog is called Biscuit, a beagle", true).unwrap();
        assert!(a2.pinned);
        assert_eq!(list(&c, None).unwrap()[0].id, a.id, "pinned first");
        delete(&c, b.id).unwrap();
        assert_eq!(list(&c, None).unwrap().len(), 1);
        assert!(update(&c, 999, "fact", "x", false).is_err());
    }

    #[test]
    fn duplicates_are_merged() {
        let c = conn();
        let a = add(&c, "fact", "x is y").unwrap();
        let b = add(&c, "fact", "x is y").unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(list(&c, None).unwrap().len(), 1);
    }

    #[test]
    fn rejects_bad_input_and_secrets() {
        let c = conn();
        assert!(add(&c, "nope", "x").is_err());
        assert!(add(&c, "fact", "   ").is_err());
        assert!(add(&c, "fact", &"a".repeat(MAX_CONTENT + 1)).is_err());
        assert!(add(&c, "fact", "my password is hunter2").is_err());
        assert!(add(&c, "fact", "key sk-abcdefghijklmnop1234").is_err());
        assert!(add(&c, "fact", "ghp_0123456789abcdefABCDEF").is_err());
        assert!(add(&c, "fact", "I like sk-ate boarding").is_ok(), "short tokens are fine");
    }

    #[test]
    fn prompt_section_respects_the_budget_and_order() {
        let c = conn();
        assert_eq!(prompt_section(&c, PROMPT_BUDGET), "");
        add(&c, "fact", "first").unwrap();
        let p = add(&c, "preference", "pinned one").unwrap();
        update(&c, p.id, "preference", "pinned one", true).unwrap();
        add(&c, "fact", &"z".repeat(500)).unwrap();
        let s = prompt_section(&c, 100);
        assert!(s.contains("- (preference) pinned one"));
        assert!(s.contains("- (fact) first"));
        assert!(!s.contains("zzzz"), "too long for the budget");
        let pinned_at = s.find("pinned one").unwrap();
        assert!(pinned_at < s.find("first").unwrap());
    }

    #[test]
    fn remember_requests() {
        assert_eq!(remember_request("Remember that I prefer dark mode."), Some(("preference", "I prefer dark mode".into())));
        assert_eq!(remember_request("remember: the build server is ci-01"), Some(("fact", "the build server is ci-01".into())));
        assert_eq!(remember_request("Remember my sister is Ana"), Some(("fact", "my sister is Ana".into())));
        assert_eq!(remember_request("remember I like jazz"), Some(("preference", "I like jazz".into())));
        assert_eq!(remember_request("Do you remember that film?"), None);
        assert_eq!(remember_request("remember that "), None);
    }

    #[test]
    fn history_is_separate_from_memory() {
        let c = conn();
        add(&c, "fact", "keep me").unwrap();
        let id = record_turn(&c, None, "Hello there\nsecond line", "Hi!", "ollama").unwrap();
        let same = record_turn(&c, Some(id), "And again", "Sure", "ollama").unwrap();
        assert_eq!(id, same);
        let convs = conversations(&c).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].title, "Hello there");
        assert_eq!(convs[0].messages, 4);
        assert_eq!(messages(&c, id).unwrap()[1].content, "Hi!");
        // A conversation id that was deleted starts a new one.
        delete_conversation(&c, id).unwrap();
        assert!(messages(&c, id).unwrap().is_empty(), "messages cascade");
        let fresh = record_turn(&c, Some(id), "new", "chat", "x").unwrap();
        assert_ne!(fresh, id);
        assert_eq!(clear_conversations(&c).unwrap(), 1);
        assert_eq!(list(&c, None).unwrap().len(), 1, "clearing chats keeps memories");
        add(&c, "fact", "another").unwrap();
        record_turn(&c, None, "a", "b", "x").unwrap();
        clear(&c, None).unwrap();
        assert_eq!(conversations(&c).unwrap().len(), 1, "clearing memories keeps chats");
    }

    #[test]
    fn retention_prunes_old_conversations_only() {
        let c = conn();
        let old = record_turn(&c, None, "old", "x", "p").unwrap();
        c.execute("UPDATE conversations SET updated_at = 0 WHERE id = ?1", [old]).unwrap();
        record_turn(&c, None, "new", "x", "p").unwrap();
        assert_eq!(prune(&c, 0, db::now()).unwrap(), 0, "0 keeps forever");
        assert_eq!(prune(&c, 30, db::now()).unwrap(), 1);
        assert_eq!(conversations(&c).unwrap()[0].title, "new");
    }

    #[test]
    fn export_has_every_store() {
        let c = conn();
        add(&c, "project", "Groovy uses Tauri").unwrap();
        record_turn(&c, None, "q", "a", "p").unwrap();
        crate::tasks::add(&c, "water plants", None).unwrap();
        let doc = export(&c).unwrap();
        assert_eq!(doc["memories"].as_array().unwrap().len(), 1);
        assert_eq!(doc["conversations"][0]["messages"].as_array().unwrap().len(), 2);
        assert_eq!(doc["tasks"].as_array().unwrap().len(), 1);
    }
}
