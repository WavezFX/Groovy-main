// The Skills Manager: agent skills (SKILL.md packages) for each project,
// through Vercel's `skills` CLI (cli.rs). Groovy analyses a project
// (analyze.rs), recommends skills and flags ones it may not need
// (recommend.rs), and installs, removes, updates and edits them (manage.rs),
// keeping its own state per project (store.rs).
//
// Nothing installs without the user's approval: an install needs the token of
// a preview the user was shown (the full SKILL.md, its source, its scripts and
// allowed tools). Removed skills go to `.groovy/skills-trash/` first.

pub mod analyze;
pub mod cli;
pub mod manage;
pub mod recommend;
pub mod store;

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── SKILL.md ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    /// `allowed-tools`: what the skill lets an agent run without asking.
    pub allowed_tools: Vec<String>,
}

/// The YAML frontmatter's few keys Groovy cares about (a tolerant reader:
/// `key: value`, quoted values, and `- item` lists).
pub fn frontmatter(text: &str) -> Option<Frontmatter> {
    let text = text.trim_start_matches('\u{feff}');
    let rest = text.strip_prefix("---")?;
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];
    let mut fm = Frontmatter::default();
    let mut in_tools = false;
    let unquote = |v: &str| v.trim().trim_matches(['"', '\'']).trim().to_string();
    for line in block.lines() {
        if in_tools {
            if let Some(item) = line.trim().strip_prefix("- ") {
                fm.allowed_tools.push(unquote(item));
                continue;
            }
            in_tools = false;
        }
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        match key.trim() {
            "name" => fm.name = Some(unquote(value)).filter(|v| !v.is_empty()),
            "description" => fm.description = Some(unquote(value)).filter(|v| !v.is_empty()),
            "allowed-tools" | "allowed_tools" => {
                if value.is_empty() {
                    in_tools = true;
                } else {
                    let v = value.trim_matches(['[', ']']);
                    fm.allowed_tools.extend(v.split(',').map(unquote).filter(|s| !s.is_empty()));
                }
            }
            _ => {}
        }
    }
    Some(fm)
}

/// What the editor refuses to save.
pub fn check_skill_md(text: &str) -> Result<Frontmatter, String> {
    let fm = frontmatter(text).ok_or("SKILL.md must start with a frontmatter block between --- lines.")?;
    if fm.name.is_none() {
        return Err("The frontmatter needs a name.".into());
    }
    if fm.description.is_none() {
        return Err("The frontmatter needs a description: agents use it to decide when to load the skill.".into());
    }
    Ok(fm)
}

/// Files in a skill that agents may run.
pub fn is_script(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.starts_with("scripts/")
        || [".sh", ".bash", ".ps1", ".bat", ".cmd", ".py", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl", ".exe"].iter().any(|e| lower.ends_with(e))
}

// ── Paths ─────────────────────────────────────────────────────────────────────

/// A project folder from the page: absolute, existing, a directory.
pub fn project_dir(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path.trim());
    if !p.is_absolute() {
        return Err("Pick a project folder.".into());
    }
    let p = p.canonicalize().map_err(|_| format!("{path} was not found."))?;
    if !p.is_dir() {
        return Err(format!("{path} is not a folder."));
    }
    Ok(strip_verbatim(p))
}

/// `\\?\C:\x` → `C:\x`: what canonicalize gives on Windows confuses child processes.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

/// Whether `file` is a Markdown file inside a `skills` folder, in `project`
/// or in the user's home: the only files the editor may read or write.
pub fn editable(file: &Path, project: Option<&Path>, home: &Path) -> bool {
    if file.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    if !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
        return false;
    }
    let in_skills = file.components().any(|c| c.as_os_str() == "skills");
    let under = |root: &Path| file.starts_with(root);
    in_skills && (project.is_some_and(under) || under(home))
}

/// The real path of a file the editor works on, checked.
pub fn editable_path(path: &str, project: Option<&str>) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    let real = if p.exists() { strip_verbatim(p.canonicalize().map_err(|e| e.to_string())?) } else { p };
    let project = project.map(project_dir).transpose()?;
    let home = strip_verbatim(crate::platform::home_dir().canonicalize().unwrap_or_else(|_| crate::platform::home_dir()));
    if !editable(&real, project.as_deref(), &home) {
        return Err("Only the Markdown files of a skill can be opened here.".into());
    }
    Ok(real)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub struct TestDir(PathBuf);

#[cfg(test)]
impl TestDir {
    pub fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("groovy-skills-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_read() {
        let md = "---\nname: frontend-design\ndescription: \"Build UIs: pretty ones\"\nallowed-tools:\n  - Bash(npm run *)\n  - Read\nlicense: MIT\n---\n# Body\n";
        let fm = frontmatter(md).unwrap();
        assert_eq!(fm.name.as_deref(), Some("frontend-design"));
        assert_eq!(fm.description.as_deref(), Some("Build UIs: pretty ones"));
        assert_eq!(fm.allowed_tools, vec!["Bash(npm run *)", "Read"]);
        let inline = frontmatter("---\nname: x\nallowed-tools: [Bash, Write]\n---\n").unwrap();
        assert_eq!(inline.allowed_tools, vec!["Bash", "Write"]);
        assert!(frontmatter("# no frontmatter").is_none());
    }

    #[test]
    fn the_editor_checks_what_it_saves() {
        assert!(check_skill_md("---\nname: x\ndescription: does y\n---\nbody").is_ok());
        assert!(check_skill_md("---\nname: x\n---\nbody").is_err());
        assert!(check_skill_md("body").is_err());
    }

    #[test]
    fn scripts_are_flagged() {
        assert!(is_script("scripts/setup.md"));
        assert!(is_script("tools/run.PS1"));
        assert!(!is_script("SKILL.md"));
        assert!(!is_script("reference/api.md"));
    }

    #[test]
    fn only_skill_markdown_is_editable() {
        let home = Path::new("/home/me");
        let project = Path::new("/work/app");
        assert!(editable(Path::new("/work/app/.claude/skills/x/SKILL.md"), Some(project), home));
        assert!(editable(Path::new("/home/me/.claude/skills/x/SKILL.md"), None, home));
        assert!(!editable(Path::new("/work/app/.claude/skills/x/run.sh"), Some(project), home));
        assert!(!editable(Path::new("/work/app/README.md"), Some(project), home));
        assert!(!editable(Path::new("/etc/skills/x.md"), Some(project), home));
        assert!(!editable(Path::new("/work/app/.claude/skills/../../../etc/x.md"), Some(project), home));
    }
}
