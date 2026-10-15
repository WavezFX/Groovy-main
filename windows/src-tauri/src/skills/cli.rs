// Runs Vercel's `skills` CLI (MIT, pinned to 1.7.2) as a child process: no
// shell, the argument list built here, telemetry off, the project as the
// working directory, a timeout, stderr kept for the error. Only an optional
// GITHUB_TOKEN from Groovy's secret store is passed on, for private repos.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const CLI_VERSION: &str = "1.7.2";
/// `engines.node` of skills 1.7.2.
pub const NODE_MIN: (u32, u32) = (22, 20);
const TIMEOUT: Duration = Duration::from_secs(240);

// ── Node ──────────────────────────────────────────────────────────────────────

/// "v22.20.1" → (22, 20).
pub fn parse_node_version(text: &str) -> Option<(u32, u32)> {
    let v = text.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").trim().parse().ok()?;
    Some((major, minor))
}

pub fn node_new_enough(v: (u32, u32)) -> bool {
    v >= NODE_MIN
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE".into()).split(';').map(|e| e.to_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // Node's own installer on Windows, when PATH was not refreshed yet.
    if cfg!(windows) && name == "node" {
        let pf = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
        let p = pf.join("nodejs").join("node.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn node_path() -> Option<PathBuf> {
    which("node")
}

/// npm's `npx-cli.js` next to `node`, so npx runs through node.exe directly
/// (a `.cmd` would go through cmd.exe and its quoting rules).
fn npx_cli(node: &Path) -> Option<PathBuf> {
    let dir = node.parent()?;
    [
        dir.join("node_modules/npm/bin/npx-cli.js"),
        dir.join("../lib/node_modules/npm/bin/npx-cli.js"),
        dir.join("../node_modules/npm/bin/npx-cli.js"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// `node --version`, None when Node is not installed.
pub fn node_version() -> Option<String> {
    let node = node_path()?;
    let mut cmd = Command::new(node);
    cmd.arg("--version");
    hide_window(&mut cmd);
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_window(_: &mut Command) {}

// ── Arguments ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Project,
    Global,
}

/// A skill name as the CLI writes them (sanitizeName: a-z 0-9 . _ -).
pub fn valid_skill(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with(['-', '.'])
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// An agent id from the CLI's agents.ts ("claude-code", "codex"…).
pub fn valid_agent(id: &str) -> bool {
    !id.is_empty() && id.len() <= 60 && !id.starts_with('-') && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A source: owner/repo, a git or https URL, or a local folder. Never an
/// option, never spaces or control characters.
pub fn valid_source(src: &str) -> bool {
    !src.is_empty()
        && src.len() <= 400
        && !src.starts_with('-')
        && !src.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '`' | '|' | ';' | '&' | '<' | '>' | '$'))
}

fn check<'a>(items: &'a [String], ok: fn(&str) -> bool, what: &str) -> Result<&'a [String], String> {
    match items.iter().find(|i| !ok(i)) {
        Some(bad) => Err(format!("Not a valid {what}: {bad}")),
        None => Ok(items),
    }
}

fn scope_args(args: &mut Vec<String>, scope: Scope) {
    if scope == Scope::Global {
        args.push("-g".into());
    }
}

pub fn list_args(scope: Scope) -> Vec<String> {
    let mut a = vec!["list".into(), "--json".into()];
    scope_args(&mut a, scope);
    a
}

pub fn add_args(source: &str, skills: &[String], agents: &[String], scope: Scope, windows: bool) -> Result<Vec<String>, String> {
    if !valid_source(source) {
        return Err(format!("Not a valid source: {source}"));
    }
    if skills.is_empty() {
        return Err("Pick at least one skill.".into());
    }
    let mut a: Vec<String> = vec!["add".into(), source.into(), "-y".into(), "--json".into()];
    a.push("-s".into());
    a.extend(check(skills, valid_skill, "skill name")?.iter().cloned());
    if !agents.is_empty() {
        a.push("-a".into());
        a.extend(check(agents, valid_agent, "agent")?.iter().cloned());
    }
    scope_args(&mut a, scope);
    // Symlinks need Developer Mode or admin rights on Windows.
    if windows {
        a.push("--copy".into());
    }
    Ok(a)
}

pub fn remove_args(skills: &[String], agents: &[String], scope: Scope) -> Result<Vec<String>, String> {
    if skills.is_empty() {
        return Err("Pick at least one skill.".into());
    }
    let mut a: Vec<String> = vec!["remove".into(), "-y".into(), "-s".into()];
    a.extend(check(skills, valid_skill, "skill name")?.iter().cloned());
    if !agents.is_empty() {
        a.push("-a".into());
        a.extend(check(agents, valid_agent, "agent")?.iter().cloned());
    }
    scope_args(&mut a, scope);
    Ok(a)
}

pub fn update_args(skills: &[String], scope: Scope) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = vec!["update".into(), "-y".into()];
    a.push(if scope == Scope::Global { "-g" } else { "-p" }.into());
    a.extend(check(skills, valid_skill, "skill name")?.iter().cloned());
    Ok(a)
}

pub fn init_args(name: &str) -> Result<Vec<String>, String> {
    if !valid_skill(name) {
        return Err(format!("Not a valid skill name: {name} (a-z, 0-9, . _ -)"));
    }
    Ok(vec!["init".into(), name.into()])
}

// ── Output ────────────────────────────────────────────────────────────────────

/// One entry of `list --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub name: String,
    pub path: String,
    pub scope: Scope,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
}

/// One entry of `add --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    #[serde(default)]
    pub name: Option<String>,
    pub status: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// The JSON array in `stdout`, skipping anything printed before it (npx's
/// own notices).
fn json_array(stdout: &str) -> Option<&str> {
    let start = stdout.find('[')?;
    let end = stdout.rfind(']')?;
    (end >= start).then(|| &stdout[start..=end])
}

pub fn parse_list(stdout: &str) -> Result<Vec<Installed>, String> {
    let Some(json) = json_array(stdout) else {
        return Ok(Vec::new());
    };
    serde_json::from_str(json).map_err(|e| format!("Unexpected output from skills list: {e}"))
}

pub fn parse_add(stdout: &str) -> Result<Vec<AddResult>, String> {
    let json = json_array(stdout).ok_or_else(|| "No result from skills add.".to_string())?;
    serde_json::from_str(json).map_err(|e| format!("Unexpected output from skills add: {e}"))
}

/// ANSI colour codes out of the CLI's text output.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

// ── Running it ────────────────────────────────────────────────────────────────

pub struct Output {
    pub stdout: String,
}

/// The fixed environment of every run: telemetry off, no colours, no prompts.
pub fn env(github_token: Option<String>) -> Vec<(&'static str, String)> {
    let mut e = vec![
        ("DISABLE_TELEMETRY", "1".to_string()),
        ("DO_NOT_TRACK", "1".to_string()),
        ("NO_COLOR", "1".to_string()),
        ("FORCE_COLOR", "0".to_string()),
        ("CI", "1".to_string()),
        ("npm_config_update_notifier", "false".to_string()),
        ("npm_config_fund", "false".to_string()),
    ];
    if let Some(t) = github_token.filter(|t| !t.trim().is_empty()) {
        e.push(("GITHUB_TOKEN", t));
    }
    e
}

/// Runs `skills@1.7.2 <args>` in `cwd` and waits (up to 4 min). Blocking: call
/// it from a blocking task.
pub fn run(cwd: &Path, args: &[String]) -> Result<Output, String> {
    let node = node_path().ok_or("Node.js is not installed.")?;
    let version = node_version().and_then(|v| parse_node_version(&v)).ok_or("Node.js does not answer.")?;
    if !node_new_enough(version) {
        return Err(format!("Node.js {}.{} or newer is needed (found {}.{}).", NODE_MIN.0, NODE_MIN.1, version.0, version.1));
    }
    let mut cmd = match npx_cli(&node) {
        Some(npx) => {
            let mut c = Command::new(&node);
            c.arg(npx);
            c
        }
        None if !cfg!(windows) => Command::new(which("npx").ok_or("npx was not found next to Node.js.")?),
        None => return Err("npx was not found next to Node.js.".into()),
    };
    cmd.arg("-y").arg(format!("skills@{CLI_VERSION}")).args(args);
    cmd.current_dir(cwd);
    for (k, v) in env(crate::secrets::get("github-token")) {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_window(&mut cmd);
    crate::log::line(format!("skills: {}", args.first().map(String::as_str).unwrap_or("")));
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the skills CLI: {e}"))?;
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out_pipe.read_to_string(&mut s);
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err_pipe.read_to_string(&mut s);
        s
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The skills CLI took too long and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = strip_ansi(&err_reader.join().unwrap_or_default());
    if !status.success() && json_array(&stdout).is_none() {
        let msg = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        let msg = if msg.is_empty() { strip_ansi(stdout.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("")) } else { msg };
        return Err(format!("The skills CLI failed: {msg}"));
    }
    Ok(Output { stdout })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_versions() {
        assert_eq!(parse_node_version("v22.20.0\n"), Some((22, 20)));
        assert_eq!(parse_node_version("v24.1.3"), Some((24, 1)));
        assert_eq!(parse_node_version("nope"), None);
        assert!(node_new_enough((22, 20)));
        assert!(node_new_enough((23, 0)));
        assert!(!node_new_enough((22, 19)));
        assert!(!node_new_enough((20, 30)));
    }

    #[test]
    fn arguments_are_built_not_parsed() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            add_args("vercel-labs/skills", &s(&["find-skills"]), &s(&["claude-code", "codex"]), Scope::Project, true).unwrap(),
            s(&["add", "vercel-labs/skills", "-y", "--json", "-s", "find-skills", "-a", "claude-code", "codex", "--copy"])
        );
        assert_eq!(
            add_args("https://github.com/o/r/tree/main/skills/x", &s(&["x"]), &[], Scope::Global, false).unwrap(),
            s(&["add", "https://github.com/o/r/tree/main/skills/x", "-y", "--json", "-s", "x", "-g"])
        );
        assert_eq!(remove_args(&s(&["a", "b"]), &[], Scope::Project).unwrap(), s(&["remove", "-y", "-s", "a", "b"]));
        assert_eq!(update_args(&[], Scope::Project).unwrap(), s(&["update", "-y", "-p"]));
        assert_eq!(list_args(Scope::Global), s(&["list", "--json", "-g"]));
        // Nothing that could be read as an option or by a shell.
        assert!(add_args("--all", &s(&["x"]), &[], Scope::Project, false).is_err());
        assert!(add_args("o/r; rm -rf /", &s(&["x"]), &[], Scope::Project, false).is_err());
        assert!(add_args("o/r", &s(&["-g"]), &[], Scope::Project, false).is_err());
        assert!(add_args("o/r", &s(&["../../etc"]), &[], Scope::Project, false).is_err());
        assert!(add_args("o/r", &s(&["x"]), &s(&["Claude Code"]), Scope::Project, false).is_err());
        assert!(add_args("o/r", &[], &[], Scope::Project, false).is_err());
        assert!(init_args("my skill").is_err());
        assert_eq!(init_args("my-skill").unwrap(), s(&["init", "my-skill"]));
    }

    #[test]
    fn telemetry_is_always_off_and_only_the_github_token_is_passed() {
        let e = env(Some("ghp_x".into()));
        assert!(e.contains(&("DISABLE_TELEMETRY", "1".into())));
        assert!(e.contains(&("GITHUB_TOKEN", "ghp_x".into())));
        assert!(e.iter().all(|(k, _)| !k.contains("ANTHROPIC") && !k.contains("OPENAI")));
        assert!(!env(None).iter().any(|(k, _)| *k == "GITHUB_TOKEN"));
        assert!(!env(Some("  ".into())).iter().any(|(k, _)| *k == "GITHUB_TOKEN"));
    }

    #[test]
    fn list_output_is_parsed() {
        let out = r#"npm warn exec something
[
  {
    "name": "frontend-design",
    "path": "/p/.agents/skills/frontend-design",
    "scope": "project",
    "agents": ["Claude Code", "Codex"],
    "source": "anthropics/skills",
    "sourceUrl": "https://github.com/anthropics/skills.git",
    "sourceType": "github"
  },
  { "name": "mine", "path": "/p/.claude/skills/mine", "scope": "project", "agents": ["Claude Code"], "source": null, "sourceUrl": null, "sourceType": null }
]"#;
        let list = parse_list(out).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].source.as_deref(), Some("anthropics/skills"));
        assert_eq!(list[1].source, None);
        assert!(parse_list("No skills installed").unwrap().is_empty());
        assert!(parse_list("[{\"oops\": 1}]").is_err());
    }

    #[test]
    fn add_output_is_parsed() {
        let out = r#"[{"name":"x","status":"installed","path":"/p/.agents/skills/x","scope":"project"},{"status":"failed","error":"Repository not found"}]"#;
        let r = parse_add(out).unwrap();
        assert_eq!(r[0].status, "installed");
        assert_eq!(r[1].error.as_deref(), Some("Repository not found"));
        assert!(parse_add("").is_err());
    }

    #[test]
    fn colours_are_stripped() {
        assert_eq!(strip_ansi("\u{1b}[31merror\u{1b}[0m: no"), "error: no");
    }
}
