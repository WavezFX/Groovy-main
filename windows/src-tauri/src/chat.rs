// The chat, whoever answers it: the conversation, the system prompt, and which
// provider a turn goes to. Each provider's own wire format lives in its module:
// claude.rs (Anthropic), openai_compat.rs (OpenAI, Google AI, OpenRouter) and
// local_chat.rs (Ollama, LM Studio, any OpenAI-compatible server).
//
// API keys never leave the credential store and file bytes never cross the IPC
// boundary: the island sends the question and gets the answer's text back.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::assistant::{self, AssistantPrefs};
use crate::settings::Settings;
use crate::{claude, local_chat, memory, openai_compat, secrets};

pub const ANTHROPIC: &str = "anthropic";
/// Groovy's own engine: llama.cpp running a GGUF model (models.rs).
pub const LLAMACPP: &str = "llamacpp";

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatContext {
    File { name: String, path: String },
    Window { app_name: String, title: String, url: Option<String> },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatReply {
    pub text: String,
    /// A desktop tool waiting for the user's approval (tools.rs): the chat
    /// shows Allow once / Always / Don't under the answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<crate::tools::PendingAction>,
}

impl ChatReply {
    pub fn text(text: String) -> Self {
        ChatReply { text, action: None }
    }
}

/// A model a provider offers, for the picker in the chat view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
}

// ── Conversation ──────────────────────────────────────────────────────────────

/// The conversation is kept twice: in the wire format of the provider that
/// answered last (Claude's carries web search blocks no other provider would
/// understand), and as plain text turns. Switching provider mid-conversation
/// rebuilds the history from the plain turns, so nothing in one provider's
/// format is ever sent to another.
#[derive(Default)]
pub struct Chat {
    inner: Mutex<Conversation>,
    /// Personality and memories for the system prompt (assistant.rs, memory.rs),
    /// refreshed before every turn.
    instructions: Mutex<Instructions>,
    /// The chat history row this conversation is saved under (memory.rs).
    saved_as: Mutex<Option<i64>>,
    /// This turn was spoken in a voice call: short answers, for the ear.
    spoken: std::sync::atomic::AtomicBool,
}

/// Added to the system prompt for a voice call turn: light local models
/// answer quickly, and every answer is easy to listen to.
pub const VOICE_RULE: &str = "This is a spoken voice call: reply in 1-2 sentences unless asked for more, in plain words (no lists, headings, markdown or emoji).";

#[derive(Default, Clone)]
struct Instructions {
    name: String,
    text: String,
    /// The whole system prompt, instead of Groovy's chat one (the Coding Studio).
    replace: Option<String>,
}

#[derive(Default)]
struct Conversation {
    /// Bumped by every reset, so an answer that lands after "New chat" is dropped.
    epoch: u64,
    /// The provider `native` belongs to.
    owner: Option<String>,
    native: Vec<Value>,
    /// `{"role", "content": text}` turns, the same whoever answered.
    plain: Vec<Value>,
}

/// What a provider needs to build one turn.
pub struct Turn {
    epoch: u64,
    provider: String,
    /// No earlier turn: the file or window context rides along with this one.
    pub first: bool,
    /// The earlier turns, in this provider's format.
    pub history: Vec<Value>,
}

impl Chat {
    pub fn reset(&self) {
        let mut c = self.inner.lock().unwrap();
        let epoch = c.epoch + 1;
        *c = Conversation { epoch, ..Default::default() };
        *self.saved_as.lock().unwrap() = None;
    }

    /// What the next turns' system prompt says about who the assistant is and
    /// what it remembers.
    pub fn set_instructions(&self, name: &str, text: String) {
        *self.instructions.lock().unwrap() = Instructions { name: name.to_string(), text, replace: None };
    }

    /// A conversation with its own system prompt (studio.rs).
    pub fn with_system(system: &str) -> Self {
        let chat = Chat::default();
        *chat.instructions.lock().unwrap() = Instructions { replace: Some(system.to_string()), ..Default::default() };
        chat
    }

    /// The system prompt for this conversation's next turn.
    pub fn system_prompt(&self, web_search: bool) -> String {
        let i = self.instructions.lock().unwrap().clone();
        if let Some(system) = i.replace {
            return system;
        }
        let name = if i.name.is_empty() { "Groovy" } else { i.name.as_str() };
        let base = system_prompt_for(name, crate::identity::first_name(), web_search);
        let base = if i.text.is_empty() { base } else { format!("{base}\n\n{}", i.text) };
        if self.spoken.load(std::sync::atomic::Ordering::Relaxed) { format!("{base}\n\n{VOICE_RULE}") } else { base }
    }

    /// Whether the next turn comes from a voice call.
    pub fn set_spoken(&self, spoken: bool) {
        self.spoken.store(spoken, std::sync::atomic::Ordering::Relaxed);
    }

    /// Starts a turn with `provider`, converting the history if another
    /// provider answered the previous turns.
    pub fn begin(&self, provider: &str) -> Turn {
        let mut c = self.inner.lock().unwrap();
        if c.owner.as_deref() != Some(provider) {
            c.native = c.plain.clone();
            c.owner = Some(provider.to_string());
        }
        Turn {
            epoch: c.epoch,
            provider: provider.to_string(),
            first: c.plain.is_empty(),
            history: c.native.clone(),
        }
    }

    /// Records a finished turn: the user message and the answer in the
    /// provider's format, and their plain text. Only a successful turn is
    /// recorded, so the history always matches what the model saw.
    pub fn commit(&self, turn: &Turn, user: Value, assistant: Value, user_text: &str, answer: &str) {
        let mut c = self.inner.lock().unwrap();
        if c.epoch != turn.epoch || c.owner.as_deref() != Some(turn.provider.as_str()) {
            return;
        }
        c.native.push(user);
        c.native.push(assistant);
        c.plain.push(json!({ "role": "user", "content": user_text }));
        c.plain.push(json!({ "role": "assistant", "content": answer }));
    }
}

// ── System prompt ─────────────────────────────────────────────────────────────

/// Groovy's instructions. Greets the user by their first name when the account
/// has one worth using (identity.rs), and only claims web search where the
/// provider runs it (Claude). The personality (assistant.rs) follows it.
fn system_prompt_for(assistant: &str, first_name: Option<&str>, web_search: bool) -> String {
    let opening = match first_name {
        Some(name) => format!("You are {assistant}, {name}'s personal AI assistant living at the top of their screen."),
        None => format!("You are {assistant}, a personal AI assistant living at the top of the user's screen."),
    };
    let abilities = if web_search {
        "You have web search access and can help with absolutely anything — research, coding, finding places, recommendations, tasks, questions."
    } else {
        "You can help with absolutely anything — research, coding, recommendations, tasks, questions. You have no web access: say so when something needs current information."
    };
    format!(
        "{opening} {abilities} \
Use light Markdown when it helps: short paragraphs, bullet lists, **bold**, `inline code` and fenced code blocks. Avoid tables and big headings: the chat window is small."
    )
}

/// The window context line, as ClaudeService.chat() writes it.
pub fn window_line(app_name: &str, title: &str, url: Option<&str>) -> String {
    let mut text = format!("Context — App: {app_name}, Window: {title}");
    if let Some(url) = url {
        text.push_str(&format!(", URL: {url}"));
    }
    text
}

/// The plain-text record of what the user asked, context included.
pub fn plain_question(first: bool, context: Option<&ChatContext>, query: &str) -> String {
    match context.filter(|_| first) {
        Some(ChatContext::File { name, .. }) => format!("File: {name}\n\n{query}"),
        Some(ChatContext::Window { app_name, title, url }) => {
            format!("{}\n\n{query}", window_line(app_name, title, url.as_deref()))
        }
        None => query.to_string(),
    }
}

// ── Which provider ────────────────────────────────────────────────────────────

/// The model chosen for `provider`, or its default.
pub fn model_for(settings: &Settings, provider: &str) -> String {
    if provider == ANTHROPIC {
        let m = settings.model.trim();
        return if m.is_empty() { claude::DEFAULT_MODEL.to_string() } else { m.to_string() };
    }
    settings
        .chat_models
        .get(provider)
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .or_else(|| openai_compat::provider(provider).map(|p| p.default_model.to_string()))
        .unwrap_or_default()
}

/// A file rides along only if it is one of Groovy's own copies of a dropped
/// file (files.rs puts them in the inbox). The page names the path, so without
/// this any file the user can read could be sent to a chat provider.
fn checked_context(context: ChatContext) -> Result<ChatContext, String> {
    match context {
        ChatContext::File { name, path } => {
            let inbox = crate::files::inbox_dir();
            if !is_inside(&inbox, std::path::Path::new(&path)) {
                return Err(crate::i18n::t("Only a file dropped on the island can be sent with a question."));
            }
            Ok(ChatContext::File { name, path })
        }
        other => Ok(other),
    }
}

/// True when `path` is a regular file directly inside `dir`, both resolved
/// (no `..`, no symlink pointing out of it).
fn is_inside(dir: &std::path::Path, path: &std::path::Path) -> bool {
    let (Ok(dir), Ok(file)) = (dir.canonicalize(), path.canonicalize()) else { return false };
    file.parent() == Some(dir.as_path())
        && std::fs::symlink_metadata(&file).map(|m| m.is_file()).unwrap_or(false)
}

/// The personality and, when memory is on, what is remembered: the part of
/// the system prompt that follows Groovy's own instructions.
pub fn instructions(prefs: &AssistantPrefs) -> String {
    let mut text = assistant::personality_prompt(prefs);
    if prefs.memory_enabled {
        if let Ok(conn) = crate::db::conn() {
            let section = memory::prompt_section(&conn, memory::PROMPT_BUDGET);
            if !section.is_empty() {
                text.push_str("\n\n");
                text.push_str(&section);
            }
        }
    }
    text
}

fn today() -> String {
    let t = crate::platform::local_time();
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// One chat turn: "remember that…" is handled here, on this computer; anything
/// else goes where the AI mode says (assistant.rs), falling back to the local
/// model in hybrid mode when the online provider can't answer. The finished
/// turn is saved to the chat history when history is on.
pub async fn send(
    app: &AppHandle,
    chat: &Chat,
    settings: &Settings,
    query: String,
    context: Option<ChatContext>,
) -> Result<ChatReply, String> {
    let context = context.map(checked_context).transpose()?;
    let prefs = settings.assistant.clone().sanitised();
    let plain = plain_question(chat.inner.lock().unwrap().plain.is_empty(), context.as_ref(), &query);

    if prefs.memory_enabled {
        if let Some((kind, fact)) = memory::remember_request(&query) {
            let reply = match crate::db::conn().and_then(|c| memory::add(&c, kind, &fact)) {
                Ok(m) => crate::i18n::tf("Got it — I'll remember that: {fact}", &[("fact", &m.content)]),
                Err(e) => e,
            };
            save_turn(chat, &prefs, &plain, &reply, "memory");
            return Ok(ChatReply::text(reply));
        }
    }

    // "Pause the music", "play my favourite songs"… run here, on this computer,
    // and the answer says what really happened.
    if let Some(cmd) = crate::music::parse_command(&query).filter(crate::music::worth_handling) {
        let reply = match crate::music::execute(&cmd) {
            Ok(text) | Err(text) => text,
        };
        save_turn(chat, &prefs, &plain, &reply, "music");
        return Ok(ChatReply::text(reply));
    }
    // "Remind me to…", "open Notepad", "how's my battery?"… (tools.rs), with
    // the permission the user gave each tool.
    if let Some(call) = crate::tools::parse(&query) {
        let (reply, action) = crate::tools::handle(call);
        save_turn(chat, &prefs, &plain, &reply, "tools");
        return Ok(ChatReply { text: reply, action });
    }

    chat.set_instructions(&prefs.name, instructions(&prefs));
    let picked = if settings.chat_provider.is_empty() { ANTHROPIC } else { settings.chat_provider.as_str() };
    let limit_reached = prefs.daily_online_limit > 0
        && crate::db::conn().map(|c| crate::db::usage_online(&c, &today()) >= prefs.daily_online_limit).unwrap_or(false);
    let route = assistant::route(&prefs, picked, &settings.custom_url, limit_reached);

    let first = send_to(app, chat, settings, &prefs, &route.primary, query.clone(), context.clone()).await;
    let (mut reply, used, note) = match (first, route.fallback) {
        (Ok(r), _) => (r, route.primary.clone(), route.note.map(crate::i18n::t)),
        (Err(e), Some(fallback)) if assistant::worth_falling_back(&e) => {
            crate::log::line(format!("chat: {} unavailable, answering with {fallback}", route.primary));
            match send_to(app, chat, settings, &prefs, &fallback, query, context).await {
                Ok(r) => (r, fallback, Some(crate::i18n::t("The online provider couldn't be reached, so the model on this computer answered."))),
                Err(e2) => return Err(format!("{e}\n\n{}: {e2}", crate::i18n::t("The local fallback failed too"))),
            }
        }
        (Err(e), _) => return Err(e),
    };
    if !assistant::is_local(&used, &settings.custom_url) {
        if let Ok(c) = crate::db::conn() {
            let _ = crate::db::usage_bump(&c, &today(), &used);
        }
    }
    save_turn(chat, &prefs, &plain, &reply.text, &used);
    if let Some(note) = note {
        reply.text = format!("_{note}_\n\n{}", reply.text);
    }
    Ok(reply)
}

/// Who would answer now with these settings, and whether it is on this computer.
pub fn target(settings: &Settings) -> (String, bool) {
    let prefs = settings.assistant.clone().sanitised();
    let picked = if settings.chat_provider.is_empty() { ANTHROPIC } else { settings.chat_provider.as_str() };
    let limit_reached = prefs.daily_online_limit > 0
        && crate::db::conn().map(|c| crate::db::usage_online(&c, &today()) >= prefs.daily_online_limit).unwrap_or(false);
    let route = assistant::route(&prefs, picked, &settings.custom_url, limit_reached);
    let local = assistant::is_local(&route.primary, &settings.custom_url);
    (route.primary, local)
}

/// "Test my voice setup": one spoken-style question to whoever answers the
/// chat now, outside the conversation (nothing saved). Returns (answer, who).
pub async fn probe(app: &AppHandle, settings: &Settings, question: &str) -> Result<(String, String), String> {
    let (who, _) = target(settings);
    let system = format!("{}\n\n{VOICE_RULE}", system_prompt_for("Groovy", crate::identity::first_name(), false));
    let answer = complete(app, settings, &system, question.to_string(), |_| {}).await?;
    Ok((answer, who))
}

/// One question with its own system prompt, outside the chat's conversation
/// (the Coding Studio). Same routing and fallback as the chat; `on_delta`
/// gets the text so far (streamed by local models, at once by the others).
pub async fn complete(
    app: &AppHandle,
    settings: &Settings,
    system: &str,
    prompt: String,
    mut on_delta: impl FnMut(String) + Send,
) -> Result<String, String> {
    let prefs = settings.assistant.clone().sanitised();
    let picked = if settings.chat_provider.is_empty() { ANTHROPIC } else { settings.chat_provider.as_str() };
    let limit_reached = prefs.daily_online_limit > 0
        && crate::db::conn().map(|c| crate::db::usage_online(&c, &today()) >= prefs.daily_online_limit).unwrap_or(false);
    let route = assistant::route(&prefs, picked, &settings.custom_url, limit_reached);
    let mut tried = vec![route.primary.clone()];
    if let Some(f) = route.fallback.clone() {
        tried.push(f);
    }
    let mut last_err = String::new();
    for (i, provider) in tried.iter().enumerate() {
        let chat = Chat::with_system(system);
        let result = complete_with(app, &chat, settings, &prefs, provider, prompt.clone(), &mut on_delta).await;
        match result {
            Ok(text) => {
                if !assistant::is_local(provider, &settings.custom_url) {
                    if let Ok(c) = crate::db::conn() {
                        let _ = crate::db::usage_bump(&c, &today(), provider);
                    }
                }
                return Ok(text);
            }
            Err(e) if i == 0 && tried.len() > 1 && assistant::worth_falling_back(&e) => last_err = e,
            Err(e) => return Err(e),
        }
    }
    Err(last_err)
}

async fn complete_with(
    app: &AppHandle,
    chat: &Chat,
    settings: &Settings,
    prefs: &AssistantPrefs,
    provider: &str,
    prompt: String,
    on_delta: &mut (impl FnMut(String) + Send),
) -> Result<String, String> {
    if provider == assistant::NONE {
        return Err(crate::i18n::t("Today's online limit is reached. Raise it in Groovy → Assistant, or choose local AI there."));
    }
    let model = model_for(settings, provider);
    let reply = if provider == ANTHROPIC {
        claude::send(chat, &model, prompt, None).await?
    } else if let Some(p) = openai_compat::provider(provider) {
        openai_compat::send(chat, p, &model, prompt, None).await?
    } else if provider == LLAMACPP {
        let mut prefs = prefs.clone();
        if !model.is_empty() {
            prefs.local_model = model;
        }
        let url = crate::models::ensure_running(&prefs, Some(app)).await?;
        let server = local_chat::Server { id: LLAMACPP, name: "Groovy local model", url, key: None };
        let model = crate::models::running_model().unwrap_or_else(|| "local".into());
        let r = local_chat::send_with(chat, &server, &model, prompt, None, |t| on_delta(t)).await;
        crate::models::touch();
        r?
    } else if let Some(server) = local_chat::server(settings, provider) {
        let model = if model.is_empty() { local_chat::first_model(&server).await? } else { model };
        local_chat::send_with(chat, &server, &model, prompt, None, |t| on_delta(t)).await?
    } else {
        return Err(format!("Unknown chat provider: {provider}"));
    };
    on_delta(reply.text.clone());
    Ok(reply.text)
}

fn save_turn(chat: &Chat, prefs: &AssistantPrefs, user: &str, answer: &str, provider: &str) {
    if !prefs.history_enabled {
        return;
    }
    let Ok(conn) = crate::db::conn() else { return };
    let mut saved = chat.saved_as.lock().unwrap();
    match memory::record_turn(&conn, *saved, user, answer, provider) {
        Ok(id) => *saved = Some(id),
        Err(e) => crate::log::line(format!("chat history: {e}")),
    }
}

/// One turn with exactly `provider`.
async fn send_to(
    app: &AppHandle,
    chat: &Chat,
    settings: &Settings,
    prefs: &AssistantPrefs,
    provider: &str,
    query: String,
    context: Option<ChatContext>,
) -> Result<ChatReply, String> {
    if provider == assistant::NONE {
        return Err(crate::i18n::t("Today's online limit is reached. Raise it in Groovy → Assistant, or choose local AI there."));
    }
    let model = model_for(settings, provider);
    if provider == ANTHROPIC {
        return claude::send(chat, &model, query, context).await;
    }
    if let Some(p) = openai_compat::provider(provider) {
        return openai_compat::send(chat, p, &model, query, context).await;
    }
    if provider == LLAMACPP {
        // A model picked above the chat box wins over the one in Settings.
        let mut prefs = prefs.clone();
        if !model.is_empty() {
            prefs.local_model = model;
        }
        let url = crate::models::ensure_running(&prefs, Some(app)).await?;
        let server = local_chat::Server { id: LLAMACPP, name: "Groovy local model", url, key: None };
        let model = crate::models::running_model().unwrap_or_else(|| "local".into());
        let result = local_chat::send(app, chat, &server, &model, query, context).await;
        crate::models::touch();
        return result;
    }
    if let Some(server) = local_chat::server(settings, provider) {
        // No model picked yet: the first one the server has.
        let model = if model.is_empty() { local_chat::first_model(&server).await? } else { model };
        return local_chat::send(app, chat, &server, &model, query, context)
            .await
            .map_err(|e| if server.id == "ollama" && e.starts_with("Cannot reach") { local_chat::ollama_help() } else { e });
    }
    Err(format!("Unknown chat provider: {provider}"))
}

/// One turn of the phone's conversation (phone_link/chat_backend.rs): the same providers as
/// [`send`], with no file or window context and no `AppHandle`. `on_delta` gets the streamed
/// text so far, which only the model servers produce; the cloud providers answer all at once.
pub async fn send_for_phone(
    settings: &Settings,
    chat: &Chat,
    provider: &str,
    model: &str,
    query: String,
    on_delta: impl FnMut(String),
) -> Result<ChatReply, String> {
    let prefs = settings.assistant.clone().sanitised();
    chat.set_instructions(&prefs.name, instructions(&prefs));
    if provider == ANTHROPIC {
        return claude::send(chat, model, query, None).await;
    }
    if let Some(p) = openai_compat::provider(provider) {
        return openai_compat::send(chat, p, model, query, None).await;
    }
    if let Some(server) = local_chat::server(settings, provider) {
        return local_chat::send_with(chat, &server, model, query, None, on_delta).await;
    }
    Err(format!("Unknown chat provider: {provider}"))
}

/// The models `provider` offers. Asked only when the user opens the picker on
/// that provider, and only once it has a key (or, for a local server, an
/// address): nothing is sent anywhere before that.
pub async fn models(settings: &Settings, provider: &str) -> Result<Vec<ModelInfo>, String> {
    let no_key = || crate::i18n::t("No API key — add it in Settings.");
    if provider == ANTHROPIC {
        let key = secrets::get(claude::KEY).ok_or_else(no_key)?;
        return claude::models(&key).await;
    }
    if let Some(p) = openai_compat::provider(provider) {
        let key = secrets::get(p.key).ok_or_else(no_key)?;
        return openai_compat::models(p, &key).await;
    }
    if provider == LLAMACPP {
        let models = crate::models::discover(&crate::models::models_dir(), 2048);
        if models.is_empty() {
            return Err(crate::i18n::t("No local model yet: download one in Settings → Models."));
        }
        return Ok(models
            .into_iter()
            .filter(|m| m.info.is_some())
            .map(|m| ModelInfo { label: m.info.as_ref().map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or(m.file.clone()), id: m.file })
            .collect());
    }
    if let Some(server) = local_chat::server(settings, provider) {
        return local_chat::models(&server).await;
    }
    Err(format!("Unknown chat provider: {provider}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spoken_turn_asks_for_a_short_answer() {
        let chat = Chat::default();
        assert!(!chat.system_prompt(false).contains(VOICE_RULE));
        chat.set_spoken(true);
        assert!(chat.system_prompt(false).ends_with(VOICE_RULE));
        chat.set_spoken(false);
        assert!(!chat.system_prompt(false).contains(VOICE_RULE));
        // The Coding Studio's own prompt is left alone.
        let studio = Chat::with_system("studio");
        studio.set_spoken(true);
        assert_eq!(studio.system_prompt(false), "studio");
    }

    fn turn_texts(history: &[Value]) -> Vec<(String, Value)> {
        history
            .iter()
            .map(|m| (m["role"].as_str().unwrap().to_string(), m["content"].clone()))
            .collect()
    }

    #[test]
    fn a_turn_is_recorded_only_once_it_succeeds() {
        let chat = Chat::default();
        let t = chat.begin("anthropic");
        assert!(t.first);
        assert!(t.history.is_empty());
        // A failed turn records nothing: the next one is still the first.
        let t = chat.begin("anthropic");
        assert!(t.first);
        chat.commit(&t, json!({"role":"user","content":[{"type":"text","text":"hi"}]}), json!({"role":"assistant","content":[{"type":"text","text":"hello"}]}), "hi", "hello");
        let t = chat.begin("anthropic");
        assert!(!t.first);
        assert_eq!(t.history.len(), 2);
        assert!(t.history[0]["content"].is_array());
    }

    #[test]
    fn switching_provider_never_sends_claude_blocks_to_another_one() {
        let chat = Chat::default();
        let t = chat.begin("anthropic");
        let blocks = json!([
            {"type":"server_tool_use","id":"srvtoolu_1","name":"web_search","input":{"query":"x"}},
            {"type":"web_search_tool_result","tool_use_id":"srvtoolu_1","content":[]},
            {"type":"text","text":"Found it."}
        ]);
        chat.commit(&t, json!({"role":"user","content":[{"type":"text","text":"look"}]}), json!({"role":"assistant","content":blocks}), "look", "Found it.");

        let t = chat.begin("openai");
        assert!(!t.first);
        assert_eq!(
            turn_texts(&t.history),
            vec![("user".into(), json!("look")), ("assistant".into(), json!("Found it."))]
        );
        let raw = serde_json::to_string(&t.history).unwrap();
        assert!(!raw.contains("web_search"), "{raw}");

        // And back: Claude gets the plain turns too, including OpenAI's answer.
        chat.commit(&t, json!({"role":"user","content":"more"}), json!({"role":"assistant","content":"Sure."}), "more", "Sure.");
        let t = chat.begin("anthropic");
        assert_eq!(t.history.len(), 4);
        assert_eq!(t.history[3], json!({"role":"assistant","content":"Sure."}));
        assert!(!serde_json::to_string(&t.history).unwrap().contains("web_search"));
    }

    #[test]
    fn an_answer_that_lands_after_a_reset_or_a_switch_is_dropped() {
        let chat = Chat::default();
        let t = chat.begin("openai");
        chat.reset();
        chat.commit(&t, json!({"role":"user","content":"q"}), json!({"role":"assistant","content":"a"}), "q", "a");
        assert!(chat.begin("openai").first);

        let t = chat.begin("openai");
        let _other = chat.begin("google");
        chat.commit(&t, json!({"role":"user","content":"q"}), json!({"role":"assistant","content":"a"}), "q", "a");
        assert!(chat.begin("google").first);
    }

    #[test]
    fn the_prompt_greets_by_first_name_and_claims_web_search_only_for_claude() {
        let p = system_prompt_for("Groovy", Some("Louis"), true);
        assert!(p.starts_with("You are Groovy, Louis's personal AI assistant living at the top of their screen."));
        assert!(p.contains("web search access"));
        assert!(p.contains("light Markdown"));
        let p = system_prompt_for("Groovy", None, false);
        assert!(p.starts_with("You are Groovy, a personal AI assistant living at the top of the user's screen."));
        assert!(!p.contains("web search"));
        assert!(p.contains("no web access"));
    }

    #[test]
    fn the_personality_and_name_reach_the_system_prompt() {
        let chat = Chat::default();
        let base = chat.system_prompt(false);
        assert!(base.contains("You are Groovy,"));
        let prefs = AssistantPrefs { name: "Mochi".into(), personality: "quiet".into(), language: "German".into(), memory_enabled: false, ..Default::default() };
        chat.set_instructions(&prefs.name, instructions(&prefs));
        let p = chat.system_prompt(false);
        assert!(p.contains("You are Mochi,"));
        assert!(p.contains("calm and understated"));
        assert!(p.contains("Always respond in German"));
        assert!(!p.contains("What you remember"), "memory off adds nothing");
        // "New chat" keeps the personality: it is refreshed every turn anyway.
        chat.reset();
        assert!(chat.system_prompt(false).contains("You are Mochi,"));
    }

    #[test]
    fn context_goes_with_the_first_question_only() {
        let file = ChatContext::File { name: "a.txt".into(), path: "/x/a.txt".into() };
        assert_eq!(plain_question(true, Some(&file), "why?"), "File: a.txt\n\nwhy?");
        assert_eq!(plain_question(false, Some(&file), "why?"), "why?");
        let win = ChatContext::Window { app_name: "Code".into(), title: "main.rs".into(), url: None };
        assert_eq!(plain_question(true, Some(&win), "q"), "Context — App: Code, Window: main.rs\n\nq");
        assert_eq!(window_line("Edge", "Docs", Some("https://x.dev")), "Context — App: Edge, Window: Docs, URL: https://x.dev");
    }

    #[test]
    fn the_model_comes_from_the_settings_or_the_provider_default() {
        let mut s = Settings::default();
        assert_eq!(model_for(&s, "anthropic"), claude::DEFAULT_MODEL);
        assert_eq!(model_for(&s, "openai"), openai_compat::provider("openai").unwrap().default_model);
        assert_eq!(model_for(&s, "ollama"), "");
        s.chat_models.insert("openai".into(), " gpt-x ".into());
        s.chat_models.insert("ollama".into(), "llama3.2".into());
        s.model = "claude-haiku-4-5".into();
        assert_eq!(model_for(&s, "openai"), "gpt-x");
        assert_eq!(model_for(&s, "ollama"), "llama3.2");
        assert_eq!(model_for(&s, "anthropic"), "claude-haiku-4-5");
    }

    #[test]
    fn only_files_in_the_inbox_ride_along() {
        let base = std::env::temp_dir().join(format!("groovy-chat-ctx-{}", std::process::id()));
        let inbox = base.join("inbox");
        std::fs::create_dir_all(inbox.join("sub")).unwrap();
        std::fs::write(inbox.join("a.txt"), b"a").unwrap();
        std::fs::write(base.join("secret.txt"), b"s").unwrap();
        std::fs::write(inbox.join("sub").join("b.txt"), b"b").unwrap();
        assert!(is_inside(&inbox, &inbox.join("a.txt")));
        assert!(!is_inside(&inbox, &base.join("secret.txt")));
        assert!(!is_inside(&inbox, &inbox.join("..").join("secret.txt")));
        assert!(!is_inside(&inbox, &inbox.join("sub").join("b.txt")));
        assert!(!is_inside(&inbox, &inbox.join("missing.txt")));
        assert!(!is_inside(&inbox, &inbox));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("secret.txt"), inbox.join("link.txt")).unwrap();
            assert!(!is_inside(&inbox, &inbox.join("link.txt")));
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
