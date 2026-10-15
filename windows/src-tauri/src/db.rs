// Groovy's local database: %LOCALAPPDATA%\Groovy\groovy.db (Windows) or
// ~/.local/share/groovy/groovy.db (Linux). SQLite, compiled in (no DLL to
// ship), opened once and shared behind a mutex. Nothing in it ever leaves the
// machine unless the user exports it.
//
// What lives here is kept apart on purpose (memory.rs, tasks.rs):
//   memories        long-term facts, preferences, project and task context
//   conversations   chat history, one row per conversation
//   messages        its turns
//   tasks           reminders and to-dos
// Deleting chats never touches memories, and the other way round.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use rusqlite::Connection;

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Each entry upgrades the schema by one version; never edit a shipped one.
const MIGRATIONS: &[&str] = &[
    // 1
    "CREATE TABLE memories (
        id INTEGER PRIMARY KEY,
        kind TEXT NOT NULL CHECK (kind IN ('preference', 'fact', 'project', 'task')),
        content TEXT NOT NULL,
        pinned INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE conversations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
        content TEXT NOT NULL,
        provider TEXT NOT NULL DEFAULT '',
        created_at INTEGER NOT NULL
    );
    CREATE INDEX messages_by_conversation ON messages(conversation_id, id);
    CREATE TABLE tasks (
        id INTEGER PRIMARY KEY,
        text TEXT NOT NULL,
        due_at INTEGER,
        done INTEGER NOT NULL DEFAULT 0,
        notified INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX tasks_due ON tasks(done, due_at);",
    // 2: messages sent to online providers, per local day, for the daily limit.
    "CREATE TABLE usage (
        day TEXT NOT NULL,
        provider TEXT NOT NULL,
        count INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (day, provider)
    );",
    // 3: the local music library (music.rs).
    "CREATE TABLE music_folders (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
    );
    CREATE TABLE tracks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        folder_id INTEGER NOT NULL REFERENCES music_folders(id) ON DELETE CASCADE,
        path TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        artist TEXT NOT NULL DEFAULT '',
        album TEXT NOT NULL DEFAULT '',
        genre TEXT NOT NULL DEFAULT '',
        duration REAL NOT NULL DEFAULT 0,
        track_no INTEGER,
        has_art INTEGER NOT NULL DEFAULT 0,
        favourite INTEGER NOT NULL DEFAULT 0,
        play_count INTEGER NOT NULL DEFAULT 0,
        last_played INTEGER,
        mtime INTEGER NOT NULL DEFAULT 0,
        added_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX tracks_by_album ON tracks(artist, album, track_no);
    CREATE TABLE playlists (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE playlist_tracks (
        playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position INTEGER NOT NULL
    );
    CREATE INDEX playlist_order ON playlist_tracks(playlist_id, position);
    CREATE TABLE kv (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );",
    // 4: what the desktop tools did, asked and were refused (tools.rs).
    "CREATE TABLE tool_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        tool TEXT NOT NULL,
        args TEXT NOT NULL,
        outcome TEXT NOT NULL,
        at INTEGER NOT NULL
    );",
];

pub fn path() -> PathBuf {
    crate::settings::local_dir().join("groovy.db")
}

/// Opens (creating if needed) and migrates the database. Called once at start;
/// an error is logged and every later call reports the database as unavailable.
pub fn init() -> Result<(), String> {
    if DB.get().is_some() {
        return Ok(());
    }
    let dir = crate::settings::local_dir();
    crate::platform::ensure_private_dir(&dir).map_err(|e| e.to_string())?;
    let conn = open(&path())?;
    let _ = DB.set(Mutex::new(conn));
    Ok(())
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    prepare(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, String> {
    prepare(Connection::open_in_memory().map_err(|e| e.to_string())?)
}

fn prepare(conn: Connection) -> Result<Connection, String> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 2000;")
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if version > MIGRATIONS.len() {
        return Err(format!(
            "groovy.db is from a newer Groovy (schema {version}, this one knows {}); not touching it",
            MIGRATIONS.len()
        ));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| format!("migration {}: {e}", i + 1))?;
        tx.pragma_update(None, "user_version", i + 1).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The shared connection, or a readable error when the database could not open.
pub fn conn() -> Result<MutexGuard<'static, Connection>, String> {
    let db = DB.get().ok_or_else(|| crate::i18n::t("The local database is not available."))?;
    Ok(db.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Online messages sent on `day` (YYYY-MM-DD), all providers together.
pub fn usage_online(conn: &Connection, day: &str) -> u32 {
    conn.query_row("SELECT COALESCE(SUM(count), 0) FROM usage WHERE day = ?1", [day], |r| r.get::<_, i64>(0))
        .map(|n| n.max(0) as u32)
        .unwrap_or(0)
}

pub fn usage_bump(conn: &Connection, day: &str, provider: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage (day, provider, count) VALUES (?1, ?2, 1)
         ON CONFLICT (day, provider) DO UPDATE SET count = count + 1",
        [day, provider],
    )
    .map_err(|e| e.to_string())?;
    // Only the last few weeks are worth keeping.
    let _ = conn.execute("DELETE FROM usage WHERE day < date(?1, '-60 days')", [day]);
    Ok(())
}

#[tauri::command]
pub fn usage_today() -> Result<u32, String> {
    let t = crate::platform::local_time();
    Ok(usage_online(&*conn()?, &format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)))
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_a_fresh_database_to_the_latest_schema() {
        let conn = open_in_memory().unwrap();
        let v: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len());
        for table in ["memories", "conversations", "messages", "tasks", "usage", "tracks", "playlists", "playlist_tracks", "kv", "tool_log"] {
            let n: i64 = conn
                .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1", [table], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1, "{table}");
        }
    }

    #[test]
    fn reopening_keeps_the_data_and_does_not_migrate_twice() {
        let dir = std::env::temp_dir().join(format!("groovy-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.db");
        let _ = std::fs::remove_file(&file);
        {
            let c = open(&file).unwrap();
            c.execute("INSERT INTO tasks (text, created_at) VALUES ('x', 1)", []).unwrap();
        }
        let c = open(&file).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(prepare(conn).unwrap_err().contains("newer Groovy"));
    }

    #[test]
    fn counts_online_usage_per_day() {
        let c = open_in_memory().unwrap();
        assert_eq!(usage_online(&c, "2026-10-15"), 0);
        usage_bump(&c, "2026-10-15", "openai").unwrap();
        usage_bump(&c, "2026-10-15", "openai").unwrap();
        usage_bump(&c, "2026-10-15", "anthropic").unwrap();
        usage_bump(&c, "2026-10-14", "openai").unwrap();
        assert_eq!(usage_online(&c, "2026-10-15"), 3);
        assert_eq!(usage_online(&c, "2026-10-14"), 1);
        usage_bump(&c, "2027-01-01", "openai").unwrap();
        assert_eq!(usage_online(&c, "2026-10-14"), 0, "old days are dropped");
    }

    #[test]
    fn rejects_an_unknown_memory_kind() {
        let conn = open_in_memory().unwrap();
        let r = conn.execute(
            "INSERT INTO memories (kind, content, created_at, updated_at) VALUES ('secret', 'x', 1, 1)",
            [],
        );
        assert!(r.is_err());
    }
}
