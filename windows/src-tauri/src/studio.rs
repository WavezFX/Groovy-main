// The Coding Studio's backend: workspaces, files, search, commands, git, and
// asking the AI about code.
//
// Safety rules, enforced here and not only in the page:
//   - Files are only read or written inside a workspace the user added with
//     the folder picker (or created through "New project"). Paths from the
//     page are relative, and are resolved and checked to stay inside it —
//     no "..", no absolute paths, no symlink out.
//   - Every save keeps the previous content in the history (undo / revert);
//     "delete" moves the file to Groovy's own trash, never erases it.
//   - Commands only run from an explicit click on a "Run" button that shows
//     the exact command and folder (the page sends `approved: true` only from
//     there); they run in the workspace, are logged to the audit log, stream
//     their output, and are killed after the timeout. AI answers never run
//     anything by themselves.
//   - Read-only git (status, diff, log) runs without asking; commit and push
//     are explicit buttons, push showing the remote first.
//   - Code goes to an AI provider only through studio_ask, which reports the
//     provider that will receive it so the page can ask first when it is
//     online (local-only mode never sends it anywhere).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{db, log};

const MAX_READ: u64 = 2 * 1024 * 1024;
const MAX_OUTPUT: usize = 2 * 1024 * 1024;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".venv", "venv", "__pycache__", ".next", ".idea"];

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

// ── Workspaces ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub git: bool,
}

pub fn add_workspace(conn: &Connection, dir: &Path) -> Result<Workspace, String> {
    let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
    if !dir.is_dir() {
        return Err("Not a folder.".into());
    }
    if dir.parent().is_none() {
        return Err("A whole drive can't be a workspace: pick a project folder.".into());
    }
    let p = dir.to_string_lossy().to_string();
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
    conn.execute("INSERT OR IGNORE INTO workspaces (path, name, added_at) VALUES (?1, ?2, ?3)", params![p, name, db::now()])
        .map_err(sql)?;
    let id: i64 = conn.query_row("SELECT id FROM workspaces WHERE path = ?1", [&p], |r| r.get(0)).map_err(sql)?;
    workspace(conn, id)
}

pub fn workspace(conn: &Connection, id: i64) -> Result<Workspace, String> {
    let (path, name): (String, String) = conn
        .query_row("SELECT path, name FROM workspaces WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(sql)?
        .ok_or("No such workspace.")?;
    let p = Path::new(&path);
    Ok(Workspace { id, exists: p.is_dir(), git: p.join(".git").exists(), path, name })
}

pub fn workspaces(conn: &Connection) -> Result<Vec<Workspace>, String> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM workspaces ORDER BY name COLLATE NOCASE").map_err(sql)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)?
    };
    ids.into_iter().map(|id| workspace(conn, id)).collect()
}

/// A relative path from the page, checked: no root, no prefix, no "..".
pub fn clean_relative(rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim().replace('\\', "/");
    let mut out = PathBuf::new();
    for c in Path::new(&rel).components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(format!("Not a path inside the workspace: {rel}")),
        }
    }
    Ok(out)
}

/// `rel` inside workspace `root`, for an existing or a new file: the deepest
/// existing ancestor must resolve inside the workspace (no symlink out).
pub fn resolve_in(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let rel = clean_relative(rel)?;
    let full = root.join(&rel);
    let mut probe = full.clone();
    while !probe.exists() {
        if !probe.pop() {
            return Err("Not a path inside the workspace.".into());
        }
    }
    let real = probe.canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(&root) {
        return Err("That path leads outside the workspace.".into());
    }
    Ok(full)
}

fn root_of(conn: &Connection, ws: i64) -> Result<PathBuf, String> {
    let w = workspace(conn, ws)?;
    if !w.exists {
        return Err("The workspace folder is gone.".into());
    }
    Ok(PathBuf::from(w.path))
}

// ── Files ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
    pub size: u64,
}

pub fn list_dir(root: &Path, rel: &str) -> Result<Vec<Entry>, String> {
    let dir = resolve_in(root, rel)?;
    let base = clean_relative(rel)?;
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let path = base.join(&name).to_string_lossy().replace('\\', "/");
        out.push(Entry { name, path, dir: meta.is_dir(), size: if meta.is_dir() { 0 } else { meta.len() } });
    }
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub text: String,
    /// Not shown as text: binary, or larger than 2 MB.
    pub binary: bool,
    pub size: u64,
}

pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

pub fn read_text(root: &Path, rel: &str) -> Result<FileText, String> {
    let path = resolve_in(root, rel)?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("Not a file.".into());
    }
    if meta.len() > MAX_READ {
        return Ok(FileText { text: String::new(), binary: true, size: meta.len() });
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if looks_binary(&bytes) {
        return Ok(FileText { text: String::new(), binary: true, size: meta.len() });
    }
    Ok(FileText { text: String::from_utf8_lossy(&bytes).into_owned(), binary: false, size: meta.len() })
}

fn remember(conn: &Connection, ws: i64, rel: &str, path: &Path, reason: &str) -> Result<(), String> {
    let before = std::fs::read(path).ok();
    conn.execute(
        "INSERT INTO file_history (workspace_id, path, content, existed, reason, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![ws, rel, before, before.is_some() as i64, reason, db::now()],
    )
    .map_err(sql)?;
    // 50 versions per file are plenty.
    conn.execute(
        "DELETE FROM file_history WHERE workspace_id = ?1 AND path = ?2 AND id NOT IN
           (SELECT id FROM file_history WHERE workspace_id = ?1 AND path = ?2 ORDER BY id DESC LIMIT 50)",
        params![ws, rel],
    )
    .map_err(sql)?;
    Ok(())
}

pub fn write_text(conn: &Connection, ws: i64, root: &Path, rel: &str, text: &str, reason: &str) -> Result<(), String> {
    let path = resolve_in(root, rel)?;
    if path.is_dir() {
        return Err("That is a folder.".into());
    }
    let rel = clean_relative(rel)?.to_string_lossy().replace('\\', "/");
    remember(conn, ws, &rel, &path, reason)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

/// Moves the file into Groovy's trash (kept in the history too), never erases.
pub fn trash(conn: &Connection, ws: i64, root: &Path, rel: &str) -> Result<(), String> {
    let path = resolve_in(root, rel)?;
    if !path.is_file() {
        return Err("Only files can be deleted here.".into());
    }
    let rel_clean = clean_relative(rel)?.to_string_lossy().replace('\\', "/");
    remember(conn, ws, &rel_clean, &path, "delete")?;
    let bin = crate::settings::local_dir().join("studio-trash").join(db::now().to_string());
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let dest = bin.join(path.file_name().unwrap_or_default());
    if std::fs::rename(&path, &dest).is_err() {
        std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: i64,
    pub reason: String,
    pub at: i64,
    pub existed: bool,
    pub size: i64,
}

pub fn history(conn: &Connection, ws: i64, rel: &str) -> Result<Vec<Version>, String> {
    let rel = clean_relative(rel)?.to_string_lossy().replace('\\', "/");
    let mut stmt = conn
        .prepare("SELECT id, reason, at, existed, COALESCE(length(content), 0) FROM file_history WHERE workspace_id = ?1 AND path = ?2 ORDER BY id DESC")
        .map_err(sql)?;
    let rows = stmt
        .query_map(params![ws, rel], |r| Ok(Version { id: r.get(0)?, reason: r.get(1)?, at: r.get(2)?, existed: r.get::<_, i64>(3)? != 0, size: r.get(4)? }))
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// Puts the file back as it was before version `id` was saved over it (the
/// current content is kept in the history too, so a revert can be undone).
pub fn revert(conn: &Connection, ws: i64, root: &Path, id: i64) -> Result<String, String> {
    let (rel, content, existed): (String, Option<Vec<u8>>, i64) = conn
        .query_row("SELECT path, content, existed FROM file_history WHERE id = ?1 AND workspace_id = ?2", params![id, ws], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(sql)?;
    let path = resolve_in(root, &rel)?;
    remember(conn, ws, &rel, &path, "revert")?;
    if existed == 0 {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, content.unwrap_or_default()).map_err(|e| e.to_string())?;
    }
    Ok(rel)
}

// ── Search ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// Lines containing `query` (case-insensitive) in the workspace's text files.
pub fn search(root: &Path, query: &str, limit: usize) -> Vec<Hit> {
    let q = query.to_lowercase();
    if q.trim().is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut files = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
            if meta.is_dir() {
                if depth < 16 && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push((e.path(), depth + 1));
                }
                continue;
            }
            if !meta.is_file() || meta.len() > 1024 * 1024 {
                continue;
            }
            files += 1;
            if files > 20_000 {
                return hits;
            }
            let Ok(bytes) = std::fs::read(e.path()) else { continue };
            if looks_binary(&bytes) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let rel = e.path().strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            for (i, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&q) {
                    hits.push(Hit { path: rel.clone(), line: i + 1, text: line.trim().chars().take(240).collect() });
                    if hits.len() >= limit {
                        return hits;
                    }
                }
            }
        }
    }
    hits
}

// ── Commands ──────────────────────────────────────────────────────────────────

struct Run {
    child: Arc<Mutex<Child>>,
    started: Instant,
}

static RUNS: Mutex<Option<HashMap<u64, Run>>> = Mutex::new(None);
static NEXT_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    run: u64,
    stream: &'static str,
    text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub run: u64,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub seconds: f64,
}

fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd.exe");
        c.args(["/D", "/S", "/C", command]);
        crate::platform::no_console(&mut c);
        c
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("/bin/sh");
        c.args(["-c", command]);
        // Its own process group, so "Stop" ends everything it started.
        c.process_group(0);
        c
    }
}

fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let mut k = Command::new("taskkill");
        k.args(["/T", "/F", "/PID", &child.id().to_string()]);
        let _ = crate::platform::no_console(&mut k).status();
    }
    #[cfg(not(windows))]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

fn pump(app: AppHandle, run: u64, stream: &'static str, mut src: impl Read + Send + 'static, budget: Arc<Mutex<usize>>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let n = match src.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let mut left = budget.lock().unwrap();
            if *left == 0 {
                continue; // keep draining so the process doesn't block, show nothing more
            }
            let take = n.min(*left);
            *left -= take;
            let mut text = String::from_utf8_lossy(&buf[..take]).into_owned();
            if *left == 0 {
                text.push_str("\n[output cut at 2 MB]\n");
            }
            drop(left);
            let _ = app.emit_to("studio", "studio-output", Output { run, stream, text });
        }
    });
}

/// Starts `command` in the workspace; output streams as "studio-output",
/// the end as "studio-finished". Returns the run id.
pub fn start(app: &AppHandle, ws: &Workspace, command: &str, timeout: Duration) -> Result<u64, String> {
    let command = command.trim();
    if command.is_empty() || command.len() > 4000 || command.contains('\0') {
        return Err("Not a command.".into());
    }
    let mut cmd = shell(command);
    cmd.current_dir(&ws.path).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start it: {e}"))?;
    let run = NEXT_RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let budget = Arc::new(Mutex::new(MAX_OUTPUT));
    if let Some(out) = child.stdout.take() {
        pump(app.clone(), run, "stdout", out, budget.clone());
    }
    if let Some(err) = child.stderr.take() {
        pump(app.clone(), run, "stderr", err, budget);
    }
    let child = Arc::new(Mutex::new(child));
    RUNS.lock().unwrap().get_or_insert_with(HashMap::new).insert(run, Run { child: child.clone(), started: Instant::now() });
    log::line(format!("studio: run #{run} in workspace {}", ws.id));
    let app = app.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut timed_out = false;
        let code = loop {
            if let Ok(Some(status)) = child.lock().unwrap().try_wait() {
                break status.code();
            }
            if started.elapsed() > timeout {
                timed_out = true;
                kill_tree(&mut child.lock().unwrap());
                break None;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        // Let the pumps flush the last lines first.
        std::thread::sleep(Duration::from_millis(150));
        RUNS.lock().unwrap().as_mut().map(|m| m.remove(&run));
        let _ = app.emit_to("studio", "studio-finished", Finished { run, code, timed_out, seconds: started.elapsed().as_secs_f64() });
    });
    Ok(run)
}

pub fn stop(run: u64) -> bool {
    let child = RUNS.lock().unwrap().as_ref().and_then(|m| m.get(&run).map(|r| (r.child.clone(), r.started)));
    match child {
        Some((c, _)) => {
            kill_tree(&mut c.lock().unwrap());
            true
        }
        None => false,
    }
}

// ── Git ───────────────────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(root).stdin(Stdio::null());
    #[cfg(windows)]
    crate::platform::no_console(&mut cmd);
    let out = cmd.output().map_err(|e| format!("git isn't available: {e}"))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(if err.trim().is_empty() { format!("git {} failed", args.first().unwrap_or(&"")) } else { err.trim().to_string() });
    }
    if text.len() > MAX_OUTPUT {
        text.truncate(MAX_OUTPUT);
        text.push_str("\n[cut]");
    }
    Ok(text)
}

/// A commit message is one argument to git, never shell text.
pub fn commit_args(message: &str) -> Result<Vec<String>, String> {
    let m = message.trim();
    if m.is_empty() {
        return Err("A commit needs a message.".into());
    }
    Ok(vec!["commit".into(), "-m".into(), m.chars().take(5000).collect()])
}

// ── AI ────────────────────────────────────────────────────────────────────────

/// What the AI is told when it works on code here: plain instructions for the
/// formats the Studio parses (studio/ai.ts), and that nothing it writes runs
/// without the user.
pub const SYSTEM: &str = "You are Groovy's coding assistant inside the user's code editor. \
When you propose changes to files, give each changed or new file in full, as a level-3 heading with its path relative to the project root, \
followed by one fenced code block with the complete new content, for example:\n\n### src/main.py\n```python\n...\n```\n\n\
Never use placeholders such as \"rest of the file unchanged\": always the whole file. \
When a command should be run (install, build, test), put each one alone in a ```shell block; the user decides whether to run it. \
Keep explanations short and precise.";

pub const MAX_CONTEXT_CHARS: usize = 60_000;

/// The prompt for a question about some files of the workspace, cut to a budget.
pub fn build_prompt(question: &str, files: &[(String, String)], extra: &str) -> String {
    let mut out = String::new();
    let mut used = 0;
    for (path, text) in files {
        let room = MAX_CONTEXT_CHARS.saturating_sub(used);
        if room < 200 {
            out.push_str(&format!("(more files left out: {path} and later)\n\n"));
            break;
        }
        let body: String = text.chars().take(room).collect();
        let cut = body.len() < text.len();
        used += body.len();
        out.push_str(&format!("### {path}\n```\n{body}\n```\n{}\n", if cut { "(file cut here)" } else { "" }));
    }
    if !extra.trim().is_empty() {
        out.push_str(&format!("Output of the last command:\n```\n{}\n```\n\n", extra.chars().rev().take(8000).collect::<String>().chars().rev().collect::<String>()));
    }
    out.push_str(question.trim());
    out
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn studio_workspaces() -> Result<Vec<Workspace>, String> {
    workspaces(&*db::conn()?)
}

async fn pick_folder(app: &AppHandle, title: String) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |p| {
        let _ = tx.send(p);
    });
    match rx.await.map_err(|e| e.to_string())? {
        Some(p) => Ok(Some(p.into_path().map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn studio_add_workspace(app: AppHandle) -> Result<Option<Workspace>, String> {
    let Some(dir) = pick_folder(&app, crate::i18n::t("Open a project folder")).await? else { return Ok(None) };
    let w = add_workspace(&*db::conn()?, &dir)?;
    crate::tools::audit(&*db::conn()?, "studio", &serde_json::json!({ "workspace": w.path }), "workspace added");
    Ok(Some(w))
}

/// A new, empty project folder `name` inside a folder the user picks.
#[tauri::command]
pub async fn studio_new_project(app: AppHandle, name: String) -> Result<Option<Workspace>, String> {
    let name: String = name.trim().chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.')).take(60).collect();
    let name = name.trim_matches('.').trim().to_string();
    if name.is_empty() {
        return Err("Give the project a name (letters, numbers, - and _).".into());
    }
    let Some(parent) = pick_folder(&app, crate::i18n::t("Where should the project go?")).await? else { return Ok(None) };
    let dir = parent.join(&name);
    if dir.exists() {
        return Err(format!("{} already exists.", dir.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let w = add_workspace(&*db::conn()?, &dir)?;
    crate::tools::audit(&*db::conn()?, "studio", &serde_json::json!({ "workspace": w.path }), "project created");
    Ok(Some(w))
}

#[tauri::command]
pub fn studio_remove_workspace(id: i64) -> Result<(), String> {
    db::conn()?.execute("DELETE FROM workspaces WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

#[tauri::command]
pub fn studio_list(ws: i64, path: String) -> Result<Vec<Entry>, String> {
    let root = root_of(&*db::conn()?, ws)?;
    list_dir(&root, &path)
}

#[tauri::command]
pub fn studio_read(ws: i64, path: String) -> Result<FileText, String> {
    let root = root_of(&*db::conn()?, ws)?;
    read_text(&root, &path)
}

#[tauri::command]
pub fn studio_write(ws: i64, path: String, text: String, reason: Option<String>) -> Result<(), String> {
    if text.len() > 8 * 1024 * 1024 {
        return Err("That file is too large to save from the editor.".into());
    }
    let conn = db::conn()?;
    let root = root_of(&conn, ws)?;
    let reason = reason.unwrap_or_else(|| "save".into());
    write_text(&conn, ws, &root, &path, &text, &reason)?;
    crate::tools::audit(&conn, "studio", &serde_json::json!({ "file": path }), &reason);
    Ok(())
}

#[tauri::command]
pub fn studio_delete(ws: i64, path: String) -> Result<(), String> {
    let conn = db::conn()?;
    let root = root_of(&conn, ws)?;
    trash(&conn, ws, &root, &path)?;
    crate::tools::audit(&conn, "studio", &serde_json::json!({ "file": path }), "moved to trash");
    Ok(())
}

#[tauri::command]
pub fn studio_history(ws: i64, path: String) -> Result<Vec<Version>, String> {
    history(&*db::conn()?, ws, &path)
}

#[tauri::command]
pub fn studio_revert(ws: i64, id: i64) -> Result<String, String> {
    let conn = db::conn()?;
    let root = root_of(&conn, ws)?;
    revert(&conn, ws, &root, id)
}

#[tauri::command]
pub async fn studio_search(ws: i64, query: String) -> Result<Vec<Hit>, String> {
    let root = root_of(&*db::conn()?, ws)?;
    tauri::async_runtime::spawn_blocking(move || search(&root, &query, 500)).await.map_err(|e| e.to_string())
}

/// Runs a command the user approved by clicking Run on it.
#[tauri::command]
pub fn studio_run(app: AppHandle, ws: i64, command: String, approved: bool, timeout_minutes: Option<u64>) -> Result<u64, String> {
    if !approved {
        return Err("Commands only run when you click Run.".into());
    }
    let conn = db::conn()?;
    let w = workspace(&conn, ws)?;
    crate::tools::audit(&conn, "terminal", &serde_json::json!({ "workspace": w.path, "command": command }), "run (approved)");
    drop(conn);
    start(&app, &w, &command, Duration::from_secs(timeout_minutes.unwrap_or(10).clamp(1, 120) * 60))
}

#[tauri::command]
pub fn studio_stop(run: u64) -> bool {
    stop(run)
}

#[tauri::command]
pub async fn studio_git(ws: i64, what: String) -> Result<String, String> {
    let root = root_of(&*db::conn()?, ws)?;
    tauri::async_runtime::spawn_blocking(move || match what.as_str() {
        "status" => git(&root, &["status", "--short", "--branch"]),
        "diff" => git(&root, &["diff", "--no-color"]),
        "log" => git(&root, &["log", "--oneline", "--decorate", "-n", "30", "--no-color"]),
        "remote" => git(&root, &["remote", "-v"]),
        _ => Err("Unknown git view.".into()),
    })
    .await
    .map_err(|e| e.to_string())?
}

/// git init / add -A / commit / push — each one an explicit click in the Studio.
#[tauri::command]
pub async fn studio_git_action(ws: i64, action: String, message: Option<String>) -> Result<String, String> {
    let root = {
        let conn = db::conn()?;
        let root = root_of(&conn, ws)?;
        crate::tools::audit(&conn, "git", &serde_json::json!({ "workspace": root.to_string_lossy(), "action": action }), "approved");
        root
    };
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "init" => git(&root, &["init"]),
        "stage" => git(&root, &["add", "-A"]),
        "commit" => {
            let args = commit_args(message.as_deref().unwrap_or(""))?;
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            git(&root, &refs)
        }
        "push" => git(&root, &["push"]),
        _ => Err("Unknown git action.".into()),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskTarget {
    pub provider: String,
    pub local: bool,
}

/// Who would receive code sent from the Studio now, so the page can ask first.
#[tauri::command]
pub fn studio_ai_target(shared: tauri::State<crate::Shared>) -> AskTarget {
    let settings = shared.settings.lock().unwrap().clone();
    let (provider, local) = crate::chat::target(&settings);
    AskTarget { provider, local }
}

/// Asks the AI about files of the workspace. `consent` must be true when the
/// target is online (the page asked); the answer streams as "studio-delta".
#[tauri::command]
pub async fn studio_ask(
    app: AppHandle,
    shared: tauri::State<'_, crate::Shared>,
    ws: i64,
    question: String,
    files: Vec<String>,
    output: Option<String>,
    consent: bool,
) -> Result<String, String> {
    let settings = shared.settings.lock().unwrap().clone();
    let (provider, local) = crate::chat::target(&settings);
    if !local && !consent {
        return Err(format!("Sending code to {provider} needs your OK first."));
    }
    let contents = {
        let conn = db::conn()?;
        let root = root_of(&conn, ws)?;
        let mut v = Vec::new();
        for f in files.iter().take(30) {
            if let Ok(t) = read_text(&root, f) {
                if !t.binary {
                    v.push((f.clone(), t.text));
                }
            }
        }
        if let Ok(conn) = db::conn() {
            crate::tools::audit(&conn, "studio_ai", &serde_json::json!({ "provider": provider, "files": files }), if local { "asked (local)" } else { "asked (online, consented)" });
        }
        v
    };
    let prompt = build_prompt(&question, &contents, output.as_deref().unwrap_or(""));
    let sink = app.clone();
    crate::chat::complete(&app, &settings, SYSTEM, prompt, move |so_far| {
        let _ = sink.emit_to("studio", "studio-delta", so_far);
    })
    .await
}

#[tauri::command]
pub fn open_studio_window(app: AppHandle) {
    show(&app);
}

/// The Coding Studio's window.
pub const LABEL: &str = "studio";

pub fn show(app: &AppHandle) {
    crate::ceiling::one_panel(app, LABEL);
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    // Made when opened, destroyed when closed, from its own thread and with
    // the island's browser arguments (see lib.rs create_hub_window).
    static OPENING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if OPENING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let url = crate::page_url(&app, "studio.html", None);
    let _ = std::thread::Builder::new().name("groovy-studio-window".into()).spawn(move || {
        match tauri::WebviewWindowBuilder::new(&app, LABEL, url)
            .additional_browser_args(crate::BROWSER_ARGS)
            .title(crate::i18n::t("Coding Studio — Groovy"))
            .inner_size(1200.0, 800.0)
            .min_inner_size(720.0, 480.0)
            .resizable(true)
            .center()
            .build()
        {
            Ok(win) => {
                log::line("studio window opened");
                crate::webview_memory::trim(&win);
            }
            Err(e) => log::line(format!("studio window failed: {e}")),
        }
        OPENING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("groovy-studio-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn paths_stay_inside_the_workspace() {
        let d = tmp("paths");
        std::fs::create_dir_all(d.join("src")).unwrap();
        assert!(resolve_in(&d, "src/main.rs").is_ok());
        assert!(resolve_in(&d, "new/dir/file.txt").is_ok(), "new files are fine");
        assert!(resolve_in(&d, "../escape.txt").is_err());
        assert!(resolve_in(&d, "src/../../escape.txt").is_err());
        assert!(resolve_in(&d, "/etc/passwd").is_err());
        assert!(resolve_in(&d, "C:\\Windows\\win.ini").is_err() || cfg!(not(windows)));
        assert_eq!(clean_relative("./src//a.rs").unwrap(), PathBuf::from("src/a.rs"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", d.join("out")).unwrap();
            assert!(resolve_in(&d, "out/x").is_err(), "no symlink out");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn saves_keep_history_and_revert_undoes() {
        let d = tmp("history");
        let c = db::open_in_memory().unwrap();
        let w = add_workspace(&c, &d).unwrap();
        write_text(&c, w.id, &d, "a.txt", "one", "save").unwrap();
        write_text(&c, w.id, &d, "a.txt", "two", "save").unwrap();
        assert_eq!(read_text(&d, "a.txt").unwrap().text, "two");
        let h = history(&c, w.id, "a.txt").unwrap();
        assert_eq!(h.len(), 2);
        assert!(!h[1].existed, "the first save created the file");
        // Back to before the second save.
        revert(&c, w.id, &d, h[0].id).unwrap();
        assert_eq!(read_text(&d, "a.txt").unwrap().text, "one");
        // Before the first save: the file did not exist.
        revert(&c, w.id, &d, h[1].id).unwrap();
        assert!(!d.join("a.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn delete_moves_to_the_trash_and_can_be_reverted() {
        let d = tmp("trash");
        let c = db::open_in_memory().unwrap();
        let w = add_workspace(&c, &d).unwrap();
        write_text(&c, w.id, &d, "keep.txt", "precious", "save").unwrap();
        trash(&c, w.id, &d, "keep.txt").unwrap();
        assert!(!d.join("keep.txt").exists());
        let h = history(&c, w.id, "keep.txt").unwrap();
        revert(&c, w.id, &d, h[0].id).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("keep.txt")).unwrap(), "precious");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lists_and_searches_skipping_heavy_folders() {
        let d = tmp("search");
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::create_dir_all(d.join("node_modules/x")).unwrap();
        std::fs::write(d.join("src/main.rs"), "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();
        std::fs::write(d.join("node_modules/x/i.js"), "hello").unwrap();
        std::fs::write(d.join("bin.dat"), [0u8, 1, 2, b'h', b'e', b'l', b'l', b'o']).unwrap();
        let top = list_dir(&d, "").unwrap();
        assert_eq!(top[0].name, "node_modules", "folders first");
        assert!(top.iter().any(|e| e.name == "src" && e.dir));
        let hits = search(&d, "HELLO", 50);
        assert_eq!(hits, vec![Hit { path: "src/main.rs".into(), line: 2, text: "println!(\"Hello\");".into() }]);
        assert!(read_text(&d, "bin.dat").unwrap().binary);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn workspaces_must_be_real_folders_not_drives() {
        let c = db::open_in_memory().unwrap();
        assert!(add_workspace(&c, Path::new("/")).is_err());
        assert!(add_workspace(&c, Path::new("/definitely/not/here")).is_err());
    }

    #[test]
    fn commit_messages_are_arguments() {
        assert_eq!(commit_args("  fix: thing; rm -rf / ").unwrap(), vec!["commit", "-m", "fix: thing; rm -rf /"]);
        assert!(commit_args("   ").is_err());
    }

    #[test]
    fn prompts_carry_files_within_a_budget() {
        let big = "x".repeat(MAX_CONTEXT_CHARS + 100);
        let p = build_prompt("Why?", &[("a.rs".into(), "fn a() {}".into()), ("big.txt".into(), big), ("c.rs".into(), "c".into())], "error: boom");
        assert!(p.contains("### a.rs\n```\nfn a() {}\n```"));
        assert!(p.contains("(file cut here)"));
        assert!(p.contains("more files left out: c.rs"));
        assert!(p.contains("error: boom"));
        assert!(p.ends_with("Why?"));
    }

    #[test]
    fn commands_stream_and_report_their_exit_code() {
        // The runner itself, without a window: a real child process.
        let mut cmd = shell(if cfg!(windows) { "echo hi && exit 3" } else { "echo hi; exit 3" });
        let out = cmd.output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
        assert_eq!(out.status.code(), Some(3));
    }
}
