// Groovy's own state for a project's skills, in `<project>/.groovy/skills.json`:
// the last recommendations and what was dismissed, the history of installs and
// removals (for undo; removed skills are moved to `.groovy/skills-trash/`
// first), and the hash of every skill's files when Groovy last installed or
// updated it, so an update never overwrites the user's own edits.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::recommend::Recommendations;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectState {
    pub profile_hash: String,
    pub recommendations: Option<Recommendations>,
    /// Ids ("install:<source>@<skill>", "remove:<skill>") the user said no to.
    pub dismissed: Vec<String>,
    pub history: Vec<HistoryEntry>,
    /// Skill folder → SHA-256 of its files when Groovy last wrote it.
    pub hashes: BTreeMap<String, String>,
    /// Ids already shown in the pill, so a bubble is never repeated.
    pub announced: Vec<String>,
    /// Off by default: install from the trusted owners below without asking.
    pub auto_approve: bool,
    pub trusted_owners: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub at: u64,
    /// "install", "remove", "update", "save", "fork", "create"
    pub action: String,
    pub skills: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub global: bool,
    /// Removed: where each skill's folder was copied before (trash path, original path).
    #[serde(default)]
    pub backups: Vec<(String, String)>,
    /// Saved: the file and its previous content.
    #[serde(default)]
    pub previous: Option<(String, String)>,
    #[serde(default)]
    pub undone: bool,
}

pub fn dir(project: &Path) -> PathBuf {
    project.join(".groovy")
}

fn file(project: &Path) -> PathBuf {
    dir(project).join("skills.json")
}

pub fn trash(project: &Path) -> PathBuf {
    dir(project).join("skills-trash")
}

/// Whether Groovy has been used on this project (it then may watch it).
pub fn exists(project: &Path) -> bool {
    file(project).is_file()
}

pub fn load(project: &Path) -> ProjectState {
    std::fs::read_to_string(file(project)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(project: &Path, state: &ProjectState) -> Result<(), String> {
    std::fs::create_dir_all(dir(project)).map_err(|e| e.to_string())?;
    // The trash and the state are Groovy's, not the project's.
    let ignore = dir(project).join(".gitignore");
    if !ignore.exists() {
        let _ = std::fs::write(&ignore, "skills-trash/\n");
    }
    let tmp = file(project).with_extension("json.tmp");
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file(project)).map_err(|e| e.to_string())
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl ProjectState {
    pub fn record(&mut self, entry: HistoryEntry) {
        self.history.push(entry);
        let len = self.history.len();
        if len > 50 {
            self.history.drain(..len - 50);
        }
    }

    /// The last change that can still be undone.
    pub fn last_undoable(&mut self) -> Option<&mut HistoryEntry> {
        self.history.iter_mut().rev().find(|h| !h.undone && matches!(h.action.as_str(), "install" | "remove" | "save"))
    }
}

// ── Edit protection ──────────────────────────────────────────────────────────

/// SHA-256 over every file in a skill's folder (paths and contents, sorted).
pub fn hash_dir(dir: &Path) -> Option<String> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).ok()?.flatten() {
            let p = e.path();
            let ft = e.file_type().ok()?;
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    for f in files {
        let rel = f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        ctx.update(rel.as_bytes());
        ctx.update(&[0]);
        ctx.update(&std::fs::read(&f).ok()?);
        ctx.update(&[0]);
    }
    Some(super::hex(ctx.finish().as_ref()))
}

/// What an update may do with one skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateCheck {
    /// Unchanged since Groovy wrote it: safe to update.
    Clean,
    /// The user changed it: skip unless they say otherwise.
    Edited,
    /// Groovy never recorded it (installed by hand or before): treated as edited.
    Unknown,
}

pub fn update_check(recorded: Option<&str>, current: Option<&str>) -> UpdateCheck {
    match (recorded, current) {
        (Some(r), Some(c)) if r == c => UpdateCheck::Clean,
        (Some(_), _) => UpdateCheck::Edited,
        (None, _) => UpdateCheck::Unknown,
    }
}

/// Copies a folder (a skill) to `to`, which must not exist yet.
pub fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(format!("{} already exists.", to.display()));
    }
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let ft = e.file_type().map_err(|e| e.to_string())?;
        let target = to.join(e.file_name());
        if ft.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_and_keeps_its_history_short() {
        let dir = super::super::TestDir::new("store");
        let mut s = load(dir.path());
        assert_eq!(s, ProjectState::default());
        for i in 0..60 {
            s.record(HistoryEntry {
                at: i, action: "install".into(), skills: vec![format!("s{i}")], source: Some("o/r".into()),
                agents: vec![], global: false, backups: vec![], previous: None, undone: false,
            });
        }
        s.dismissed.push("install:o/r@x".into());
        save(dir.path(), &s).unwrap();
        let back = load(dir.path());
        assert_eq!(back.history.len(), 50);
        assert_eq!(back.history[0].skills, vec!["s10"]);
        assert_eq!(back.dismissed, vec!["install:o/r@x"]);
        assert!(exists(dir.path()));
        assert!(dir.path().join(".groovy/.gitignore").is_file());
    }

    #[test]
    fn edits_are_protected_from_updates() {
        let dir = super::super::TestDir::new("hash");
        let skill = dir.path().join("x");
        std::fs::create_dir_all(skill.join("scripts")).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: x\n---\nhi").unwrap();
        std::fs::write(skill.join("scripts/run.sh"), "echo").unwrap();
        let recorded = hash_dir(&skill).unwrap();
        assert_eq!(update_check(Some(&recorded), hash_dir(&skill).as_deref()), UpdateCheck::Clean);
        std::fs::write(skill.join("SKILL.md"), "---\nname: x\n---\nmine").unwrap();
        assert_eq!(update_check(Some(&recorded), hash_dir(&skill).as_deref()), UpdateCheck::Edited);
        assert_eq!(update_check(None, hash_dir(&skill).as_deref()), UpdateCheck::Unknown);
        // A renamed file is a change too.
        std::fs::write(skill.join("SKILL.md"), "---\nname: x\n---\nhi").unwrap();
        std::fs::rename(skill.join("scripts/run.sh"), skill.join("scripts/go.sh")).unwrap();
        assert_eq!(update_check(Some(&recorded), hash_dir(&skill).as_deref()), UpdateCheck::Edited);
    }

    #[test]
    fn undo_finds_the_last_change() {
        let mut s = ProjectState::default();
        let e = |action: &str| HistoryEntry {
            at: 0, action: action.into(), skills: vec![], source: None, agents: vec![], global: false, backups: vec![], previous: None, undone: false,
        };
        s.record(e("install"));
        s.record(e("remove"));
        s.record(e("update"));
        assert_eq!(s.last_undoable().unwrap().action, "remove");
        s.last_undoable().unwrap().undone = true;
        assert_eq!(s.last_undoable().unwrap().action, "install");
    }
}
