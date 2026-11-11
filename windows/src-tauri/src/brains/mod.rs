// One interface for every brain (round 4D item 1).
//
// Claude, the OpenAI-compatible services (Gemini, OpenAI, OpenRouter), the
// subscriptions reached through their command-line tools (Claude Code, Codex)
// and the model servers on this PC each implement `Brain`: what it can do
// (`Caps`), how the user is known to it (`Auth`), and one turn of a
// conversation in and out in the Messages shape the callers already use
// (`{ content: [blocks], stop_reason }`). Each adapter converts inside itself.
//
// Who answers, and in which order, is still decided in brain_ai.rs (cloud and
// subscription first, this PC last, switching when a limit is hit); it asks
// the registry here for the brain and no longer cares which kind it is.
// Adding a brain is one adapter and one line in `registry`.

pub mod cli;

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::assistant::AssistantPrefs;
use crate::settings::Settings;

/// A light model for small jobs, or a big one: what a router plugin sorts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    Small,
    Large,
}

/// What a brain can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Caps {
    /// It can ask for Groovy's tools (voice commands, the screen buddy).
    pub tools: bool,
    /// It can be shown a picture.
    pub vision: bool,
    /// Its answer can arrive as it is written.
    pub streaming: bool,
    /// It can search the web by itself.
    pub web_search: bool,
    /// It runs on this PC: nothing goes online.
    pub local: bool,
    /// The model the background jobs use with it.
    pub tier: Tier,
}

/// How the user is known to a brain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Auth {
    /// The user's own account and plan, signed in through the brain's tool.
    Account { signed_in: bool },
    /// The user's API key, in the system's keychain.
    ApiKey { present: bool },
    /// A server on this PC: nothing to sign in to.
    None,
}

impl Auth {
    /// The brain can be asked right now.
    pub fn ready(&self) -> bool {
        match self {
            Auth::Account { signed_in } => *signed_in,
            Auth::ApiKey { present } => *present,
            Auth::None => true,
        }
    }
}

/// Why a brain didn't answer, as far as switching to the next one goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// A free tier or a plan ran out, or too many requests.
    Limit,
    /// The service couldn't be reached at all.
    Network,
    /// No key, a refused key, not signed in.
    Auth,
    /// Anything else: a refusal, a bad request, an empty answer.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrainError {
    pub fault: Fault,
    /// The brain's own words, shown to the user as they are.
    pub message: String,
}

impl BrainError {
    /// Sorts an error message the adapters got back (the one place that reads error text).
    pub fn of(message: impl Into<String>) -> BrainError {
        let message = message.into();
        let e = message.to_ascii_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| e.contains(w));
        let fault = if has(&["network error", "error sending request", "dns error", "failed to lookup", "connection refused", "timed out", "couldn't reach", "cannot reach"]) {
            Fault::Network
        } else if has(&["429", "rate limit", "rate_limit", "quota", "usage limit", "resource_exhausted", "too many requests", "limit reached"]) {
            Fault::Limit
        } else if has(&["401", "403", "api key", "api_key", "unauthorized", "not signed in", "not logged in", "is not installed", "no key"]) {
            Fault::Auth
        } else {
            Fault::Other
        };
        BrainError { fault, message }
    }
}

impl std::fmt::Display for BrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// One turn to answer. `messages` and `tools` are in the Messages shape
/// (content blocks, tool_use / tool_result); no tools means a plain answer.
#[derive(Clone, Copy)]
pub struct Request<'a> {
    pub system: &'a str,
    pub messages: &'a [Value],
    pub tools: &'a [Value],
    pub max_tokens: u32,
    pub timeout: Duration,
}

/// What an adapter needs from the app to answer.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub app: &'a AppHandle,
    pub settings: &'a Settings,
    pub prefs: &'a AssistantPrefs,
}

pub type Turn<'a> = Pin<Box<dyn Future<Output = Result<Value, BrainError>> + Send + 'a>>;

pub trait Brain: Sync {
    /// The id stored in settings and used by the Brains page.
    fn id(&self) -> &'static str;
    /// Its name as the user knows it.
    fn name(&self) -> &'static str;
    fn caps(&self) -> Caps;
    fn auth(&self) -> Auth;
    /// One turn: `{ content: [blocks], stop_reason }`.
    fn turn<'a>(&'a self, ctx: Ctx<'a>, req: Request<'a>) -> Turn<'a>;
}

/// A plain answer as a turn's reply.
pub fn text_reply(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "stop_reason": "end_turn" })
}

/// The words of a reply, its text blocks joined.
pub fn text_of(reply: &Value) -> String {
    reply["content"].as_array().map(|blocks| blocks.iter().filter_map(|b| (b["type"] == "text").then(|| b["text"].as_str()).flatten()).collect::<Vec<_>>().join("")).unwrap_or_default()
}

/// The last thing the user said, as plain text (for the adapters that take one prompt).
pub fn last_user_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .map(|m| match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" "),
            _ => String::new(),
        })
        .unwrap_or_default()
}

// ── Claude, with the user's key ───────────────────────────────────────────────

struct Anthropic;

impl Brain for Anthropic {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    fn name(&self) -> &'static str {
        "Claude"
    }
    fn caps(&self) -> Caps {
        Caps { tools: true, vision: true, streaming: false, web_search: true, local: false, tier: Tier::Small }
    }
    fn auth(&self) -> Auth {
        Auth::ApiKey { present: crate::secrets::present(crate::claude::KEY) }
    }
    fn turn<'a>(&'a self, _ctx: Ctx<'a>, req: Request<'a>) -> Turn<'a> {
        Box::pin(async move {
            let model = crate::brain_ai::CLAUDE_FAST;
            if req.tools.is_empty() {
                crate::claude::complete(model, req.system, &last_user_text(req.messages), req.max_tokens, req.timeout).await.map(text_reply).map_err(BrainError::of)
            } else {
                crate::claude::voice_turn(model, req.system, req.messages, req.tools).await.map_err(BrainError::of)
            }
        })
    }
}

// ── OpenAI-compatible services, with the user's key ───────────────────────────

struct Compat {
    id: &'static str,
    name: &'static str,
}

impl Brain for Compat {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn caps(&self) -> Caps {
        Caps { tools: true, vision: true, streaming: false, web_search: false, local: false, tier: Tier::Small }
    }
    fn auth(&self) -> Auth {
        Auth::ApiKey { present: crate::openai_compat::provider(self.id).is_some_and(|p| crate::secrets::present(p.key)) }
    }
    fn turn<'a>(&'a self, ctx: Ctx<'a>, req: Request<'a>) -> Turn<'a> {
        Box::pin(async move { crate::brain_ai::openai_turn(ctx, Some(self.id), None, req).await.map_err(BrainError::of) })
    }
}

// ── The model servers on this PC ──────────────────────────────────────────────

struct Local {
    id: &'static str,
    name: &'static str,
}

impl Brain for Local {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn caps(&self) -> Caps {
        Caps { tools: true, vision: false, streaming: false, web_search: false, local: true, tier: Tier::Small }
    }
    fn auth(&self) -> Auth {
        Auth::None
    }
    fn turn<'a>(&'a self, ctx: Ctx<'a>, req: Request<'a>) -> Turn<'a> {
        Box::pin(async move { crate::brain_ai::openai_turn(ctx, None, Some(self.id), req).await.map_err(BrainError::of) })
    }
}

static BRAINS: [&dyn Brain; 9] = [
    &Anthropic,
    &Compat { id: "google", name: "Gemini" },
    &Compat { id: "openai", name: "OpenAI" },
    &Compat { id: "openrouter", name: "OpenRouter" },
    &cli::Cli { id: "claude-code", name: "Claude Code" },
    &cli::Cli { id: "codex", name: "Codex" },
    &Local { id: "ollama", name: "Ollama" },
    &Local { id: "lmstudio", name: "LM Studio" },
    &Local { id: "llamacpp", name: "Groovy local model" },
];

/// A server the user typed the address of.
static CUSTOM: Local = Local { id: "custom", name: "Local" };

/// Every brain Groovy can ask.
pub fn registry() -> impl Iterator<Item = &'static dyn Brain> {
    BRAINS.iter().copied().chain(std::iter::once(&CUSTOM as &dyn Brain))
}

pub fn get(id: &str) -> Option<&'static dyn Brain> {
    registry().find(|b| b.id() == id)
}

/// One row of the brains table (Settings, STATUS.md): what each brain is and can do.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    id: &'static str,
    name: &'static str,
    caps: Caps,
    auth: Auth,
}

/// Every brain with what it can do and whether it is set up.
#[tauri::command]
pub async fn brains() -> Vec<Row> {
    tauri::async_runtime::spawn_blocking(|| registry().map(|b| Row { id: b.id(), name: b.name(), caps: b.caps(), auth: b.auth() }).collect()).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stored_id_has_a_brain_and_no_id_is_used_twice() {
        let ids: Vec<&str> = registry().map(|b| b.id()).collect();
        for id in ["anthropic", "google", "openai", "openrouter", "claude-code", "codex", "ollama", "lmstudio", "llamacpp", "custom"] {
            assert!(get(id).is_some(), "{id}");
        }
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
        assert!(get("nobody").is_none());
        assert!(registry().all(|b| !b.name().is_empty()));
    }

    #[test]
    fn what_each_kind_of_brain_can_do() {
        let caps = |id: &str| get(id).unwrap().caps();
        assert!(caps("anthropic").web_search && caps("anthropic").vision && caps("anthropic").tools);
        assert!(caps("google").tools && caps("google").vision && !caps("google").local);
        assert!(caps("ollama").local && !caps("ollama").vision);
        assert!(caps("custom").local);
        assert!(!caps("codex").local && !caps("claude-code").web_search);
        assert_eq!(get("ollama").unwrap().auth(), Auth::None);
        assert!(Auth::None.ready() && !Auth::ApiKey { present: false }.ready() && Auth::Account { signed_in: true }.ready());
    }

    #[test]
    fn errors_are_sorted_in_one_place() {
        let fault = |m: &str| BrainError::of(m).fault;
        assert_eq!(fault("HTTP 429: quota exceeded"), Fault::Limit);
        assert_eq!(fault("You've hit your usage limit"), Fault::Limit);
        assert_eq!(fault("Cannot reach api.example.com: error sending request"), Fault::Network);
        assert_eq!(fault("codex took too long. timed out"), Fault::Network);
        assert_eq!(fault("HTTP 401: invalid x-api-key"), Fault::Auth);
        assert_eq!(fault("claude-code is not installed."), Fault::Auth);
        assert_eq!(fault("HTTP 400: bad request"), Fault::Other);
        assert_eq!(fault("empty answer"), Fault::Other);
        assert_eq!(BrainError::of("HTTP 400: nope").to_string(), "HTTP 400: nope", "the brain's own words are kept");
    }

    #[test]
    fn a_reply_is_read_and_written_in_the_messages_shape() {
        let reply = text_reply("It is three o'clock.");
        assert_eq!(reply["stop_reason"], "end_turn");
        assert_eq!(text_of(&reply), "It is three o'clock.");
        let mixed = json!({ "content": [{ "type": "text", "text": "Done" }, { "type": "tool_use", "name": "music", "input": {} }, { "type": "text", "text": "." }] });
        assert_eq!(text_of(&mixed), "Done.");
        let messages = [json!({ "role": "user", "content": "first" }), json!({ "role": "assistant", "content": "ok" }), json!({ "role": "user", "content": [{ "type": "text", "text": "second" }] })];
        assert_eq!(last_user_text(&messages), "second");
        assert_eq!(last_user_text(&[]), "");
    }
}
