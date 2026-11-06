//! Memories as linked markdown notes (round 3I). One `.md` file per memory is
//! the source of truth; the `memories` table is the index, rebuilt from the
//! files. Pure rusqlite + std, so it is easy to test on its own.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{params, Connection, OptionalExtension};

/// Migration 2: where each note lives, when a fact ends, the search index and the links.
pub const SCHEMA_V2: &str = "
    ALTER TABLE memories ADD COLUMN file TEXT;
    ALTER TABLE memories ADD COLUMN file_mtime INTEGER;
    ALTER TABLE memories ADD COLUMN valid_until INTEGER;
    ALTER TABLE memories ADD COLUMN replaced_by INTEGER;
    CREATE VIRTUAL TABLE memory_fts USING fts5(title, body, tokenize = 'porter unicode61');
    CREATE TABLE memory_links (from_id INTEGER NOT NULL, to_id INTEGER NOT NULL, PRIMARY KEY (from_id, to_id));";

/// Set whenever a note changes; the next recall rebuilds the (small) index.
static DIRTY: AtomicBool = AtomicBool::new(true);
pub fn mark_dirty() {
    DIRTY.store(true, Ordering::Relaxed);
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

/// A safe file stem: lowercase letters, digits and dashes only, never `..` or a separator.
pub fn slug(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    let mut out = String::new();
    for c in first.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() && c.is_ascii() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "note".into() } else { out }
}

/// Whether `file` is a plain file name inside the folder (no separators, no `..`).
pub fn safe_name(file: &str) -> bool {
    !file.is_empty() && !file.contains(['/', '\\']) && !file.contains("..") && file.ends_with(".md")
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Note {
    pub id: Option<i64>,
    pub kind: String,
    pub pinned: bool,
    pub created: i64,
    pub updated: i64,
    pub valid_until: Option<i64>,
    pub replaced_by: Option<i64>,
    pub body: String,
}

pub fn render(n: &Note) -> String {
    let mut s = String::from("---\n");
    if let Some(id) = n.id {
        s.push_str(&format!("id: {id}\n"));
    }
    s.push_str(&format!("kind: {}\npinned: {}\ncreated: {}\nupdated: {}\n", n.kind, n.pinned, n.created, n.updated));
    if let Some(v) = n.valid_until {
        s.push_str(&format!("valid_until: {v}\n"));
    }
    if let Some(v) = n.replaced_by {
        s.push_str(&format!("replaced_by: {v}\n"));
    }
    s.push_str("---\n");
    s.push_str(n.body.trim());
    s.push('\n');
    s
}

/// A note file's text. A file without frontmatter is a plain fact, so notes made in an editor import too.
pub fn parse(text: &str, now: i64) -> Note {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut n = Note { kind: "fact".into(), created: now, updated: now, ..Default::default() };
    let Some(rest) = text.strip_prefix("---\n") else {
        n.body = text.trim().to_string();
        return n;
    };
    let Some((head, body)) = rest.split_once("\n---") else {
        n.body = text.trim().to_string();
        return n;
    };
    for line in head.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim();
        match k.trim() {
            "id" => n.id = v.parse().ok(),
            "kind" => n.kind = v.to_string(),
            "pinned" => n.pinned = v == "true",
            "created" => n.created = v.parse().unwrap_or(now),
            "updated" => n.updated = v.parse().unwrap_or(now),
            "valid_until" => n.valid_until = v.parse().ok(),
            "replaced_by" => n.replaced_by = v.parse().ok(),
            _ => {}
        }
    }
    n.body = body.trim_start_matches('\n').trim().to_string();
    n
}

/// The `[[targets]]` in a note.
pub fn links_in(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("[[") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("]]") else { break };
        let t = after[..j].trim();
        if !t.is_empty() && !t.contains('\n') {
            out.push(t.trim_end_matches(".md").to_string());
        }
        rest = &after[j + 2..];
    }
    out
}

/// Writes the file next to a temp name, then renames it over.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn mtime(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(m.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

struct Row {
    id: i64,
    kind: String,
    content: String,
    pinned: bool,
    created: i64,
    updated: i64,
    file: Option<String>,
    file_mtime: Option<i64>,
    valid_until: Option<i64>,
    replaced_by: Option<i64>,
}

fn row(conn: &Connection, id: i64) -> Result<Option<Row>, String> {
    conn.query_row(
        "SELECT id, kind, content, pinned, created_at, updated_at, file, file_mtime, valid_until, replaced_by FROM memories WHERE id = ?1",
        [id],
        |r| {
            Ok(Row {
                id: r.get(0)?,
                kind: r.get(1)?,
                content: r.get(2)?,
                pinned: r.get::<_, i64>(3)? != 0,
                created: r.get(4)?,
                updated: r.get(5)?,
                file: r.get(6)?,
                file_mtime: r.get(7)?,
                valid_until: r.get(8)?,
                replaced_by: r.get(9)?,
            })
        },
    )
    .optional()
    .map_err(sql)
}

fn free_stem(conn: &Connection, dir: &Path, base: &str, id: i64) -> String {
    let taken = |stem: &str| {
        let name = format!("{stem}.md");
        let other: Option<i64> = conn
            .query_row("SELECT id FROM memories WHERE file = ?1 AND id != ?2", params![name, id], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        other.is_some() || (dir.join(&name).exists() && conn.query_row("SELECT 1 FROM memories WHERE file = ?1 AND id = ?2", params![name, id], |_| Ok(())).optional().ok().flatten().is_none())
    };
    if !taken(base) { base.to_string() } else { format!("{base}-{id}") }
}

/// Writes (or rewrites) the note's file after a change made in Groovy. If the
/// file was edited elsewhere since Groovy last read it, that edit is taken
/// instead and nothing is overwritten.
pub fn write_note(conn: &Connection, dir: &Path, id: i64) -> Result<(), String> {
    let Some(r) = row(conn, id)? else { return Ok(()) };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let name = match &r.file {
        Some(f) if safe_name(f) => f.clone(),
        _ => format!("{}.md", free_stem(conn, dir, &slug(&r.content), id)),
    };
    let path = dir.join(&name);
    if r.file.is_some() && path.exists() && mtime(&path) != r.file_mtime {
        return ingest_file(conn, dir, &name, now_ms() / 1000).map(|_| ());
    }
    let note = Note {
        id: Some(r.id),
        kind: r.kind,
        pinned: r.pinned,
        created: r.created,
        updated: r.updated,
        valid_until: r.valid_until,
        replaced_by: r.replaced_by,
        body: r.content,
    };
    write_atomic(&path, &render(&note)).map_err(|e| e.to_string())?;
    conn.execute("UPDATE memories SET file = ?2, file_mtime = ?3 WHERE id = ?1", params![id, name, mtime(&path)]).map_err(sql)?;
    mark_dirty();
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Deletes a note's file (the row and its index rows are the caller's).
pub fn remove_file(conn: &Connection, dir: &Path, id: i64) {
    if let Ok(Some(Row { file: Some(f), .. })) = row(conn, id) {
        if safe_name(&f) {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
    mark_dirty();
}

/// Reads one file into the index. The newer of file and row wins.
pub fn ingest_file(conn: &Connection, dir: &Path, name: &str, now: i64) -> Result<Option<i64>, String> {
    if !safe_name(name) {
        return Ok(None);
    }
    let path = dir.join(name);
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(None) };
    let mt = mtime(&path);
    let mut n = parse(&text, now);
    if n.body.is_empty() || n.body.chars().count() > 2000 || sensitive(&n.body) {
        return Ok(None);
    }
    if !["preference", "fact", "project", "task"].contains(&n.kind.as_str()) {
        n.kind = "fact".into();
    }
    let existing = match n.id {
        Some(id) => row(conn, id)?,
        None => conn.query_row("SELECT id FROM memories WHERE file = ?1", [name], |r| r.get::<_, i64>(0)).optional().map_err(sql)?.map(|id| row(conn, id)).transpose()?.flatten(),
    };
    let id = match existing {
        Some(r) => {
            if r.file_mtime == mt && r.file.as_deref() == Some(name) {
                return Ok(Some(r.id));
            }
            conn.execute(
                "UPDATE memories SET kind = ?2, content = ?3, pinned = ?4, updated_at = ?5, file = ?6, file_mtime = ?7, valid_until = ?8, replaced_by = ?9 WHERE id = ?1",
                params![r.id, n.kind, n.body, n.pinned as i64, (mt.unwrap_or(0) / 1000).max(n.updated), name, mt, n.valid_until, n.replaced_by],
            )
            .map_err(sql)?;
            r.id
        }
        None => {
            conn.execute(
                "INSERT INTO memories (kind, content, pinned, created_at, updated_at, file, file_mtime, valid_until, replaced_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![n.kind, n.body, n.pinned as i64, n.created, n.updated.max(now), name, mt, n.valid_until, n.replaced_by],
            )
            .map_err(sql)?;
            let id = conn.last_insert_rowid();
            // Give a note made elsewhere its id, so the file and the index agree.
            let _ = write_note_forced(conn, dir, id);
            id
        }
    };
    mark_dirty();
    Ok(Some(id))
}

fn write_note_forced(conn: &Connection, dir: &Path, id: i64) -> Result<(), String> {
    conn.execute("UPDATE memories SET file_mtime = NULL WHERE id = ?1", [id]).map_err(sql)?;
    let Some(r) = row(conn, id)? else { return Ok(()) };
    let name = r.file.clone().unwrap_or_default();
    let note = Note { id: Some(r.id), kind: r.kind, pinned: r.pinned, created: r.created, updated: r.updated, valid_until: r.valid_until, replaced_by: r.replaced_by, body: r.content };
    let path = dir.join(&name);
    write_atomic(&path, &render(&note)).map_err(|e| e.to_string())?;
    conn.execute("UPDATE memories SET file_mtime = ?2 WHERE id = ?1", params![id, mtime(&path)]).map_err(sql)?;
    Ok(())
}

/// Brings the index and the folder into step: first run writes a file for each
/// memory that has none (no loss), then every file is read, and a row whose
/// file has gone is dropped.
pub fn sync(conn: &Connection, dir: &Path, now: i64) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut present = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if safe_name(&name) {
                present.insert(name);
            }
        }
    }
    let gone: Vec<i64> = {
        let mut st = conn.prepare("SELECT id, file FROM memories WHERE file IS NOT NULL").map_err(sql)?;
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(sql)?;
        rows.flatten().filter(|(_, f)| !present.contains(f)).map(|(id, _)| id).collect()
    };
    for id in gone {
        conn.execute("DELETE FROM memories WHERE id = ?1", [id]).map_err(sql)?;
    }
    let loose: Vec<i64> = {
        let mut st = conn.prepare("SELECT id FROM memories WHERE file IS NULL").map_err(sql)?;
        let rows = st.query_map([], |r| r.get(0)).map_err(sql)?;
        rows.flatten().collect()
    };
    for id in loose {
        write_note(conn, dir, id)?;
    }
    let mut names: Vec<_> = present.into_iter().collect();
    names.sort();
    for name in names {
        ingest_file(conn, dir, &name, now)?;
    }
    mark_dirty();
    Ok(())
}

pub fn sensitive(text: &str) -> bool {
    let lower = text.to_lowercase();
    const WORDS: &[&str] = &["password", "passwd", "api key", "api_key", "secret key", "private key", "pin code"];
    const PREFIXES: &[&str] = &["sk-", "sk_live_", "ghp_", "gho_", "github_pat_", "xox", "aiza", "-----begin"];
    WORDS.iter().any(|w| lower.contains(w)) || lower.split_whitespace().any(|t| PREFIXES.iter().any(|p| t.starts_with(p) && t.len() > 12))
}

// ── Search index and links ───────────────────────────────────────────────────

/// Rebuilds the full-text index and the link table from the rows, when something changed.
/// They live on disk in the same database; small enough to rebuild in a blink.
pub fn ensure_index(conn: &Connection) -> Result<(), String> {
    // DIRTY is one flag for the whole process; a connection whose index doesn't hold every
    // note (another connection cleared the flag, e.g. tests running side by side) rebuilds too.
    let dirty = DIRTY.swap(false, Ordering::Relaxed);
    let behind = conn
        .query_row("SELECT (SELECT count(*) FROM memory_fts) != (SELECT count(*) FROM memories)", [], |r| r.get::<_, bool>(0))
        .unwrap_or(true);
    if !dirty && !behind {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(sql)?;
    tx.execute_batch("DELETE FROM memory_fts; DELETE FROM memory_links;").map_err(sql)?;
    let all: Vec<(i64, String, Option<String>)> = {
        let mut st = tx.prepare("SELECT id, content, file FROM memories").map_err(sql)?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(sql)?;
        rows.flatten().collect()
    };
    for (id, content, file) in &all {
        let title = file.clone().unwrap_or_default().trim_end_matches(".md").replace('-', " ");
        tx.execute("INSERT INTO memory_fts (rowid, title, body) VALUES (?1, ?2, ?3)", params![id, title, content]).map_err(sql)?;
    }
    for (id, content, _) in &all {
        for t in links_in(content) {
            let to: Option<i64> = tx
                .query_row("SELECT id FROM memories WHERE lower(file) = lower(?1)", [format!("{t}.md")], |r| r.get(0))
                .optional()
                .map_err(sql)?;
            if let Some(to) = to.filter(|to| to != id) {
                tx.execute("INSERT OR IGNORE INTO memory_links (from_id, to_id) VALUES (?1, ?2)", params![id, to]).map_err(sql)?;
            }
        }
    }
    tx.commit().map_err(sql)
}

const STOP: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "what", "when", "where", "which", "who", "how", "can", "you", "your", "are", "was",
    "were", "have", "has", "had", "does", "did", "not", "but", "from", "about", "into", "than", "then", "them", "they", "his", "her",
    "its", "our", "out", "any", "all", "get", "got", "let", "use", "please", "would", "could", "should", "tell", "give", "show", "make",
];

fn match_expr(query: &str) -> Option<String> {
    let mut seen = HashSet::new();
    let words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(&w.as_str()) && seen.insert(w.clone()))
        .take(12)
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect();
    if words.is_empty() { None } else { Some(words.join(" OR ")) }
}

/// Ids of the notes that match the words of `query`, best first.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<i64>, String> {
    ensure_index(conn)?;
    let Some(expr) = match_expr(query) else { return Ok(vec![]) };
    let mut st = conn
        .prepare("SELECT rowid FROM memory_fts WHERE memory_fts MATCH ?1 ORDER BY bm25(memory_fts) LIMIT ?2")
        .map_err(sql)?;
    let rows = st.query_map(params![expr, limit as i64], |r| r.get(0)).map_err(sql)?;
    Ok(rows.flatten().collect())
}

/// (links to, linked from) for one note.
pub fn related(conn: &Connection, id: i64) -> Result<(Vec<i64>, Vec<i64>), String> {
    ensure_index(conn)?;
    let col = |sqlq: &str| -> Result<Vec<i64>, String> {
        let mut st = conn.prepare(sqlq).map_err(sql)?;
        let rows = st.query_map([id], |r| r.get(0)).map_err(sql)?;
        Ok(rows.flatten().collect())
    };
    Ok((col("SELECT to_id FROM memory_links WHERE from_id = ?1")?, col("SELECT from_id FROM memory_links WHERE to_id = ?1")?))
}

fn asks_about_the_past(query: &str) -> bool {
    let q = query.to_lowercase();
    ["used to", "before", "previous", "previously", "past", "back then", "history", "formerly", "no longer", "old "].iter().any(|w| q.contains(w))
}

/// The ids to put in the prompt, in order: pinned, matches for the request,
/// notes linked one hop from the top matches, then the newest. Ended facts are
/// skipped unless the request is about the past.
pub fn recall_ids(conn: &Connection, query: &str, now: i64) -> Result<Vec<i64>, String> {
    let past = asks_about_the_past(query);
    let live = |id: i64| -> bool {
        past || conn
            .query_row("SELECT valid_until IS NULL OR valid_until > ?2 FROM memories WHERE id = ?1", params![id, now], |r| r.get::<_, i64>(0))
            .map(|v| v != 0)
            .unwrap_or(false)
    };
    let mut out: Vec<i64> = Vec::new();
    let push = |ids: Vec<i64>, out: &mut Vec<i64>| {
        for id in ids {
            if !out.contains(&id) && live(id) {
                out.push(id);
            }
        }
    };
    let pinned: Vec<i64> = {
        let mut st = conn.prepare("SELECT id FROM memories WHERE pinned = 1 ORDER BY updated_at DESC, id DESC").map_err(sql)?;
        let rows = st.query_map([], |r| r.get(0)).map_err(sql)?;
        rows.flatten().collect()
    };
    push(pinned, &mut out);
    let hits = search(conn, query, 8)?;
    push(hits.clone(), &mut out);
    for id in hits.iter().take(3) {
        let (to, from) = related(conn, *id)?;
        push(to, &mut out);
        push(from, &mut out);
    }
    let newest: Vec<i64> = {
        let mut st = conn.prepare("SELECT id FROM memories ORDER BY updated_at DESC, id DESC").map_err(sql)?;
        let rows = st.query_map([], |r| r.get(0)).map_err(sql)?;
        rows.flatten().collect()
    };
    push(newest, &mut out);
    Ok(out)
}

// ── Forget, and facts that end ───────────────────────────────────────────────

/// The note that best matches `what`, for "forget that".
pub fn find_best(conn: &Connection, what: &str, now: i64) -> Result<Option<(i64, String)>, String> {
    for id in search(conn, what, 5)? {
        if let Some(r) = row(conn, id)? {
            if r.valid_until.map_or(true, |v| v > now) {
                return Ok(Some((id, r.content)));
            }
        }
    }
    Ok(None)
}

/// Deletes the note: its file, its row, its index and links.
pub fn forget(conn: &Connection, dir: &Path, id: i64) -> Result<(), String> {
    remove_file(conn, dir, id);
    conn.execute("DELETE FROM memories WHERE id = ?1", [id]).map_err(sql)?;
    conn.execute("DELETE FROM memory_fts WHERE rowid = ?1", [id]).map_err(sql)?;
    conn.execute("DELETE FROM memory_links WHERE from_id = ?1 OR to_id = ?1", [id]).map_err(sql)?;
    mark_dirty();
    Ok(())
}

/// Ends an old fact (after the user agreed): `valid_until` now, `replaced_by` the new note.
pub fn end_note(conn: &Connection, dir: &Path, old: i64, new: i64, now: i64) -> Result<(), String> {
    conn.execute("UPDATE memories SET valid_until = ?2, replaced_by = ?3, updated_at = ?2 WHERE id = ?1", params![old, now, new]).map_err(sql)?;
    write_note(conn, dir, old)
}

/// The folder for the notes: the user's choice, else Documents/Groovy/memory.
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    let docs = home.join("Documents");
    let base = if docs.is_dir() { docs } else { home };
    base.join("Groovy").join("memory")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE memories (id INTEGER PRIMARY KEY, kind TEXT NOT NULL, content TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);",
        )
        .unwrap();
        c.execute_batch(SCHEMA_V2).unwrap();
        c
    }
    fn put(c: &Connection, kind: &str, text: &str, pinned: bool, at: i64) -> i64 {
        c.execute("INSERT INTO memories (kind, content, pinned, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)", params![kind, text, pinned as i64, at]).unwrap();
        c.last_insert_rowid()
    }
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("groovy-memnotes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }
    fn render_all(c: &Connection, q: &str, budget: usize, now: i64) -> Vec<String> {
        let mut used = 0;
        let mut out = vec![];
        for id in recall_ids(c, q, now).unwrap() {
            let t: String = c.query_row("SELECT content FROM memories WHERE id=?1", [id], |r| r.get(0)).unwrap();
            if used + t.len() + 1 <= budget {
                used += t.len() + 1;
                out.push(t);
            }
        }
        out
    }

    #[test]
    fn slugs_cannot_escape_the_folder() {
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("   "), "note");
        assert!(!safe_name("../x.md") && !safe_name("a/b.md") && !safe_name("a\\b.md") && safe_name("a.md"));
    }

    #[test]
    fn render_and_parse_round_trip() {
        let n = Note { id: Some(4), kind: "fact".into(), pinned: true, created: 1, updated: 2, valid_until: Some(9), replaced_by: Some(5), body: "I live in York, see [[work]]".into() };
        assert_eq!(parse(&render(&n), 0), n);
        assert_eq!(parse("just a plain note", 7).body, "just a plain note");
        assert_eq!(links_in("a [[x]] b [[Y.md]]"), vec!["x", "Y"]);
    }

    #[test]
    fn existing_memories_become_files_and_back() {
        let (c, d) = (db(), tmp("import"));
        let a = put(&c, "fact", "My dog is called Biscuit", false, 10);
        sync(&c, &d, 100).unwrap();
        assert!(d.join("my-dog-is-called-biscuit.md").exists());
        // The index is rebuilt from the files if the table is empty.
        c.execute("DELETE FROM memories", []).unwrap();
        sync(&c, &d, 100).unwrap();
        let (n, id): (String, i64) = c.query_row("SELECT content, id FROM memories", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((n.as_str(), id), ("My dog is called Biscuit", a));
    }

    #[test]
    fn outside_edits_are_seen_and_deleted_files_go() {
        let (c, d) = (db(), tmp("edit"));
        let a = put(&c, "fact", "I like tea", false, 10);
        sync(&c, &d, 100).unwrap();
        let f = d.join("i-like-tea.md");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let t = std::fs::read_to_string(&f).unwrap().replace("I like tea", "I like coffee");
        std::fs::write(&f, t).unwrap();
        // Groovy writing now must not overwrite the outside edit.
        write_note(&c, &d, a).unwrap();
        let body: String = c.query_row("SELECT content FROM memories WHERE id=?1", [a], |r| r.get(0)).unwrap();
        assert_eq!(body, "I like coffee");
        assert!(std::fs::read_to_string(&f).unwrap().contains("coffee"));
        // A new file made in an editor appears, with an id written back.
        std::fs::write(d.join("garden.md"), "I grow tomatoes").unwrap();
        sync(&c, &d, 200).unwrap();
        assert!(std::fs::read_to_string(d.join("garden.md")).unwrap().contains("id:"));
        std::fs::remove_file(&f).unwrap();
        sync(&c, &d, 300).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM memories", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn recall_is_by_relevance_with_pinned_first_and_a_budget() {
        let (c, d) = (db(), tmp("recall"));
        let _pin = put(&c, "preference", "Answer briefly", true, 1);
        let _h = put(&c, "fact", "I listen to music on my sony headphones", false, 5);
        let _n = put(&c, "fact", "Buy oat milk on Fridays", false, 50);
        sync(&c, &d, 100).unwrap();
        let music = render_all(&c, "what music should I play", 60, 100);
        assert_eq!(music[0], "Answer briefly");
        assert!(music.iter().any(|t| t.contains("headphones")));
        let shop = render_all(&c, "add to my shopping list", 60, 100);
        assert!(!shop.iter().any(|t| t.contains("headphones")));
        assert!(render_all(&c, "music", 30, 100).iter().map(|t| t.len() + 1).sum::<usize>() <= 30);
    }

    #[test]
    fn links_pull_in_one_hop_and_are_listed_both_ways() {
        let (c, d) = (db(), tmp("links"));
        let a = put(&c, "project", "Groovy island rewrite, owner is [[sam-reviewer]]", false, 5);
        let b = put(&c, "fact", "Sam reviewer prefers small PRs", false, 1);
        std::fs::create_dir_all(&d).unwrap();
        sync(&c, &d, 100).unwrap();
        c.execute("UPDATE memories SET file='sam-reviewer.md' WHERE id=?1", [b]).unwrap();
        std::fs::rename(d.join("sam-reviewer-prefers-small-prs.md"), d.join("sam-reviewer.md")).unwrap();
        mark_dirty();
        let r = recall_ids(&c, "island rewrite", 100).unwrap();
        assert_eq!(&r[..2], &[a, b]);
        assert_eq!(related(&c, a).unwrap().0, vec![b]);
        assert_eq!(related(&c, b).unwrap().1, vec![a]);
    }

    #[test]
    fn forget_removes_file_index_and_links_and_ended_notes_are_skipped() {
        let (c, d) = (db(), tmp("forget"));
        let old = put(&c, "fact", "I live in Leeds", false, 1);
        let new = put(&c, "fact", "I moved to York", false, 2);
        sync(&c, &d, 100).unwrap();
        end_note(&c, &d, old, new, 150).unwrap();
        assert!(!recall_ids(&c, "where do I live", 200).unwrap().contains(&old));
        assert!(recall_ids(&c, "where did I live before", 200).unwrap().contains(&old));
        assert!(std::fs::read_to_string(d.join("i-live-in-leeds.md")).unwrap().contains("valid_until: 150"));
        let (id, _) = find_best(&c, "forget my york move", 200).unwrap().unwrap();
        assert_eq!(id, new);
        forget(&c, &d, new).unwrap();
        assert!(!d.join("i-moved-to-york.md").exists());
        assert!(search(&c, "york", 5).unwrap().is_empty());
    }
}
