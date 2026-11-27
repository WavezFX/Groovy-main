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
// The order and the switching live in brains/switch.rs (round 4D item 2).
pub use crate::brains::switch::{Job, Who};
use crate::brains::switch::{current_order, current_order_for};

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

/// The app, once Groovy has started (for callers without one: the phone).
pub(crate) fn app_handle() -> Option<&'static AppHandle> {
    APP.get()
}

fn settings(app: &AppHandle) -> Settings {
    app.try_state::<crate::Shared>().map(|s| s.settings.lock().unwrap().clone()).unwrap_or_default()
}


#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clis {
    claude_code: bool,
    codex: bool,
    gemini: bool,
}

/// Which subscription CLIs this PC has, for the AI setup.
#[tauri::command]
pub async fn ai_clis() -> Clis {
    tauri::async_runtime::spawn_blocking(|| Clis {
        claude_code: crate::brains::cli::path("claude-code").is_some(),
        codex: crate::brains::cli::path("codex").is_some(),
        gemini: crate::brains::cli::path("gemini-cli").is_some(),
    })
        .await
        .unwrap_or(Clis { claude_code: false, codex: false, gemini: false })
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
        Who::Cli("gemini-cli") => "Gemini",
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

pub(crate) fn local_configured(s: &Settings, prefs: &AssistantPrefs) -> bool {
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


/// One question, one answer, no history (Flow's clean-up, the notes' summary).
pub async fn complete(system: &str, user: &str, max_tokens: u32, timeout: Duration) -> Result<String, String> {
    let app = app()?;
    let messages = [json!({ "role": "user", "content": user })];
    let req = crate::brains::Request { system, messages: &messages, tools: &[], max_tokens, timeout, on_delta: None };
    crate::brains::switch::run(app, &settings(app), Job::WRITING, req).await.map(|(reply, _)| crate::brains::text_of(&reply))
}

/// One turn of a conversation with tools, in and out in Anthropic's Messages
/// shape: `{ content: [blocks], stop_reason }`.
pub async fn tool_turn(system: &str, messages: &[Value], tools: &[Value]) -> Result<Value, String> {
    let app = app()?;
    let req = crate::brains::Request { system, messages, tools, max_tokens: 1024, timeout: Duration::from_secs(60), on_delta: None };
    crate::brains::switch::run(app, &settings(app), Job::COMMANDS, req).await.map(|(reply, _)| reply)
}

/// Round 4C item 3: the brains, in the commands order, that can see a picture.
pub fn seeing_brains() -> Vec<&'static str> {
    let Ok(app) = app() else { return Vec::new() };
    current_order_for(&settings(app), "commands").into_iter().filter(|w| crate::brains::get(w.id()).is_some_and(|b| b.caps().vision)).map(|w| w.id()).collect()
}

/// An Anthropic image block as an OpenAI content part.
fn image_part(b: &Value) -> Option<Value> {
    let src = &b["source"];
    let data = src["data"].as_str()?;
    Some(json!({ "type": "image_url", "image_url": { "url": format!("data:{};base64,{data}", src["media_type"].as_str().unwrap_or("image/jpeg")) } }))
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
                // Round 4C: pictures (the user's, or a tool's) go after as one user message.
                let mut images: Vec<Value> = Vec::new();
                for b in blocks {
                    match b["type"].as_str() {
                        Some("text") => text.push_str(b["text"].as_str().unwrap_or("")),
                        Some("image") => images.extend(image_part(b)),
                        Some("tool_use") => calls.push(json!({
                            "id": b["id"],
                            "type": "function",
                            "function": { "name": b["name"], "arguments": b["input"].to_string() }
                        })),
                        Some("tool_result") => {
                            let content = match &b["content"] {
                                Value::String(s) => s.clone(),
                                Value::Array(parts) => {
                                    images.extend(parts.iter().filter(|p| p["type"] == "image").filter_map(image_part));
                                    parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n")
                                }
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
                } else if !images.is_empty() {
                    let mut parts = vec![json!({ "type": "text", "text": if text.is_empty() { "The picture:" } else { text.as_str() } })];
                    parts.append(&mut images);
                    out.push(json!({ "role": "user", "content": parts }));
                    images.clear();
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
    fn thinking_is_removed() {
        assert_eq!(strip_thinking("<think>a</think> b <think>c</think>d"), "b d");
        assert_eq!(strip_thinking("x <think>never closed"), "x");
    }
}
