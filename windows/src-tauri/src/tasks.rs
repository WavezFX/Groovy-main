// Reminders and to-dos, in the local database (db.rs). Works fully offline.
//
// A task with a due time is a reminder: a background loop checks once every
// 20 seconds, and each one that comes due is announced once — a `reminder`
// event to the island (which shows it and plays a sound) and a log line —
// then marked notified so it never fires twice, even across restarts.
// A reminder that came due while Groovy was closed fires at the next start.

use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db;

const MAX_TEXT: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub text: String,
    /// Unix seconds; None for a plain to-do.
    pub due_at: Option<i64>,
    pub done: bool,
    pub notified: bool,
    pub created_at: i64,
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        text: r.get(1)?,
        due_at: r.get(2)?,
        done: r.get::<_, i64>(3)? != 0,
        notified: r.get::<_, i64>(4)? != 0,
        created_at: r.get(5)?,
    })
}

const COLS: &str = "id, text, due_at, done, notified, created_at";

pub fn add(conn: &Connection, text: &str, due_at: Option<i64>) -> Result<Task, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(crate::i18n::t("A reminder needs some text."));
    }
    if text.chars().count() > MAX_TEXT {
        return Err(format!("A reminder is at most {MAX_TEXT} characters."));
    }
    if due_at.is_some_and(|d| d < 0) {
        return Err("Invalid time.".into());
    }
    conn.execute(
        "INSERT INTO tasks (text, due_at, created_at) VALUES (?1, ?2, ?3)",
        params![text, due_at, db::now()],
    )
    .map_err(sql)?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Task, String> {
    conn.query_row(&format!("SELECT {COLS} FROM tasks WHERE id = ?1"), [id], row).map_err(sql)
}

/// Open tasks first (soonest due first, undated last), then done ones if asked.
pub fn list(conn: &Connection, include_done: bool) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLS} FROM tasks WHERE ?1 OR done = 0
             ORDER BY done, due_at IS NULL, due_at, id"
        ))
        .map_err(sql)?;
    let rows = stmt.query_map([include_done], row).map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn set_done(conn: &Connection, id: i64, done: bool) -> Result<Task, String> {
    conn.execute("UPDATE tasks SET done = ?2 WHERE id = ?1", params![id, done as i64]).map_err(sql)?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

pub fn clear_done(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM tasks WHERE done = 1", []).map_err(sql)
}

/// The open reminders due at or before `now` that were not announced yet,
/// marked as announced in the same transaction.
pub fn take_due(conn: &Connection, now: i64) -> Result<Vec<Task>, String> {
    let tx = conn.unchecked_transaction().map_err(sql)?;
    let due = {
        let mut stmt = tx
            .prepare(&format!(
                "SELECT {COLS} FROM tasks WHERE done = 0 AND notified = 0 AND due_at IS NOT NULL AND due_at <= ?1 ORDER BY due_at"
            ))
            .map_err(sql)?;
        let rows = stmt.query_map([now], row).map_err(sql)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql)?
    };
    for t in &due {
        tx.execute("UPDATE tasks SET notified = 1 WHERE id = ?1", [t.id]).map_err(sql)?;
    }
    tx.commit().map_err(sql)?;
    Ok(due)
}

/// Starts the reminder loop.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let due = db::conn().and_then(|c| take_due(&c, db::now()));
            match due {
                Ok(due) => {
                    for t in due {
                        crate::log::line(format!("reminder due: #{}", t.id));
                        let _ = app.emit("reminder", &t);
                    }
                }
                Err(e) => crate::log::line(format!("reminders: {e}")),
            }
            tokio::time::sleep(Duration::from_secs(20)).await;
        }
    });
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn task_list(include_done: bool) -> Result<Vec<Task>, String> {
    list(&*db::conn()?, include_done)
}

#[tauri::command]
pub fn task_add(text: String, due_at: Option<i64>) -> Result<Task, String> {
    add(&*db::conn()?, &text, due_at)
}

#[tauri::command]
pub fn task_set_done(id: i64, done: bool) -> Result<Task, String> {
    set_done(&*db::conn()?, id, done)
}

#[tauri::command]
pub fn task_delete(id: i64) -> Result<(), String> {
    delete(&*db::conn()?, id)
}

#[tauri::command]
pub fn task_clear_done() -> Result<usize, String> {
    clear_done(&*db::conn()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_list_done_delete() {
        let c = db::open_in_memory().unwrap();
        let todo = add(&c, "buy milk", None).unwrap();
        let later = add(&c, "call mum", Some(2000)).unwrap();
        let soon = add(&c, "stand up", Some(1000)).unwrap();
        let ids: Vec<_> = list(&c, false).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![soon.id, later.id, todo.id], "soonest first, undated last");
        set_done(&c, todo.id, true).unwrap();
        assert_eq!(list(&c, false).unwrap().len(), 2);
        assert_eq!(list(&c, true).unwrap().len(), 3);
        assert_eq!(clear_done(&c).unwrap(), 1);
        delete(&c, later.id).unwrap();
        assert_eq!(list(&c, true).unwrap().len(), 1);
    }

    #[test]
    fn validates() {
        let c = db::open_in_memory().unwrap();
        assert!(add(&c, "  ", None).is_err());
        assert!(add(&c, &"x".repeat(MAX_TEXT + 1), None).is_err());
        assert!(add(&c, "x", Some(-5)).is_err());
    }

    #[test]
    fn due_reminders_fire_once() {
        let c = db::open_in_memory().unwrap();
        let a = add(&c, "a", Some(100)).unwrap();
        add(&c, "b", Some(500)).unwrap();
        add(&c, "todo", None).unwrap();
        let done = add(&c, "done", Some(50)).unwrap();
        set_done(&c, done.id, true).unwrap();
        let due = take_due(&c, 200).unwrap();
        assert_eq!(due.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a.id]);
        assert!(take_due(&c, 200).unwrap().is_empty(), "never twice");
        assert_eq!(take_due(&c, 10_000).unwrap().len(), 1);
    }
}
