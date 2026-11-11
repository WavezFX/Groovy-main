// The subscriptions (round 4D item 1): Claude Code and Codex answer through
// their own command-line tools, with the user's own account and plan, so no
// API key is needed. The tool is run for one answer: no shell, the prompt on
// stdin, in an empty folder, with the tool's own actions off.

use std::time::Duration;

use serde_json::Value;

use super::{last_user_text, text_reply, Auth, Brain, BrainError, Caps, Ctx, Request, Tier, Turn};

/// A subscription through its command-line tool.
pub struct Cli {
    pub id: &'static str,
    pub name: &'static str,
}

/// With tools to think about, a subscription's tool may take this long.
const WITH_TOOLS: Duration = Duration::from_secs(90);

impl Brain for Cli {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn caps(&self) -> Caps {
        Caps { tools: false, vision: false, streaming: false, web_search: false, local: false, tier: Tier::Large }
    }
    fn auth(&self) -> Auth {
        // Installed is taken for signed in; the tool says so itself if it isn't.
        Auth::Account { signed_in: path(self.id).is_some() }
    }
    fn turn<'a>(&'a self, _ctx: Ctx<'a>, req: Request<'a>) -> Turn<'a> {
        Box::pin(async move {
            let (prompt, timeout) = if req.tools.is_empty() {
                (format!("{}

{}", req.system, last_user_text(req.messages)), req.timeout)
            } else {
                (as_prompt(req.system, req.messages), WITH_TOOLS)
            };
            complete(self.id, prompt, timeout).await.map(text_reply).map_err(BrainError::of)
        })
    }
}

/// The command-line tool of a subscription, when it is installed.
pub fn path(tool: &str) -> Option<std::path::PathBuf> {
    match tool {
        "claude-code" => crate::skills::cli::which("claude"),
        "codex" => crate::skills::cli::which("codex"),
        _ => None,
    }
}

/// A subscription's CLI answers one prompt, text only. No shell: the prompt
/// goes in on stdin; Claude Code gets one turn (no tools), Codex a read-only
/// sandbox in an empty folder.
pub(crate) async fn complete(tool: &'static str, prompt: String, timeout: Duration) -> Result<String, String> {
    let exe = path(tool).ok_or_else(|| format!("{tool} is not installed."))?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;
        let dir = std::env::temp_dir().join("groovy-brain");
        let _ = std::fs::create_dir_all(&dir);
        let mut cmd = std::process::Command::new(&exe);
        match tool {
            "claude-code" => cmd.args(["-p", "--output-format", "text", "--max-turns", "1"]),
            _ => cmd.args(["exec", "--skip-git-repo-check", "-s", "read-only", "-"]),
        };
        cmd.current_dir(&dir).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let mut child = crate::platform::no_console(&mut cmd).spawn().map_err(|e| format!("{tool} didn't start: {e}"))?;
        crate::platform::contain(&child);
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
        }
        let started = std::time::Instant::now();
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            if started.elapsed() > timeout {
                let _ = child.kill();
                return Err(format!("{tool} took too long."));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || text.is_empty() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("{tool}: {}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no answer").trim()));
        }
        Ok(text)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A conversation as one plain-text prompt, for the subscription CLIs.
pub fn as_prompt(system: &str, messages: &[Value]) -> String {
    let mut out = format!("{system}\n\nConversation so far:\n");
    for m in messages {
        let role = if m["role"] == "assistant" { "You" } else { "User" };
        let text = match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|b| match b["type"].as_str() {
                    Some("text") => b["text"].as_str().map(String::from),
                    Some("tool_result") => Some(format!("(result: {})", b["content"].as_str().unwrap_or(""))),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        if !text.trim().is_empty() {
            out.push_str(&format!("{role}: {text}\n"));
        }
    }
    out.push_str("\nAnswer the user's last message. You have no tools here: say what you would do if a tool is needed.");
    out
}
