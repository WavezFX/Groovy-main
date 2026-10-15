// Which model answers the background features ported from Coucou 0.5.0 —
// Flow's clean-up, the Notetaker's summary and « OK Groovy »'s tool calls —
// and how to talk to it.
//
// Free and local by default: the local model picked in Settings → Assistant
// (Groovy's own llama.cpp engine, Ollama, LM Studio or an OpenAI-compatible
// server on this computer), through the OpenAI chat-completions API that all
// of them speak, tool calls included. Claude is the optional choice
// ("brain": "claude"), with the user's own key; in hybrid mode it also stands
// in when the local model can't answer and a key is set. Local-only mode never
// sends anything to Claude.
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

/// Whether there is something to answer at all: a local model set up, or Claude chosen with a key.
pub fn available() -> bool {
    let Ok(app) = app() else { return false };
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    if prefs.brain == "claude" && prefs.ai_mode != "local" {
        return crate::secrets::present(crate::claude::KEY);
    }
    local_configured(&s, &prefs) || (prefs.ai_mode == "hybrid" && crate::secrets::present(crate::claude::KEY))
}

fn local_configured(s: &Settings, prefs: &AssistantPrefs) -> bool {
    match prefs.local_provider.as_str() {
        "llamacpp" => crate::models::server_exe().is_some() && !crate::models::discover(&crate::models::models_dir(), 2048).is_empty(),
        "ollama" | "lmstudio" => true,
        "custom" => !s.custom_url.trim().is_empty(),
        _ => false,
    }
}

/// Whether local models get no web search: they don't (no local search engine).
pub fn has_web_search() -> bool {
    let Ok(app) = app() else { return false };
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    prefs.brain == "claude" && prefs.ai_mode != "local" && crate::secrets::present(crate::claude::KEY)
}

struct Local {
    base: Url,
    model: String,
    key: Option<String>,
}

async fn local(app: &AppHandle, s: &Settings, prefs: &AssistantPrefs) -> Result<Local, String> {
    let id = prefs.local_provider.as_str();
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

async fn post(l: &Local, body: &Value, timeout: Duration) -> Result<Value, String> {
    let endpoint = Url::parse(&crate::net::join(&l.base, "/v1/chat/completions")).map_err(|e| e.to_string())?;
    let mut req = crate::net::client(&endpoint, timeout)?.post(endpoint.clone()).json(body);
    if let Some(k) = l.key.as_deref().filter(|k| !k.is_empty()) {
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

/// Who answers first, and who stands in.
fn order(prefs: &AssistantPrefs) -> Vec<&'static str> {
    let key = crate::secrets::present(crate::claude::KEY);
    match (prefs.ai_mode.as_str(), prefs.brain.as_str()) {
        ("local", _) => vec!["local"],
        ("online", "claude") => vec!["claude"],
        (_, "claude") => vec!["claude", "local"],
        ("hybrid", _) if key => vec!["local", "claude"],
        _ => vec!["local"],
    }
}

/// One question, one answer, no history (Flow's clean-up, the notes' summary).
pub async fn complete(system: &str, user: &str, max_tokens: u32, timeout: Duration) -> Result<String, String> {
    let app = app()?;
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    let mut last = String::from("No AI model available.");
    for who in order(&prefs) {
        let r = if who == "claude" {
            crate::claude::complete(CLAUDE_FAST, system, user, max_tokens, timeout).await
        } else {
            match local(app, &s, &prefs).await {
                Ok(l) => {
                    let body = json!({
                        "model": l.model,
                        "max_tokens": max_tokens,
                        "stream": false,
                        "messages": [{ "role": "system", "content": system }, { "role": "user", "content": user }],
                    });
                    post(&l, &body, timeout).await.and_then(|v| {
                        let text = strip_thinking(v["choices"][0]["message"]["content"].as_str().unwrap_or(""));
                        if text.trim().is_empty() { Err("empty answer".into()) } else { Ok(text) }
                    })
                }
                Err(e) => Err(e),
            }
        };
        match r {
            Ok(t) => return Ok(t),
            Err(e) => {
                crate::log::line(format!("brain ({who}): {e}"));
                last = e;
            }
        }
    }
    Err(last)
}

/// One turn of a conversation with tools, in and out in Anthropic's Messages
/// shape: `{ content: [blocks], stop_reason }`.
pub async fn tool_turn(system: &str, messages: &[Value], tools: &[Value]) -> Result<Value, String> {
    let app = app()?;
    let s = settings(app);
    let prefs = s.assistant.clone().sanitised();
    let mut last = String::from("No AI model available.");
    for who in order(&prefs) {
        let r = if who == "claude" {
            crate::claude::voice_turn(CLAUDE_FAST, system, messages, tools).await
        } else {
            match local(app, &s, &prefs).await {
                Ok(l) => {
                    let body = json!({
                        "model": l.model,
                        "max_tokens": 1024,
                        "stream": false,
                        "messages": to_openai_messages(system, messages),
                        "tools": to_openai_tools(tools),
                    });
                    post(&l, &body, Duration::from_secs(60)).await.map(|v| from_openai(&v))
                }
                Err(e) => Err(e),
            }
        };
        match r {
            Ok(v) => return Ok(v),
            Err(e) => {
                crate::log::line(format!("brain ({who}): {e}"));
                last = e;
            }
        }
    }
    Err(last)
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
    fn local_first_unless_claude_is_chosen() {
        let p = |mode: &str, brain: &str| AssistantPrefs { ai_mode: mode.into(), brain: brain.into(), ..Default::default() };
        assert_eq!(order(&p("local", "claude")), vec!["local"], "local-only never reaches Claude");
        assert_eq!(order(&p("online", "claude")), vec!["claude"]);
        assert_eq!(order(&p("hybrid", "claude")), vec!["claude", "local"]);
        assert_eq!(order(&p("online", "local"))[0], "local");
    }

    #[test]
    fn thinking_is_removed() {
        assert_eq!(strip_thinking("<think>a</think> b <think>c</think>d"), "b d");
        assert_eq!(strip_thinking("x <think>never closed"), "x");
    }
}
