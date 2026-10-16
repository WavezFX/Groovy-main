// The Skills Manager's Tauri commands. Every change goes through here:
// installs need a preview's token (the user saw the SKILL.md), removals copy the
// skill to `.groovy/skills-trash/` first, updates skip skills the user edited,
// and each change is written to the project's history so it can be undone.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::analyze::{self, ProjectProfile};
use super::cli::{self, Installed, Scope};
use super::recommend::{self, Candidate, Recommendations};
use super::store::{self, HistoryEntry, UpdateCheck};

/// Agents Groovy offers to install for; the CLI knows many more.
pub const AGENTS: &[(&str, &str)] = &[
    ("claude-code", "Claude Code"),
    ("codex", "Codex"),
    ("cursor", "Cursor"),
    ("opencode", "OpenCode"),
    ("gemini-cli", "Gemini CLI"),
    ("github-copilot", "GitHub Copilot"),
];

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

fn audit(what: &str, detail: serde_json::Value) {
    if let Ok(conn) = crate::db::conn() {
        crate::tools::audit(&conn, "skills", &detail, what);
    }
}

// ── Status ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub node_version: Option<String>,
    pub node_ok: bool,
    pub node_min: String,
    pub cli_version: String,
    pub agents: Vec<(String, String)>,
    pub platform: String,
}

#[tauri::command]
pub async fn skills_status() -> Status {
    let v = tauri::async_runtime::spawn_blocking(cli::node_version).await.ok().flatten();
    let ok = v.as_deref().and_then(cli::parse_node_version).is_some_and(cli::node_new_enough);
    Status {
        node_version: v,
        node_ok: ok,
        node_min: format!("{}.{}", cli::NODE_MIN.0, cli::NODE_MIN.1),
        cli_version: cli::CLI_VERSION.into(),
        agents: AGENTS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        platform: if cfg!(windows) { "windows" } else { "linux" }.into(),
    }
}

/// The user clicked "Install Node.js": winget on Windows (the system asks for
/// approval again), else nodejs.org's download page.
#[tauri::command]
pub async fn skills_install_node() -> Result<String, String> {
    #[cfg(windows)]
    {
        let winget = blocking(|| {
            let mut c = std::process::Command::new("winget");
            c.args(["install", "--id", "OpenJS.NodeJS.LTS", "-e", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"]);
            c.output().map_err(|e| e.to_string())
        })
        .await;
        if let Ok(out) = winget {
            if out.status.success() {
                audit("node installed", serde_json::json!({ "with": "winget" }));
                return Ok("installed".into());
            }
        }
    }
    crate::platform::open_url("https://nodejs.org/en/download");
    Ok("opened".into())
}

// ── Projects ─────────────────────────────────────────────────────────────────

fn recent_file() -> PathBuf {
    crate::platform::local_dir().join("Skills").join("projects.json")
}

fn load_recent() -> Vec<String> {
    std::fs::read_to_string(recent_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember_project(p: &Path) {
    let s = p.to_string_lossy().into_owned();
    let mut v = load_recent();
    v.retain(|x| x != &s);
    v.insert(0, s);
    v.truncate(12);
    let f = recent_file();
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, serde_json::to_string(&v).unwrap_or_default());
}

/// Recent projects, then the Coding Studio's workspaces.
#[tauri::command]
pub fn skills_projects() -> Vec<String> {
    let mut v: Vec<String> = load_recent().into_iter().filter(|p| Path::new(p).is_dir()).collect();
    if let Ok(ws) = crate::studio::studio_workspaces() {
        for w in ws {
            if !v.contains(&w.path) && Path::new(&w.path).is_dir() {
                v.push(w.path);
            }
        }
    }
    v
}

#[tauri::command]
pub async fn skills_pick_project(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(crate::i18n::t("Pick a project")).pick_folder(move |p| {
        let _ = tx.send(p);
    });
    let Some(p) = rx.await.map_err(|e| e.to_string())? else { return Ok(None) };
    let p = super::project_dir(&p.into_path().map_err(|e| e.to_string())?.to_string_lossy())?;
    remember_project(&p);
    Ok(Some(p.to_string_lossy().into_owned()))
}

// ── Listing ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    #[serde(flatten)]
    pub installed: Installed,
    pub description: String,
    pub skill_md: Option<String>,
    /// The user changed it since Groovy installed or updated it.
    pub edited: bool,
    pub allowed_tools: Vec<String>,
}

fn describe(i: Installed, state: &store::ProjectState) -> SkillView {
    let md = Path::new(&i.path).join("SKILL.md");
    let text = std::fs::read_to_string(&md).unwrap_or_default();
    let fm = super::frontmatter(&text).unwrap_or_default();
    let edited = state.hashes.get(&i.path).is_some_and(|h| store::hash_dir(Path::new(&i.path)).as_deref() != Some(h.as_str()));
    SkillView {
        description: fm.description.unwrap_or_default(),
        skill_md: md.is_file().then(|| md.to_string_lossy().into_owned()),
        edited,
        allowed_tools: fm.allowed_tools,
        installed: i,
    }
}

fn list_blocking(project: &Path, scope: Scope) -> Result<Vec<Installed>, String> {
    let out = cli::run(project, &cli::list_args(scope))?;
    cli::parse_list(&out.stdout)
}

#[tauri::command]
pub async fn skills_list(project: String, scope: Option<Scope>) -> Result<Vec<SkillView>, String> {
    let project = super::project_dir(&project)?;
    let scope = scope.unwrap_or_default();
    let p = project.clone();
    let list = blocking(move || list_blocking(&p, scope)).await?;
    let state = store::load(&project);
    Ok(list.into_iter().map(|i| describe(i, &state)).collect())
}

// ── Search and preview ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn skills_search(query: String) -> Result<Vec<Candidate>, String> {
    let q: String = query.trim().chars().take(100).collect();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    recommend::search(&q, 20).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub source: String,
    pub skill: String,
    /// Where the SKILL.md was read, to show the user.
    pub url: String,
    pub content: String,
    pub frontmatter: super::Frontmatter,
    /// Other files of the skill, and which of them agents may run.
    pub files: Vec<String>,
    pub scripts: Vec<String>,
    pub installs: Option<u64>,
    /// Hand back to skills_install: proof the user was shown this.
    pub token: String,
}

/// Tokens of the previews shown, by token: (source, skill).
static PREVIEWS: Mutex<Option<HashMap<String, (String, String)>>> = Mutex::new(None);

fn remember_preview(source: &str, skill: &str, content: &str) -> String {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    ctx.update(source.as_bytes());
    ctx.update(&[0]);
    ctx.update(skill.as_bytes());
    ctx.update(&[0]);
    ctx.update(content.as_bytes());
    ctx.update(&store::now_ms().to_le_bytes());
    let token = super::hex(ctx.finish().as_ref());
    let mut map = PREVIEWS.lock().unwrap();
    let map = map.get_or_insert_with(HashMap::new);
    if map.len() > 200 {
        map.clear();
    }
    map.insert(token.clone(), (source.to_string(), skill.to_string()));
    token
}

fn previewed(token: &str, source: &str, skill: &str) -> bool {
    PREVIEWS.lock().unwrap().as_ref().and_then(|m| m.get(token)).is_some_and(|(s, k)| s == source && k == skill)
}

/// "owner/repo", "https://github.com/owner/repo(.git)", ".../tree/<ref>/<path>" →
/// (owner, repo, ref, subpath).
pub fn github_source(source: &str) -> Option<(String, String, String, String)> {
    let s = source.trim().trim_end_matches('/');
    let rest = s.strip_prefix("https://github.com/").or_else(|| s.strip_prefix("github.com/")).unwrap_or(s);
    if rest.contains("://") || rest.starts_with('.') || rest.starts_with('/') || rest.contains('\\') {
        return None;
    }
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return None;
    }
    let ok = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let owner = parts[0];
    let repo = parts[1].trim_end_matches(".git");
    if !ok(owner) || !ok(repo) {
        return None;
    }
    if parts.len() == 2 {
        return Some((owner.into(), repo.into(), "HEAD".into(), String::new()));
    }
    if parts.len() >= 4 && parts[2] == "tree" && ok(parts[3]) {
        return Some((owner.into(), repo.into(), parts[3].into(), parts[4..].join("/")));
    }
    None
}

/// In a repository's file list, the folder of `skill` (the one whose SKILL.md
/// sits in a folder named after it, or whose frontmatter we can't see yet: the
/// best path match), under `sub`.
pub fn find_skill_dir(paths: &[String], skill: &str, sub: &str) -> Option<String> {
    let sub = sub.trim_matches('/');
    let mut best: Option<&String> = None;
    for p in paths.iter().filter(|p| p.ends_with("SKILL.md")) {
        if !sub.is_empty() && !p.starts_with(sub) {
            continue;
        }
        let dir = p.trim_end_matches("SKILL.md").trim_end_matches('/');
        let name = dir.rsplit('/').next().unwrap_or("");
        if name == skill {
            if best.is_none_or(|b| p.len() < b.len()) {
                best = Some(p);
            }
        } else if dir == sub && best.is_none() {
            // A source that points at one skill's folder.
            best = Some(p);
        }
    }
    best.map(|p| p.trim_end_matches("SKILL.md").trim_end_matches('/').to_string())
}

async fn github_preview(owner: &str, repo: &str, reference: &str, sub: &str, skill: &str) -> Result<(String, String, Vec<String>), String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let auth = |r: reqwest::RequestBuilder| match crate::secrets::get("github-token").filter(|t| !t.trim().is_empty()) {
        Some(t) => r.bearer_auth(t),
        None => r,
    };
    let tree_url = format!("https://api.github.com/repos/{owner}/{repo}/git/trees/{reference}?recursive=1");
    let resp = auth(client.get(&tree_url).header("User-Agent", "Groovy").header("Accept", "application/vnd.github+json"))
        .send()
        .await
        .map_err(|e| format!("GitHub could not be reached: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub answered {} for {owner}/{repo}.", resp.status()));
    }
    let tree: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let paths: Vec<String> = tree["tree"]
        .as_array()
        .map(|a| a.iter().filter(|e| e["type"] == "blob").filter_map(|e| e["path"].as_str().map(String::from)).collect())
        .unwrap_or_default();
    let dir = find_skill_dir(&paths, skill, sub).ok_or_else(|| format!("No skill named {skill} in {owner}/{repo}."))?;
    let md_path = if dir.is_empty() { "SKILL.md".to_string() } else { format!("{dir}/SKILL.md") };
    let raw = format!("https://raw.githubusercontent.com/{owner}/{repo}/{reference}/{md_path}");
    let resp = auth(client.get(&raw).header("User-Agent", "Groovy")).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub answered {} for {md_path}.", resp.status()));
    }
    let content = resp.text().await.map_err(|e| e.to_string())?;
    if content.len() > 1_000_000 {
        return Err("This SKILL.md is too large to show.".into());
    }
    let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
    let files: Vec<String> = paths.iter().filter_map(|p| p.strip_prefix(&prefix).map(String::from)).filter(|p| p != "SKILL.md").take(200).collect();
    Ok((format!("https://github.com/{owner}/{repo}/blob/{reference}/{md_path}"), content, files))
}

fn local_preview(source: &str, skill: &str) -> Result<(String, String, Vec<String>), String> {
    let root = PathBuf::from(source);
    if !root.is_absolute() || !root.is_dir() {
        return Err("Only GitHub sources and local folders can be previewed.".into());
    }
    let candidates = [root.join(skill).join("SKILL.md"), root.join("skills").join(skill).join("SKILL.md"), root.join(".claude/skills").join(skill).join("SKILL.md"), root.join("SKILL.md")];
    let md = candidates.iter().find(|p| p.is_file()).ok_or_else(|| format!("No skill named {skill} in {source}."))?;
    let dir = md.parent().unwrap();
    let content = std::fs::read_to_string(md).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if rel != "SKILL.md" && files.len() < 200 {
                    files.push(rel);
                }
            }
        }
    }
    files.sort();
    Ok((md.to_string_lossy().into_owned(), content, files))
}

#[tauri::command]
pub async fn skills_preview(source: String, skill: String, installs: Option<u64>) -> Result<Preview, String> {
    if !cli::valid_source(&source) || !cli::valid_skill(&skill) {
        return Err("Not a valid skill.".into());
    }
    let (url, content, files) = match github_source(&source) {
        Some((owner, repo, reference, sub)) => github_preview(&owner, &repo, &reference, &sub, &skill).await?,
        None => {
            let (s, k) = (source.clone(), skill.clone());
            blocking(move || local_preview(&s, &k)).await?
        }
    };
    let frontmatter = super::frontmatter(&content).unwrap_or_default();
    let scripts = files.iter().filter(|f| super::is_script(f)).cloned().collect();
    let token = remember_preview(&source, &skill, &content);
    Ok(Preview { source, skill, url, content, frontmatter, files, scripts, installs, token })
}

// ── Install, remove, update ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub project: String,
    pub source: String,
    pub skills: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub scope: Scope,
    /// One per skill, from skills_preview.
    pub tokens: Vec<String>,
}

fn record_hashes(project: &Path, state: &mut store::ProjectState, scope: Scope, names: &[String]) {
    if let Ok(list) = list_blocking(project, scope) {
        for i in list.into_iter().filter(|i| names.contains(&i.name)) {
            if let Some(h) = store::hash_dir(Path::new(&i.path)) {
                state.hashes.insert(i.path, h);
            }
        }
    }
}

#[tauri::command]
pub async fn skills_install(app: AppHandle, request: InstallRequest) -> Result<Vec<cli::AddResult>, String> {
    let project = super::project_dir(&request.project)?;
    if request.tokens.len() != request.skills.len()
        || !request.skills.iter().zip(&request.tokens).all(|(s, t)| previewed(t, &request.source, s))
    {
        return Err("Look at each skill before installing it.".into());
    }
    let agents = if request.agents.is_empty() { vec!["claude-code".to_string()] } else { request.agents.clone() };
    let args = cli::add_args(&request.source, &request.skills, &agents, request.scope, cfg!(windows))?;
    let p = project.clone();
    let req = request.clone();
    let results = blocking(move || {
        let out = cli::run(&p, &args)?;
        let results = cli::parse_add(&out.stdout)?;
        let installed: Vec<String> = results.iter().filter(|r| r.status == "installed").filter_map(|r| r.name.clone()).collect();
        let mut state = store::load(&p);
        record_hashes(&p, &mut state, req.scope, &installed);
        if !installed.is_empty() {
            state.record(HistoryEntry {
                at: store::now_ms(), action: "install".into(), skills: installed, source: Some(req.source.clone()),
                agents: agents.clone(), global: req.scope == Scope::Global, backups: vec![], previous: None, undone: false,
            });
        }
        store::save(&p, &state)?;
        Ok(results)
    })
    .await?;
    remember_project(&project);
    audit("skills installed", serde_json::json!({ "project": project, "source": request.source, "skills": request.skills }));
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(results)
}

fn remove_blocking(project: &Path, skills: &[String], agents: &[String], scope: Scope, record: bool) -> Result<Vec<(String, String)>, String> {
    let args = cli::remove_args(skills, agents, scope)?;
    // Back up first, so undo works.
    let list = list_blocking(project, scope)?;
    let stamp = store::now_ms();
    let mut backups = Vec::new();
    for i in list.iter().filter(|i| skills.contains(&i.name)) {
        let to = store::trash(project).join(stamp.to_string()).join(&i.name);
        store::copy_dir(Path::new(&i.path), &to)?;
        backups.push((to.to_string_lossy().into_owned(), i.path.clone()));
    }
    cli::run(project, &args)?;
    let mut state = store::load(project);
    for (_, original) in &backups {
        state.hashes.remove(original);
    }
    if record {
        state.record(HistoryEntry {
            at: stamp, action: "remove".into(), skills: skills.to_vec(), source: None, agents: agents.to_vec(),
            global: scope == Scope::Global, backups: backups.clone(), previous: None, undone: false,
        });
    }
    store::save(project, &state)?;
    Ok(backups)
}

#[tauri::command]
pub async fn skills_remove(app: AppHandle, project: String, skills: Vec<String>, agents: Option<Vec<String>>, scope: Option<Scope>) -> Result<(), String> {
    let project = super::project_dir(&project)?;
    let (p, s) = (project.clone(), skills.clone());
    blocking(move || remove_blocking(&p, &s, &agents.unwrap_or_default(), scope.unwrap_or_default(), true).map(|_| ())).await?;
    audit("skills removed", serde_json::json!({ "project": project, "skills": skills }));
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub updated: Vec<String>,
    /// Skipped: the user edited them (or Groovy never saw them unedited).
    pub kept: Vec<String>,
    pub output: String,
}

/// Updates the project's skills, except those the user edited unless they're
/// listed in `overwrite`.
#[tauri::command]
pub async fn skills_update(app: AppHandle, project: String, overwrite: Option<Vec<String>>, scope: Option<Scope>) -> Result<UpdateReport, String> {
    let project = super::project_dir(&project)?;
    let scope = scope.unwrap_or_default();
    let overwrite = overwrite.unwrap_or_default();
    let p = project.clone();
    let report = blocking(move || {
        let list = list_blocking(&p, scope)?;
        let mut state = store::load(&p);
        let (mut go, mut kept) = (Vec::new(), Vec::new());
        for i in list.iter().filter(|i| i.source.is_some()) {
            let check = store::update_check(state.hashes.get(&i.path).map(String::as_str), store::hash_dir(Path::new(&i.path)).as_deref());
            if check == UpdateCheck::Clean || overwrite.contains(&i.name) {
                go.push(i.name.clone());
            } else {
                kept.push(i.name.clone());
            }
        }
        if go.is_empty() {
            return Ok(UpdateReport { updated: vec![], kept, output: String::new() });
        }
        let out = cli::run(&p, &cli::update_args(&go, scope)?)?;
        record_hashes(&p, &mut state, scope, &go);
        state.record(HistoryEntry {
            at: store::now_ms(), action: "update".into(), skills: go.clone(), source: None, agents: vec![],
            global: scope == Scope::Global, backups: vec![], previous: None, undone: false,
        });
        store::save(&p, &state)?;
        Ok(UpdateReport { updated: go, kept, output: cli::strip_ansi(&out.stdout).chars().take(4000).collect() })
    })
    .await?;
    audit("skills updated", serde_json::json!({ "project": project, "updated": report.updated, "kept": report.kept }));
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(report)
}

/// Undoes the last install, removal or save.
#[tauri::command]
pub async fn skills_undo(app: AppHandle, project: String) -> Result<String, String> {
    let project = super::project_dir(&project)?;
    let p = project.clone();
    let what = blocking(move || {
        let mut state = store::load(&p);
        let Some(entry) = state.last_undoable().cloned() else { return Err("Nothing to undo.".to_string()) };
        let scope = if entry.global { Scope::Global } else { Scope::Project };
        match entry.action.as_str() {
            "install" => {
                remove_blocking(&p, &entry.skills, &entry.agents, scope, false)?;
            }
            "remove" => {
                for (backup, original) in &entry.backups {
                    store::copy_dir(Path::new(backup), Path::new(original))?;
                }
            }
            "save" => {
                let (file, previous) = entry.previous.clone().ok_or("Nothing to undo.")?;
                let path = super::editable_path(&file, Some(&p.to_string_lossy()))?;
                std::fs::write(path, previous).map_err(|e| e.to_string())?;
            }
            _ => return Err("Nothing to undo.".into()),
        }
        let mut state2 = store::load(&p);
        if let Some(e) = state2.history.iter_mut().rev().find(|h| h.at == entry.at && h.action == entry.action) {
            e.undone = true;
        }
        state = state2;
        store::save(&p, &state)?;
        Ok(entry.action)
    })
    .await?;
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(what)
}

/// A new skill in the project (`<project>/.claude/skills/<name>/SKILL.md`).
#[tauri::command]
pub async fn skills_init(app: AppHandle, project: String, name: String) -> Result<String, String> {
    let project = super::project_dir(&project)?;
    let args = cli::init_args(&name)?;
    let dir = project.join(".claude").join("skills");
    let target = dir.join(&name).join("SKILL.md");
    if target.exists() {
        return Err(format!("{name} already exists."));
    }
    let d = dir.clone();
    blocking(move || {
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        cli::run(&d, &args).map(|_| ())
    })
    .await?;
    if !target.is_file() {
        return Err("The skills CLI did not create the skill.".into());
    }
    let mut state = store::load(&project);
    state.record(HistoryEntry {
        at: store::now_ms(), action: "create".into(), skills: vec![name], source: None, agents: vec![], global: false,
        backups: vec![], previous: None, undone: false,
    });
    store::save(&project, &state)?;
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(target.to_string_lossy().into_owned())
}

/// A customised copy of an installed skill, as the project's own
/// (`.claude/skills/<name>-groovy`): updates no longer touch it.
#[tauri::command]
pub async fn skills_fork(app: AppHandle, project: String, skill: String, scope: Option<Scope>) -> Result<String, String> {
    let project = super::project_dir(&project)?;
    if !cli::valid_skill(&skill) {
        return Err("Not a valid skill.".into());
    }
    let p = project.clone();
    let path = blocking(move || {
        let list = list_blocking(&p, scope.unwrap_or_default())?;
        let i = list.iter().find(|i| i.name == skill).ok_or("This skill is not installed.")?;
        let name = format!("{skill}-groovy");
        let to = p.join(".claude").join("skills").join(&name);
        store::copy_dir(Path::new(&i.path), &to)?;
        let md = to.join("SKILL.md");
        if let Ok(text) = std::fs::read_to_string(&md) {
            let renamed = text.replacen(&format!("name: {skill}"), &format!("name: {name}"), 1);
            std::fs::write(&md, renamed).map_err(|e| e.to_string())?;
        }
        let mut state = store::load(&p);
        state.record(HistoryEntry {
            at: store::now_ms(), action: "fork".into(), skills: vec![name], source: i.source.clone(), agents: vec![],
            global: false, backups: vec![], previous: None, undone: false,
        });
        store::save(&p, &state)?;
        Ok(md.to_string_lossy().into_owned())
    })
    .await?;
    let _ = app.emit("skills-changed", project.to_string_lossy());
    Ok(path)
}

// ── Editing ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn skills_read(path: String, project: Option<String>) -> Result<String, String> {
    let p = super::editable_path(&path, project.as_deref())?;
    std::fs::read_to_string(p).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn skills_save(app: AppHandle, path: String, content: String, project: Option<String>) -> Result<(), String> {
    let p = super::editable_path(&path, project.as_deref())?;
    if p.file_name().is_some_and(|n| n == "SKILL.md") {
        super::check_skill_md(&content)?;
    }
    if content.len() > 1_000_000 {
        return Err("This file is too large.".into());
    }
    let previous = std::fs::read_to_string(&p).unwrap_or_default();
    std::fs::write(&p, &content).map_err(|e| e.to_string())?;
    if let Some(project) = project.as_deref().map(super::project_dir).transpose()? {
        let mut state = store::load(&project);
        state.record(HistoryEntry {
            at: store::now_ms(), action: "save".into(), skills: vec![], source: None, agents: vec![], global: false,
            backups: vec![], previous: Some((p.to_string_lossy().into_owned(), previous)), undone: false,
        });
        store::save(&project, &state)?;
        let _ = app.emit("skills-changed", project.to_string_lossy());
    }
    Ok(())
}

// ── Analysis and recommendations ─────────────────────────────────────────────

#[tauri::command]
pub async fn skills_analyze(project: String) -> Result<ProjectProfile, String> {
    let project = super::project_dir(&project)?;
    remember_project(&project);
    blocking(move || Ok(analyze::analyze(&project))).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationsView {
    #[serde(flatten)]
    pub recs: Recommendations,
    pub dismissed: Vec<String>,
    pub can_undo: bool,
    pub ai: bool,
}

async fn compute(project: &Path) -> Result<Recommendations, String> {
    let p = project.to_path_buf();
    let (profile, installed) = blocking(move || {
        let profile = analyze::analyze(&p);
        let list = list_blocking(&p, Scope::Project).unwrap_or_default();
        let installed: Vec<(Installed, String)> = list
            .into_iter()
            .map(|i| {
                let text = std::fs::read_to_string(Path::new(&i.path).join("SKILL.md")).unwrap_or_default();
                let d = super::frontmatter(&text).and_then(|f| f.description).unwrap_or_default();
                (i, d)
            })
            .collect();
        Ok((profile, installed))
    })
    .await?;
    let recs = recommend::recommend(&profile, &installed).await;
    let mut state = store::load(project);
    state.profile_hash = profile.hash.clone();
    state.recommendations = Some(recs.clone());
    store::save(project, &state)?;
    Ok(recs)
}

/// The saved recommendations, or new ones when `refresh` or the project's
/// manifests changed.
#[tauri::command]
pub async fn skills_recommendations(project: String, refresh: Option<bool>) -> Result<RecommendationsView, String> {
    let project = super::project_dir(&project)?;
    let mut state = store::load(&project);
    let p = project.clone();
    let hash = blocking(move || Ok(analyze::manifest_hash(&p))).await?;
    let recs = match (&state.recommendations, refresh.unwrap_or(false) || state.profile_hash != hash) {
        (Some(r), false) => r.clone(),
        _ => {
            let r = compute(&project).await?;
            state = store::load(&project);
            r
        }
    };
    let can_undo = state.clone().last_undoable().is_some();
    Ok(RecommendationsView { recs, dismissed: state.dismissed, can_undo, ai: crate::brain_ai::available() })
}

#[tauri::command]
pub fn skills_dismiss(project: String, id: String) -> Result<(), String> {
    let project = super::project_dir(&project)?;
    let mut state = store::load(&project);
    if !state.dismissed.contains(&id) {
        state.dismissed.push(id);
    }
    store::save(&project, &state)
}

/// An agent session started in `cwd` (hooks): when Groovy already manages this
/// project and its manifests changed, analyse it again and, for new confident
/// recommendations, show a quiet bubble — never during a dictation or a call.
#[tauri::command]
pub async fn skills_session_started(app: AppHandle, cwd: String) -> Result<(), String> {
    let Ok(project) = super::project_dir(&cwd) else { return Ok(()) };
    if !store::exists(&project) {
        return Ok(());
    }
    let state = store::load(&project);
    let p = project.clone();
    let hash = blocking(move || Ok(analyze::manifest_hash(&p))).await?;
    if hash == state.profile_hash {
        return Ok(());
    }
    let recs = compute(&project).await?;
    let mut state = store::load(&project);
    let fresh: Vec<String> = recs
        .install
        .iter()
        .filter(|r| r.confidence >= 0.7)
        .map(|r| r.id())
        .filter(|id| !state.dismissed.contains(id) && !state.announced.contains(id))
        .collect();
    if fresh.is_empty() || crate::flow::live::dictating(&app) || crate::notes::is_recording(&app) {
        return Ok(());
    }
    let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let text = crate::i18n::tf("{0} skills could help {1}", &[("0", &fresh.len().to_string()), ("1", &name)]);
    crate::flow::live::announce(&app, "bolt", "#22D3EE", text, Some((crate::i18n::t("Review"), "openSkills")), 10);
    state.announced.extend(fresh);
    store::save(&project, &state)
}

// ── « OK Groovy » ────────────────────────────────────────────────────────────

/// The `skills` voice tool. Searching, listing and recommending answer at once;
/// install and remove never act from the voice: the Control Centre opens on
/// the skill, with its SKILL.md, for the user to confirm with a click.
pub fn voice_tool(app: &AppHandle, input: &serde_json::Value) -> (String, bool) {
    let action = input["action"].as_str().unwrap_or("");
    let project = skills_projects().into_iter().next();
    match action {
        "search" => {
            let q = input["query"].as_str().unwrap_or("").trim();
            match tauri::async_runtime::block_on(recommend::search(q, 5)) {
                Ok(found) if !found.is_empty() => (
                    found.iter().map(|c| format!("{} from {} ({} installs)", c.name, c.source, c.installs)).collect::<Vec<_>>().join("; "),
                    false,
                ),
                Ok(_) => ("No skill found for that.".into(), false),
                Err(e) => (e, false),
            }
        }
        "list" | "recommend" => {
            let Some(project) = project else {
                return ("No project picked yet: the user picks one in the Groovy window, under Skills.".into(), false);
            };
            if action == "list" {
                return match tauri::async_runtime::block_on(skills_list(project.clone(), None)) {
                    Ok(list) if !list.is_empty() => (format!("In {project}: {}", list.iter().map(|s| s.installed.name.as_str()).collect::<Vec<_>>().join(", ")), false),
                    Ok(_) => (format!("No skills installed in {project}."), false),
                    Err(e) => (e, false),
                };
            }
            match tauri::async_runtime::block_on(skills_recommendations(project.clone(), None)) {
                Ok(v) => {
                    let install: Vec<String> = v.recs.install.iter().filter(|r| !v.dismissed.contains(&r.id())).map(|r| format!("{} ({})", r.skill, r.reason)).collect();
                    let remove: Vec<String> = v.recs.remove.iter().map(|r| r.skill.clone()).collect();
                    (format!("For {project}: could help: {}. Could go: {}.", if install.is_empty() { "nothing new".into() } else { install.join("; ") }, if remove.is_empty() { "none".into() } else { remove.join(", ") }), false)
                }
                Err(e) => (e, false),
            }
        }
        "install" | "remove" => {
            let skill = input["skill"].as_str().unwrap_or("").trim();
            if !cli::valid_skill(skill) {
                return ("Which skill? Ask for its exact name.".into(), false);
            }
            crate::show_hub(app, Some("skills"));
            let _ = app.emit_to(crate::HUB_LABEL, "skills-confirm", serde_json::json!({
                "action": action, "skill": skill, "source": input["source"].as_str().unwrap_or(""), "project": project,
            }));
            ("The Groovy window is open on it: nothing changes until the user confirms there.".into(), true)
        }
        _ => (format!("unknown skills action {action}"), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_sources() {
        assert_eq!(github_source("vercel-labs/skills"), Some(("vercel-labs".into(), "skills".into(), "HEAD".into(), String::new())));
        assert_eq!(
            github_source("https://github.com/anthropics/skills/tree/main/skills/pdf"),
            Some(("anthropics".into(), "skills".into(), "main".into(), "skills/pdf".into()))
        );
        assert_eq!(github_source("https://github.com/o/r.git"), Some(("o".into(), "r".into(), "HEAD".into(), String::new())));
        assert_eq!(github_source("/home/me/skills"), None);
        assert_eq!(github_source("https://gitlab.com/o/r"), None);
        assert_eq!(github_source("o"), None);
    }

    #[test]
    fn skill_folders_are_found_in_a_tree() {
        let paths: Vec<String> = ["README.md", "skills/pdf/SKILL.md", "skills/pdf/scripts/fill.py", "skills/docx/SKILL.md", "examples/pdf/SKILL.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(find_skill_dir(&paths, "pdf", ""), Some("skills/pdf".into()));
        assert_eq!(find_skill_dir(&paths, "docx", ""), Some("skills/docx".into()));
        assert_eq!(find_skill_dir(&paths, "nope", ""), None);
        assert_eq!(find_skill_dir(&paths, "pdf", "examples"), Some("examples/pdf".into()));
        let single = vec!["SKILL.md".to_string()];
        assert_eq!(find_skill_dir(&single, "whatever", ""), Some(String::new()));
    }

    #[test]
    fn installs_need_a_preview() {
        let token = remember_preview("o/r", "x", "---\nname: x\n---");
        assert!(previewed(&token, "o/r", "x"));
        assert!(!previewed(&token, "o/r", "y"));
        assert!(!previewed(&token, "evil/r", "x"));
        assert!(!previewed("made-up", "o/r", "x"));
    }

    #[test]
    fn local_sources_are_previewed_from_disk() {
        let dir = super::super::TestDir::new("preview");
        let skill = dir.path().join("skills/hello");
        std::fs::create_dir_all(skill.join("scripts")).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: hello\ndescription: hi\n---\n").unwrap();
        std::fs::write(skill.join("scripts/run.sh"), "echo").unwrap();
        let (_, content, files) = local_preview(&dir.path().to_string_lossy(), "hello").unwrap();
        assert!(content.contains("name: hello"));
        assert_eq!(files, vec!["scripts/run.sh"]);
        assert!(local_preview("relative/path", "hello").is_err());
    }

    /// The real CLI (needs Node ≥ 22.20 and npm's registry): add a local skill,
    /// list it, remove it with a backup, undo. `cargo test -- --ignored skills_cli`.
    #[test]
    #[ignore]
    fn skills_cli_end_to_end() {
        let src = super::super::TestDir::new("cli-src");
        let skill = src.path().join("skills/hello-groovy");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: hello-groovy\ndescription: Says hello. Use when greeting.\n---\n# Hello\n").unwrap();
        let project = super::super::TestDir::new("cli-project");
        let p = super::super::project_dir(&project.path().to_string_lossy()).unwrap();
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let args = cli::add_args(&src.path().to_string_lossy(), &s(&["hello-groovy"]), &s(&["claude-code"]), Scope::Project, true).unwrap();
        let out = cli::run(&p, &args).unwrap();
        let added = cli::parse_add(&out.stdout).unwrap();
        assert!(added.iter().any(|r| r.status == "installed"), "{added:?}");
        let list = list_blocking(&p, Scope::Project).unwrap();
        let installed = list.iter().find(|i| i.name == "hello-groovy").expect("listed");
        assert!(Path::new(&installed.path).join("SKILL.md").is_file());
        let backups = remove_blocking(&p, &s(&["hello-groovy"]), &[], Scope::Project, true).unwrap();
        assert_eq!(backups.len(), 1);
        assert!(Path::new(&backups[0].0).join("SKILL.md").is_file());
        assert!(!list_blocking(&p, Scope::Project).unwrap().iter().any(|i| i.name == "hello-groovy"));
        let state = store::load(&p);
        assert_eq!(state.history.last().unwrap().action, "remove");
    }
}
