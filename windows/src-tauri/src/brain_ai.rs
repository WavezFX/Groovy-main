// Which model answers the background features ported from Coucou 0.5.0 —
// Flow's clean-up, the Notetaker's summary and « Hey Groovy »'s tool calls —
// and how to talk to it.
//
// Cloud and subscription first, local last (FIXES_ROUND1 item 2: on an 8 GB
// PC the local model ate the machine). By default ("brain": "auto") the
// chat's own cloud provider answers, then any other provider with a key
// (Claude, Gemini, OpenAI, OpenRouter), then a Claude Code or Codex
// subscription through its CLI when that was chosen at setup, then the local
// model — only if local AI was chosen (AssistantPrefs::local_allowed).
// Local-only mode never sends anything online.
//
// The callers were written for Anthropic's Messages format (content blocks,
// tool_use / tool_result). This module converts to and from the OpenAI format
// for local models, so they keep one shape.

use std::time::Duration;

use reqwest::Url;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::assistant::AssistantPrefs;
use crate::settings::Settings;

/// Claude's fast model for the background features, when Claude is chosen.
pub const CLAUDE_FAST: &str = "claude-haiku-4-5";

static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Called once at start.
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

fn app() -> Result<&'static AppHandle, String> {
    APP.get().ok_or_else(|| "not started".to_string())
}

fn settings(app: &AppHandle) -> Settings {
    app.try_state::<crate::Shared>().map(|s| s.settings.lock().unwrap().clone()).unwrap_or_default()
}

/// Someone who can answer the background features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    /// The local model (llama.cpp, Ollama, LM Studio, a custom server).
    Local,
    /// Claude with the user's key.
    Claude,
    /// An OpenAI-compatible cloud provider with the user's key: google, openai, openrouter.
    Cloud(&'static str),
    /// A subscription through its command-line tool: claude-code, codex.
    Cli(&'static str),
}

const CLOUDS: [&str; 3] = ["google", "openai", "openrouter"];

fn cloud_id(id: &str) -> Option<&'static str> {
    CLOUDS.iter().copied().find(|c| *c == id)
}

fn cli_id(id: &str) -> Option<&'static str> {
    ["claude-code", "codex"].into_iter().find(|c| *c == id)
}

/// Who answers first, and who stands in. Cloud and subscription first, local
/// last and only when local AI was chosen (FIXES_ROUND1 item 2). Pure:
/// `has_key(provider)` and `has_cli(tool)` say what this PC has.
pub fn order(prefs: &AssistantPrefs, chat_provider: &str, has_key: impl Fn(&str) -> bool, has_cli: impl Fn(&str) -> bool) -> Vec<Who> {
    let local_ok = prefs.local_allowed();
    let with_local = |mut v: Vec<Who>| {
        if local_ok && prefs.ai_mode == "hybrid" && !v.contains(&Who::Local) {
            v.push(Who::Local);
        }
        v
    };
    if prefs.ai_mode == "local" {
        return if local_ok { vec![Who::Local] } else { vec![] };
    }
    match prefs.brain.as_str() {
        "local" => return if local_ok { vec![Who::Local] } else { vec![] },
        "claude" => return with_local(vec![Who::Claude]),
        b if cloud_id(b).is_some() => return with_local(vec![Who::Cloud(cloud_id(b).unwrap())]),
        b if cli_id(b).is_some() => return with_local(vec![Who::Cli(cli_id(b).unwrap())]),
        _ => {}
    }
    // "auto": the chat's own cloud provider, then every key, then a subscription, then local.
    let mut v: Vec<Who> = Vec::new();
    let mut push = |w: Who| {
        if !v.contains(&w) {
            v.push(w);
        }
    };
    if chat_provider == "anthropic" && has_key("anthropic") {
        push(Who::Claude);
    } else if let Some(c) = cloud_id(chat_provider).filter(|c| has_key(c)) {
        push(Who::Cloud(c));
    }
    if has_key("anthropic") {
        push(Who::Claude);
    }
    for c in CLOUDS {
        if has_key(c) {
            push(Who::Cloud(c));
        }
    }
    if prefs.ai_setup == "subscription" {
        for c in ["claude-code", "codex"] {
            if has_cli(c) {
                push(Who::Cli(cli_id(c).unwrap()));
            }
        }
    }
    if local_ok && prefs.ai_mode != "online" {
        push(Who::Local);
    }
    v
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clis {
    claude_code: bool,
    codex: bool,
}

/// Which subscription CLIs this PC has, for the AI setup.
#[tauri::command]
pub async fn ai_clis() -> Clis {
    tauri::async_runtime::spawn_blocking(|| Clis { claude_code: crate::brains::cli::path("claude-code").is_some(), codex: crate::brains::cli::path("codex").is_some() })
        .await
        .unwrap_or(Clis { claude_code: false, codex: false })
}

fn current_order(s: &Settings) -> Vec<Who> {
    current_order_for(s, "")
}

/// The order for one job ("writing", "commands"): the job's own brain first
/// when one was chosen, then the Auto order.
fn current_order_for(s: &Settings, job: &str) -> Vec<Who> {
    let prefs = s.assistant.clone().sanitised();
    let ready = |id: &str| crate::brains::get(id).is_some_and(|b| b.auth().ready());
    let mut v = order(&prefs, &s.chat_provider, ready, ready);
    if !local_configured(s, &prefs) {
        v.retain(|w| *w != Who::Local);
    }
    arrange(v, &prefs.brain_order, prefs.job_brains.get(job).map(String::as_str))
}

impl Who {
    /// The id the Brains page uses for it.
    pub fn id(&self) -> &'static str {
        match self {
            Who::Local => "local",
            Who::Claude => "anthropic",
            Who::Cloud(c) | Who::Cli(c) => c,
        }
    }
}

/// The user's Auto order (Brains), then the job's own brain first. Brains not
/// in the order keep their place after those that are; this PC stays last.
pub fn arrange(mut v: Vec<Who>, user_order: &[String], job_first: Option<&str>) -> Vec<Who> {
    if !user_order.is_empty() {
        let rank = |w: &Who| user_order.iter().position(|id| id == w.id()).unwrap_or(user_order.len());
        v.sort_by_key(|w| (*w == Who::Local, rank(w)));
    }
    if let Some(id) = job_first.filter(|id| *id != "auto") {
        if let Some(i) = v.iter().position(|w| w.id() == id) {
            let w = v.remove(i);
            v.insert(0, w);
        }
    }
    v
}

/// After a failed brain: whether the next one may answer.
fn may_switch(prefs: &AssistantPrefs, err: &str) -> bool {
    prefs.switch_on_limit || !is_limit(err)
}

/// Who answers first right now, by name: "Gemini", "Claude", "Local"…
pub fn active_name() -> Option<&'static str> {
    let app = app().ok()?;
    current_order(&settings(app)).first().map(|w| match w {
        Who::Local => "Local",
        Who::Claude => "Claude",
        Who::Cloud("google") => "Gemini",
        Who::Cloud("openai") => "OpenAI",
        Who::Cloud(_) => "OpenRouter",
        Who::Cli("codex") => "Codex",
        Who::Cli(_) => "Claude Code",
    })
}

/// For quick settings (round 4 item 2): who answers first and the state of
/// the others worth knowing about, as (name, "ready" | "no key" | "on" | "off").
pub fn states() -> Vec<(&'static str, &'static str)> {
    let Ok(app) = app() else { return vec![] };
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    let mut v: Vec<(&'static str, &'static str)> = Vec::new();
    if let Some(name) = active_name() {
        v.push((name, "ready"));
    }
    if !v.iter().any(|(n, _)| *n == "Claude") && !crate::brains::get("anthropic").is_some_and(|b| b.auth().ready()) {
        v.push(("Claude", "no key"));
    }
    if !v.iter().any(|(n, _)| *n == "Local") {
        v.push(("Local", if prefs.local_allowed() && local_configured(&s, &prefs) { "on" } else { "off" }));
    }
    v
}

/// An error that says a free tier or plan ran out, or too many requests.
pub fn is_limit(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    ["429", "rate limit", "rate_limit", "quota", "usage limit", "resource_exhausted", "too many requests", "limit reached"].iter().any(|w| e.contains(w))
}

/// Whether there is someone to answer at all.
pub fn available() -> bool {
    let Ok(app) = app() else { return false };
    !current_order(&settings(app)).is_empty()
}

/// Where the first cloud brain in the order answers, for the wake word's warm-up
/// and the offline check (round 3D item 10); None when only local or CLI brains are set up.
pub fn first_cloud_url(app: &AppHandle) -> Option<String> {
    current_order_for(&settings(app), "chat").into_iter().find_map(|w| match w {
        Who::Claude => Some("https://api.anthropic.com/".to_string()),
        Who::Cloud(id) => crate::openai_compat::provider(id).map(|p| format!("{}/", p.base_url)),
        _ => None,
    })
}

fn local_configured(s: &Settings, prefs: &AssistantPrefs) -> bool {
    let built_in = || crate::models::server_exe().is_some() && !crate::models::discover(&crate::models::models_dir(), 2048).is_empty();
    match prefs.local_provider.as_str() {
        // Ollama or LM Studio seen answering, Ollama on the PC, or the built-in model ready.
        "auto" => crate::local_chat::auto_seen().is_some_and(|id| id != crate::chat::LLAMACPP) || crate::ollama::installed() || built_in(),
        "llamacpp" => built_in(),
        "ollama" | "lmstudio" => true,
        "custom" => !s.custom_url.trim().is_empty(),
        _ => false,
    }
}

/// Whether the brain that answers first can search the web by itself (Claude
/// only, today); the others say they have no internet access.
pub fn has_web_search() -> bool {
    let Ok(app) = app() else { return false };
    current_order(&settings(app)).first().is_some_and(|w| !matches!(w, Who::Local) && crate::brains::get(w.id()).is_some_and(|b| b.caps().web_search))
}

struct Local {
    base: Url,
    model: String,
    key: Option<String>,
}

/// The model server `id` on this PC ("llamacpp", "ollama", "lmstudio", "custom"), ready to be asked.
async fn local(app: &AppHandle, s: &Settings, prefs: &AssistantPrefs, id: &str) -> Result<Local, String> {
    if id == "llamacpp" {
        let mut p = prefs.clone();
        let picked = crate::chat::model_for(s, crate::chat::LLAMACPP);
        if !picked.is_empty() {
            p.local_model = picked;
        }
        let url = crate::models::ensure_running(&p, Some(app)).await?;
        let model = crate::models::running_model().unwrap_or_else(|| "local".into());
        return Ok(Local { base: crate::net::normalise_server_url(&url)?, model, key: None });
    }
    let server = crate::local_chat::server(s, id).ok_or("No local model configured.")?;
    let base = crate::local_chat::base_url(&server)?;
    let picked = crate::chat::model_for(s, id);
    let model = if picked.is_empty() { crate::local_chat::first_model(&server).await? } else { picked };
    Ok(Local { base, model, key: server.key.clone() })
}

async fn post_to(endpoint: Url, key: Option<&str>, body: &Value, timeout: Duration) -> Result<Value, String> {
    let mut req = crate::net::client(&endpoint, timeout)?.post(endpoint.clone()).json(body);
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.map_err(|e| format!("Cannot reach {}: {e}", crate::net::host_for_log(&endpoint)))?;
    let status = resp.status();
    let bytes = crate::net::read_capped(resp, 8 * 1024 * 1024).await?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), crate::net::error_detail(&bytes)));
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// One turn in the OpenAI format, for the adapters in brains/: a cloud service
/// (`cloud`: google, openai, openrouter) or a model server on this PC (`local`).
/// In and out in the Messages shape; no tools means a plain answer.
pub(crate) async fn openai_turn(ctx: crate::brains::Ctx<'_>, cloud: Option<&str>, local_id: Option<&str>, req: crate::brains::Request<'_>) -> Result<Value, String> {
    let plain = req.tools.is_empty();
    let mut body = if plain {
        json!({
            "max_tokens": req.max_tokens,
            "stream": false,
            "messages": [{ "role": "system", "content": req.system }, { "role": "user", "content": crate::brains::last_user_text(req.messages) }],
        })
    } else {
        json!({
            "max_tokens": req.max_tokens,
            "stream": false,
            "messages": to_openai_messages(req.system, req.messages),
            "tools": to_openai_tools(req.tools),
        })
    };
    let answer = if let Some(id) = cloud {
        let p = crate::openai_compat::provider(id).ok_or("unknown provider")?;
        let key = crate::secrets::get(p.key).ok_or_else(|| format!("No {} key.", p.name))?;
        let picked = crate::chat::model_for(ctx.settings, id);
        body["model"] = json!(if picked.is_empty() { p.default_model.to_string() } else { picked });
        if let Some(n) = body.as_object_mut().and_then(|o| o.remove("max_tokens")) {
            body[p.max_tokens_field] = n;
        }
        let endpoint = Url::parse(&format!("{}/chat/completions", p.base_url)).map_err(|e| e.to_string())?;
        post_to(endpoint, Some(&key), &body, req.timeout).await?
    } else {
        let id = local_id.ok_or("not an OpenAI-style provider")?;
        crate::resources::loads::note(crate::resources::loads::Part::LocalServer, "a local brain answers this request");
        let l = local(ctx.app, ctx.settings, ctx.prefs, id).await?;
        body["model"] = json!(l.model);
        // Ollama lets the model go when Groovy is quiet (round 3D item 5).
        body["keep_alive"] = json!(crate::local_chat::KEEP_ALIVE);
        let endpoint = Url::parse(&crate::net::join(&l.base, "/v1/chat/completions")).map_err(|e| e.to_string())?;
        let r = post_to(endpoint, l.key.as_deref(), &body, req.timeout).await;
        crate::models::touch();
        r?
    };
    if plain {
        let text = strip_thinking(answer["choices"][0]["message"]["content"].as_str().unwrap_or(""));
        if text.trim().is_empty() {
            return Err("empty answer".into());
        }
        Ok(crate::brains::text_reply(text))
    } else {
        Ok(from_openai(&answer))
    }
}

/// The brain behind a place in the order. This PC's is whichever server
/// answers (Ollama, LM Studio, the built-in model, a custom address).
async fn brain_of(who: Who, s: &Settings, prefs: &AssistantPrefs) -> Option<&'static dyn crate::brains::Brain> {
    match who {
        Who::Local => crate::brains::get(&crate::local_chat::resolve(s, &prefs.local_provider).await),
        other => crate::brains::get(other.id()),
    }
}

/// Asks each brain in `order` until one answers; the next one is tried only
/// when the failure is one switching may fix (`may_switch`).
async fn ask(app: &AppHandle, s: &Settings, prefs: &AssistantPrefs, order: Vec<Who>, req: crate::brains::Request<'_>) -> Result<Value, String> {
    let ctx = crate::brains::Ctx { app, settings: s, prefs };
    let mut last = crate::i18n::t("No AI is set up yet: choose one in Groovy → Assistant → Groovy's AI.");
    let mut tally = CloudTally::default();
    for who in order {
        let r = match brain_of(who, s, prefs).await {
            Some(brain) => brain.turn(ctx, req).await,
            None => Err(crate::brains::BrainError::of("No local model configured.")),
        };
        match r {
            Ok(v) => {
                tally.answered(app, who);
                return Ok(v);
            }
            Err(e) => {
                crate::log::line(format!("brain ({who:?}): {e}"));
                tally.failed(who, e.fault);
                let stop = !may_switch(prefs, &e.message);
                last = e.message;
                if stop {
                    break;
                }
            }
        }
    }
    tally.done(app);
    Err(last)
}

/// Brains that can use tools first, in the order they were in (a request with tools).
pub fn tools_first(order: Vec<Who>) -> Vec<Who> {
    let can = |w: &Who| !matches!(w, Who::Local) && crate::brains::get(w.id()).is_some_and(|b| b.caps().tools) || matches!(w, Who::Local);
    let (mut with, without): (Vec<Who>, Vec<Who>) = order.into_iter().partition(can);
    with.extend(without);
    with
}

/// One question, one answer, no history (Flow's clean-up, the notes' summary).
pub async fn complete(system: &str, user: &str, max_tokens: u32, timeout: Duration) -> Result<String, String> {
    let app = app()?;
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    let messages = [json!({ "role": "user", "content": user })];
    let req = crate::brains::Request { system, messages: &messages, tools: &[], max_tokens, timeout };
    ask(app, &s, &prefs, current_order_for(&s, "writing"), req).await.map(|reply| crate::brains::text_of(&reply))
}

/// One turn of a conversation with tools, in and out in Anthropic's Messages
/// shape: `{ content: [blocks], stop_reason }`.
pub async fn tool_turn(system: &str, messages: &[Value], tools: &[Value]) -> Result<Value, String> {
    let app = app()?;
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    let req = crate::brains::Request { system, messages, tools, max_tokens: 1024, timeout: Duration::from_secs(60) };
    ask(app, &s, &prefs, tools_first(current_order_for(&s, "commands")), req).await
}

/// How the cloud brains fared on one request (round 3D item 10): all
/// unreachable means the network is down, all out of their limit means the
/// cloud is used up; either makes Groovy Offline until the cloud answers again.
#[derive(Default)]
struct CloudTally {
    tried: u32,
    network: u32,
    limits: u32,
}

impl CloudTally {
    fn failed(&mut self, who: Who, fault: crate::brains::Fault) {
        if matches!(who, Who::Claude | Who::Cloud(_)) {
            self.tried += 1;
            match fault {
                crate::brains::Fault::Network => self.network += 1,
                crate::brains::Fault::Limit => self.limits += 1,
                _ => {}
            }
        }
    }

    fn answered(&self, app: &AppHandle, who: Who) {
        if matches!(who, Who::Claude | Who::Cloud(_)) && crate::resources::state::state() == crate::resources::state::RunState::Offline {
            crate::resources::state::fire(app, crate::resources::state::Event::NetworkUp);
        }
    }

    fn done(&self, app: &AppHandle) {
        if let Some(e) = self.verdict() {
            crate::resources::state::fire(app, e);
        }
    }

    fn verdict(&self) -> Option<crate::resources::state::Event> {
        use crate::resources::state::Event;
        if self.tried == 0 {
            None
        } else if self.network == self.tried {
            Some(Event::NetworkDown)
        } else if self.limits == self.tried {
            Some(Event::CloudExhausted)
        } else {
            None
        }
    }
}

// ── Format conversion (pure) ──────────────────────────────────────────────────

/// Reasoning models' `<think>` blocks are not part of the answer.
pub fn strip_thinking(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// Anthropic tool definitions → OpenAI functions. Server tools (web search) have no local equivalent and are left out.
pub fn to_openai_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter(|t| t.get("input_schema").is_some())
        .map(|t| {
            json!({
                "type": "function",
                "function": { "name": t["name"], "description": t["description"], "parameters": t["input_schema"] }
            })
        })
        .collect()
}

/// Anthropic messages (text, tool_use and tool_result blocks) → OpenAI messages.
pub fn to_openai_messages(system: &str, messages: &[Value]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        let role = m["role"].as_str().unwrap_or("user");
        match &m["content"] {
            Value::String(s) => out.push(json!({ "role": role, "content": s })),
            Value::Array(blocks) => {
                let mut text = String::new();
                let mut calls = Vec::new();
                for b in blocks {
                    match b["type"].as_str() {
                        Some("text") => text.push_str(b["text"].as_str().unwrap_or("")),
                        Some("tool_use") => calls.push(json!({
                            "id": b["id"],
                            "type": "function",
                            "function": { "name": b["name"], "arguments": b["input"].to_string() }
                        })),
                        Some("tool_result") => {
                            let content = match &b["content"] {
                                Value::String(s) => s.clone(),
                                Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                                other => other.to_string(),
                            };
                            out.push(json!({ "role": "tool", "tool_call_id": b["tool_use_id"], "content": content }));
                        }
                        _ => {}
                    }
                }
                if role == "assistant" {
                    let mut msg = json!({ "role": "assistant", "content": text });
                    if !calls.is_empty() {
                        msg["tool_calls"] = Value::Array(calls);
                    }
                    out.push(msg);
                } else if !text.is_empty() {
                    out.push(json!({ "role": role, "content": text }));
                }
            }
            _ => {}
        }
    }
    out
}

/// An OpenAI chat completion → Anthropic's `{ content, stop_reason }`.
pub fn from_openai(v: &Value) -> Value {
    let msg = &v["choices"][0]["message"];
    let mut content = Vec::new();
    let text = strip_thinking(msg["content"].as_str().unwrap_or(""));
    if !text.is_empty() {
        content.push(json!({ "type": "text", "text": text }));
    }
    let mut used_tool = false;
    if let Some(calls) = msg["tool_calls"].as_array() {
        for (i, c) in calls.iter().enumerate() {
            let args = &c["function"]["arguments"];
            let input = match args {
                Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({})),
                Value::Object(_) => args.clone(),
                _ => json!({}),
            };
            let id = c["id"].as_str().map(str::to_string).unwrap_or_else(|| format!("call_{i}"));
            content.push(json!({ "type": "tool_use", "id": id, "name": c["function"]["name"], "input": input }));
            used_tool = true;
        }
    }
    json!({ "content": content, "stop_reason": if used_tool { "tool_use" } else { "end_turn" } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cloud_unreachable_or_used_up_means_offline() {
        use crate::resources::state::Event;
        let mut t = CloudTally::default();
        t.failed(Who::Claude, crate::brains::BrainError::of("Network error: error sending request").fault);
        t.failed(Who::Cloud("google"), crate::brains::BrainError::of("Network error: dns error").fault);
        t.failed(Who::Local, crate::brains::BrainError::of("Couldn't start llama-server").fault);
        assert_eq!(t.verdict(), Some(Event::NetworkDown), "every cloud brain unreachable");
        let mut t = CloudTally::default();
        t.failed(Who::Claude, crate::brains::BrainError::of("429 rate limit").fault);
        t.failed(Who::Cloud("openai"), crate::brains::BrainError::of("quota exceeded").fault);
        assert_eq!(t.verdict(), Some(Event::CloudExhausted));
        let mut t = CloudTally::default();
        t.failed(Who::Claude, crate::brains::BrainError::of("429 rate limit").fault);
        t.failed(Who::Cloud("openai"), crate::brains::BrainError::of("Network error: timed out").fault);
        assert_eq!(t.verdict(), None, "mixed: nothing to conclude");
        assert_eq!(CloudTally::default().verdict(), None, "no cloud brain tried");
    }

    #[test]
    fn the_users_order_and_a_jobs_brain_rearrange_but_this_pc_stays_last() {
        let base = vec![Who::Claude, Who::Cloud("google"), Who::Cloud("openai"), Who::Local];
        let order = vec!["openai".to_string(), "anthropic".into(), "local".into(), "google".into()];
        assert_eq!(arrange(base.clone(), &order, None), vec![Who::Cloud("openai"), Who::Claude, Who::Cloud("google"), Who::Local]);
        assert_eq!(arrange(base.clone(), &[], None), base, "no order: as built");
        assert_eq!(arrange(base.clone(), &[], Some("google"))[0], Who::Cloud("google"), "the job's brain first");
        assert_eq!(arrange(base.clone(), &[], Some("auto")), base);
        assert_eq!(arrange(base.clone(), &[], Some("codex")), base, "a brain this PC hasn't is ignored");
    }

    #[test]
    fn a_limit_stops_the_switching_only_when_asked() {
        let mut p = AssistantPrefs::default();
        assert!(p.switch_on_limit, "on by default");
        assert!(may_switch(&p, "HTTP 429: quota"));
        p.switch_on_limit = false;
        assert!(!may_switch(&p, "HTTP 429: quota"));
        assert!(may_switch(&p, "connection refused"), "other failures still fall through");
    }

    #[test]
    fn a_limit_is_told_apart_from_other_failures() {
        assert!(is_limit("HTTP 429: Resource has been exhausted (e.g. check quota)."));
        assert!(is_limit("Claude usage limit reached"));
        assert!(!is_limit("invalid x-api-key"));
        assert!(!is_limit("connection refused"));
    }

    #[test]
    fn web_search_is_dropped_for_local_models() {
        let tools = vec![
            json!({ "name": "open_app", "description": "Open an app", "input_schema": { "type": "object" } }),
            json!({ "type": "web_search_20250305", "name": "web_search", "max_uses": 3 }),
        ];
        let out = to_openai_tools(&tools);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["function"]["name"], "open_app");
        assert_eq!(out[0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn a_tool_round_trip_converts_both_ways() {
        let messages = vec![
            json!({ "role": "user", "content": "open firefox" }),
            json!({ "role": "assistant", "content": [
                { "type": "text", "text": "Opening it." },
                { "type": "tool_use", "id": "t1", "name": "open_app", "input": { "name": "Firefox" } }
            ]}),
            json!({ "role": "user", "content": [ { "type": "tool_result", "tool_use_id": "t1", "content": "opened" } ] }),
        ];
        let out = to_openai_messages("sys", &messages);
        assert_eq!(out[0], json!({ "role": "system", "content": "sys" }));
        assert_eq!(out[1]["content"], "open firefox");
        assert_eq!(out[2]["tool_calls"][0]["function"]["arguments"], "{\"name\":\"Firefox\"}");
        assert_eq!(out[3], json!({ "role": "tool", "tool_call_id": "t1", "content": "opened" }));

        let reply = json!({ "choices": [{ "message": {
            "content": "<think>hmm</think>Sure.",
            "tool_calls": [{ "id": "c9", "type": "function", "function": { "name": "music", "arguments": "{\"action\":\"next\"}" } }]
        }}]});
        let back = from_openai(&reply);
        assert_eq!(back["stop_reason"], "tool_use");
        assert_eq!(back["content"][0], json!({ "type": "text", "text": "Sure." }));
        assert_eq!(back["content"][1]["input"]["action"], "next");
        let plain = from_openai(&json!({ "choices": [{ "message": { "content": "Hi" } }] }));
        assert_eq!(plain["stop_reason"], "end_turn");
    }

    #[test]
    fn cloud_first_local_last_and_only_when_chosen() {
        let p = |mode: &str, brain: &str, setup: &str| AssistantPrefs { ai_mode: mode.into(), brain: brain.into(), ai_setup: setup.into(), ..Default::default() };
        let keys = |list: &'static [&'static str]| move |k: &str| list.contains(&k);
        let none = |_: &str| false;
        // No keys, nothing chosen: nobody — not a silent local model.
        assert!(order(&p("hybrid", "auto", ""), "ollama", none, none).is_empty());
        // A Gemini key: Gemini.
        assert_eq!(order(&p("hybrid", "auto", "cloud"), "ollama", keys(&["google"]), none), vec![Who::Cloud("google")]);
        // The chat's provider first, then the other keys, then local when chosen.
        assert_eq!(
            order(&p("hybrid", "auto", "local"), "openai", keys(&["google", "openai"]), none),
            vec![Who::Cloud("openai"), Who::Cloud("google"), Who::Local]
        );
        // A subscription only when chosen at setup, and only if its CLI is installed.
        let cli = |t: &str| t == "codex";
        assert_eq!(order(&p("hybrid", "auto", "subscription"), "ollama", none, cli), vec![Who::Cli("codex")]);
        assert!(order(&p("hybrid", "auto", "cloud"), "ollama", none, cli).is_empty());
        // Explicit choices.
        assert_eq!(order(&p("online", "claude", ""), "x", none, none), vec![Who::Claude]);
        assert_eq!(order(&p("hybrid", "claude", "local"), "x", none, none), vec![Who::Claude, Who::Local]);
        assert_eq!(order(&p("hybrid", "local", ""), "x", none, none), vec![], "local brain needs local AI chosen");
        assert_eq!(order(&p("local", "claude", ""), "x", keys(&["anthropic"]), none), vec![Who::Local], "local-only never reaches the cloud");
        assert_eq!(order(&p("online", "auto", "local"), "x", none, none), vec![], "online-only never starts a local model");
    }

    #[test]
    fn a_conversation_becomes_one_prompt_for_the_clis() {
        let m = vec![json!({ "role": "user", "content": "next song" }), json!({ "role": "assistant", "content": [{ "type": "text", "text": "Done." }] })];
        let p = crate::brains::cli::as_prompt("You are Groovy.", &m);
        assert!(p.starts_with("You are Groovy."));
        assert!(p.contains("User: next song\nYou: Done.\n"));
    }

    #[test]
    fn thinking_is_removed() {
        assert_eq!(strip_thinking("<think>a</think> b <think>c</think>d"), "b d");
        assert_eq!(strip_thinking("x <think>never closed"), "x");
    }
}
