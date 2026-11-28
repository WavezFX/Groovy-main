// Round 6 (FIXES_ROUND6.md): Discord through the installed app's local RPC
// pipe — mute, deafen, the voice channel, who's speaking, leave the call.
//
// Discord's rules (checked 27 Nov 2026): RPC works for an unapproved app only
// for its owner and up to 50 testers, and the rpc / rpc.voice.* scopes are
// "approved partners only". So Groovy never ships an app of its own: the
// owner registers a free one, and its client ID is a setting. The optional
// client secret and the tokens live in Credential Manager. Messages are never
// read or sent through Discord's API (no user token, ever): they come from
// Windows' notifications (island_tiles.rs) and the user presses Enter.
//
// Lazy (round 3D item 9): one connection task, only while the island shows
// the Discord tile (a 20 s lease it renews) or a command runs; it closes on
// its own after the lease. Nothing starts with Groovy.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::i18n::{t, tf};

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;
/// Discord's frames are small; anything bigger is not Discord.
const MAX_FRAME: usize = 1 << 20;
const SCOPES: [&str; 4] = ["rpc", "rpc.voice.read", "rpc.voice.write", "identify"];
const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
/// How long the connection stays after the tile was last seen or a command ran.
pub const LEASE: Duration = Duration::from_secs(20);
const CALL_TIMEOUT: Duration = Duration::from_secs(6);
/// The user reads and approves Discord's "Authorise?" window.
const AUTHORISE_TIMEOUT: Duration = Duration::from_secs(120);

const KV_CLIENT_ID: &str = "discord-client-id";
pub const SECRET_KEY: &str = "discord-client-secret";
pub const TOKENS_KEY: &str = "discord-tokens";

// ── Frames ────────────────────────────────────────────────────────────────────

/// One frame: opcode and length (little-endian u32 each), then the JSON.
pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, op: u32, body: &Value) -> std::io::Result<()> {
    let data = serde_json::to_vec(body)?;
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&data);
    w.write_all(&buf).await?;
    w.flush().await
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    r.read_exact(&mut head).await?;
    let op = u32::from_le_bytes(head[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too big"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    let v = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))? };
    Ok((op, v))
}

// ── What the tile shows ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub name: String,
    pub speaking: bool,
    pub muted: bool,
    pub deafened: bool,
    pub me: bool,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub guild_id: Option<String>,
}

/// A Discord notification, as Windows showed it (memory only).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: u32,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct State {
    /// "off" (not connected), "not-set-up", "not-running", "needs-auth", "refused", "ready".
    pub status: String,
    /// A sentence for the tile when it isn't ready.
    pub message: String,
    pub mute: bool,
    pub deaf: bool,
    pub channel: Option<Channel>,
    pub members: Vec<Member>,
    pub notes: Vec<Note>,
    /// The client ID is saved (Settings → Discord).
    pub set_up: bool,
    /// Groovy was approved in Discord (tokens are stored).
    pub authorised: bool,
}

impl Default for State {
    fn default() -> Self {
        State { status: "off".into(), message: String::new(), mute: false, deaf: false, channel: None, members: Vec::new(), notes: Vec::new(), set_up: false, authorised: false }
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap();
    f(g.get_or_insert_with(State::default))
}

pub fn snapshot() -> State {
    with_state(|s| s.clone())
}

fn publish() {
    let s = snapshot();
    if let Some(app) = crate::brain_ai::app_handle() {
        use tauri::Emitter;
        let _ = app.emit_to(crate::island::WINDOW_LABEL, "discord-state", &s);
    }
}

/// Why a Discord command couldn't run: the kind decides the fallback.
#[derive(Debug, Clone, PartialEq)]
pub enum Fail {
    NotSetUp,
    NotRunning,
    NeedsAuth,
    Refused(String),
    Other(String),
}

impl Fail {
    pub fn status(&self) -> &'static str {
        match self {
            Fail::NotSetUp => "not-set-up",
            Fail::NotRunning => "not-running",
            Fail::NeedsAuth => "needs-auth",
            Fail::Refused(_) => "refused",
            Fail::Other(_) => "off",
        }
    }

    pub fn say(&self) -> String {
        match self {
            Fail::NotSetUp => t("Set up Discord in Settings → Discord first."),
            Fail::NotRunning => t("Discord isn't open."),
            Fail::NeedsAuth => t("Approve Groovy in Discord first: Settings → Discord → Connect."),
            Fail::Refused(m) => tf("Discord refused: {why}. In the Discord Developer Portal, add yourself as a tester of your app.", &[("why", m)]),
            Fail::Other(m) => m.clone(),
        }
    }
}

fn set_fail(f: &Fail) {
    with_state(|s| {
        s.status = f.status().into();
        s.message = f.say();
    });
    publish();
}

/// An RPC error frame (evt "ERROR") as a Fail: 4006 is "not authenticated",
/// 4002/5000-ish "OAuth2 error" or scope refusals are a refusal.
pub fn error_of(frame: &Value) -> Option<Fail> {
    if frame["evt"] != "ERROR" {
        return None;
    }
    let code = frame["data"]["code"].as_i64().unwrap_or(0);
    let msg = frame["data"]["message"].as_str().unwrap_or("error").to_string();
    Some(match code {
        4006 | 4009 => Fail::NeedsAuth,
        4002..=4005 | 4010 | 5000 => Fail::Refused(msg),
        _ => Fail::Other(tf("Discord: {why}", &[("why", &msg)])),
    })
}

// ── The voice channel from RPC data ───────────────────────────────────────────

fn member_of(v: &Value, me: &str) -> Option<Member> {
    let user = &v["user"];
    let id = user["id"].as_str()?.to_string();
    let name = v["nick"].as_str().filter(|s| !s.is_empty())
        .or_else(|| user["global_name"].as_str().filter(|s| !s.is_empty()))
        .or_else(|| user["username"].as_str())
        .unwrap_or("?")
        .to_string();
    let vs = &v["voice_state"];
    Some(Member {
        me: !me.is_empty() && id == me,
        id,
        name,
        speaking: false,
        muted: vs["self_mute"].as_bool().unwrap_or(false) || vs["mute"].as_bool().unwrap_or(false) || v["mute"].as_bool().unwrap_or(false),
        deafened: vs["self_deaf"].as_bool().unwrap_or(false) || vs["deaf"].as_bool().unwrap_or(false),
    })
}

/// GET_SELECTED_VOICE_CHANNEL's answer (null when not in a call).
pub fn apply_channel(s: &mut State, data: &Value, me: &str) {
    if data.is_null() || data["id"].is_null() {
        s.channel = None;
        s.members.clear();
        return;
    }
    s.channel = Some(Channel {
        id: data["id"].as_str().unwrap_or_default().to_string(),
        name: data["name"].as_str().unwrap_or_default().to_string(),
        guild_id: data["guild_id"].as_str().map(str::to_string),
    });
    s.members = data["voice_states"].as_array().map(|l| l.iter().filter_map(|v| member_of(v, me)).collect()).unwrap_or_default();
}

/// What an event changes. Returns true when the channel must be read again
/// (and its events subscribed): the user moved, joined or left.
pub fn apply_event(s: &mut State, evt: &str, data: &Value, me: &str) -> bool {
    match evt {
        "VOICE_SETTINGS_UPDATE" => {
            if let Some(m) = data["mute"].as_bool() {
                s.mute = m;
            }
            if let Some(d) = data["deaf"].as_bool() {
                s.deaf = d;
            }
        }
        "VOICE_CHANNEL_SELECT" => {
            let now = data["channel_id"].as_str();
            if now != s.channel.as_ref().map(|c| c.id.as_str()) {
                if now.is_none() {
                    s.channel = None;
                    s.members.clear();
                }
                return now.is_some();
            }
        }
        "SPEAKING_START" | "SPEAKING_STOP" => {
            let id = data["user_id"].as_str().unwrap_or_default();
            if let Some(m) = s.members.iter_mut().find(|m| m.id == id) {
                m.speaking = evt == "SPEAKING_START";
            }
        }
        "VOICE_STATE_CREATE" | "VOICE_STATE_UPDATE" => {
            if let Some(mut m) = member_of(data, me) {
                if let Some(old) = s.members.iter_mut().find(|x| x.id == m.id) {
                    m.speaking = old.speaking;
                    *old = m;
                } else {
                    s.members.push(m);
                }
            }
        }
        "VOICE_STATE_DELETE" => {
            let id = data["user"]["id"].as_str().unwrap_or_default();
            s.members.retain(|m| m.id != id);
        }
        _ => {}
    }
    false
}

/// "Sam, Alex and you are in #general." for "who's in the call?".
pub fn who_sentence(s: &State) -> String {
    let Some(ch) = &s.channel else { return t("You're not in a Discord call.") };
    let names: Vec<String> = s.members.iter().map(|m| if m.me { t("You") } else { m.name.clone() }).collect();
    if names.is_empty() {
        return tf("Nobody else is in {channel}.", &[("channel", &ch.name)]);
    }
    let list = names.join(", ");
    let speaking: Vec<&str> = s.members.iter().filter(|m| m.speaking && !m.me).map(|m| m.name.as_str()).collect();
    let mut out = tf("In {channel}: {names}.", &[("channel", &ch.name), ("names", &list)]);
    if !speaking.is_empty() {
        out.push(' ');
        out.push_str(&tf("Speaking: {names}.", &[("names", &speaking.join(", "))]));
    }
    out
}

// ── The session (generic over the pipe, so tests use a fake one) ──────────────

struct Link<W> {
    w: W,
    frames: mpsc::Receiver<std::io::Result<(u32, Value)>>,
    nonce: u64,
    me: String,
}

impl<W: AsyncWrite + Unpin> Link<W> {
    async fn next(&mut self, wait: Duration) -> Result<(u32, Value), Fail> {
        match tokio::time::timeout(wait, self.frames.recv()).await {
            Err(_) => Err(Fail::Other(t("Discord didn't answer."))),
            Ok(None) | Ok(Some(Err(_))) => Err(Fail::NotRunning),
            Ok(Some(Ok(f))) => Ok(f),
        }
    }

    /// Handles a frame that isn't the answer being waited for: events, pings, close.
    async fn other(&mut self, op: u32, v: Value) -> Result<bool, Fail> {
        match op {
            OP_PING => {
                let _ = write_frame(&mut self.w, OP_PONG, &v).await;
                Ok(false)
            }
            OP_CLOSE => Err(close_fail(&v)),
            _ => {
                if v["cmd"] == "DISPATCH" {
                    let evt = v["evt"].as_str().unwrap_or_default().to_string();
                    let me = self.me.clone();
                    let again = with_state(|s| apply_event(s, &evt, &v["data"], &me));
                    publish();
                    return Ok(again);
                }
                Ok(false)
            }
        }
    }

    /// Sends a command and waits for its answer; events meanwhile are applied.
    async fn call(&mut self, cmd: &str, args: Value, evt: Option<&str>, wait: Duration) -> Result<Value, Fail> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        let mut frame = json!({ "cmd": cmd, "args": args, "nonce": nonce });
        if let Some(e) = evt {
            frame["evt"] = json!(e);
        }
        write_frame(&mut self.w, OP_FRAME, &frame).await.map_err(|_| Fail::NotRunning)?;
        let started = Instant::now();
        loop {
            let left = wait.saturating_sub(started.elapsed());
            let (op, v) = self.next(left).await?;
            if op == OP_FRAME && v["nonce"] == nonce.as_str() {
                if let Some(f) = error_of(&v) {
                    return Err(f);
                }
                return Ok(v["data"].clone());
            }
            self.other(op, v).await?;
        }
    }

    async fn read_channel(&mut self) -> Result<(), Fail> {
        let data = self.call("GET_SELECTED_VOICE_CHANNEL", json!({}), None, CALL_TIMEOUT).await?;
        let me = self.me.clone();
        with_state(|s| apply_channel(s, &data, &me));
        if let Some(id) = data["id"].as_str() {
            for evt in ["SPEAKING_START", "SPEAKING_STOP", "VOICE_STATE_CREATE", "VOICE_STATE_UPDATE", "VOICE_STATE_DELETE"] {
                // A refused subscription (an older Discord) leaves the rest working.
                let _ = self.call("SUBSCRIBE", json!({ "channel_id": id }), Some(evt), CALL_TIMEOUT).await;
            }
        }
        publish();
        Ok(())
    }
}

fn close_fail(v: &Value) -> Fail {
    let msg = v["message"].as_str().unwrap_or("closed").to_string();
    match v["code"].as_i64().unwrap_or(0) {
        4000 => Fail::Refused(msg),
        _ => Fail::Other(tf("Discord: {why}", &[("why", &msg)])),
    }
}

/// Starts the reader half: frames go into a channel so waiting can time out.
fn spawn_reader<R: AsyncRead + Unpin + Send + 'static>(mut r: R) -> mpsc::Receiver<std::io::Result<(u32, Value)>> {
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        loop {
            let f = read_frame(&mut r).await;
            let stop = f.is_err();
            if tx.send(f).await.is_err() || stop {
                break;
            }
        }
    });
    rx
}

async fn handshake<W: AsyncWrite + Unpin>(link: &mut Link<W>, client_id: &str) -> Result<(), Fail> {
    write_frame(&mut link.w, OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id })).await.map_err(|_| Fail::NotRunning)?;
    loop {
        let (op, v) = link.next(CALL_TIMEOUT).await?;
        match op {
            OP_CLOSE => return Err(close_fail(&v)),
            OP_FRAME if v["evt"] == "READY" => return Ok(()),
            OP_FRAME if v["evt"] == "ERROR" => return Err(error_of(&v).unwrap_or(Fail::Other("Discord".into()))),
            _ => {}
        }
    }
}

/// One request into the session.
pub struct Req {
    cmd: String,
    args: Value,
    reply: oneshot::Sender<Result<Value, Fail>>,
}

/// The whole session: handshake, AUTHENTICATE, the first state, then
/// requests and events until the lease runs out or the pipe closes.
async fn session<S>(stream: S, client_id: String, access: String, mut rx: mpsc::Receiver<Req>, lease: std::sync::Arc<Mutex<Instant>>) -> Result<(), Fail>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (r, w) = tokio::io::split(stream);
    let mut link = Link { w, frames: spawn_reader(r), nonce: 0, me: String::new() };
    let setup = async {
        handshake(&mut link, &client_id).await?;
        let auth = link.call("AUTHENTICATE", json!({ "access_token": access }), None, CALL_TIMEOUT).await?;
        link.me = auth["user"]["id"].as_str().unwrap_or_default().to_string();
        let vs = link.call("GET_VOICE_SETTINGS", json!({}), None, CALL_TIMEOUT).await?;
        with_state(|s| {
            s.status = "ready".into();
            s.message.clear();
            s.mute = vs["mute"].as_bool().unwrap_or(false);
            s.deaf = vs["deaf"].as_bool().unwrap_or(false);
        });
        let _ = link.call("SUBSCRIBE", json!({}), Some("VOICE_SETTINGS_UPDATE"), CALL_TIMEOUT).await;
        let _ = link.call("SUBSCRIBE", json!({}), Some("VOICE_CHANNEL_SELECT"), CALL_TIMEOUT).await;
        link.read_channel().await
    };
    if let Err(f) = setup.await {
        rx.close();
        while let Some(req) = rx.recv().await {
            let _ = req.reply.send(Err(f.clone()));
        }
        return Err(f);
    }
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            req = rx.recv() => {
                let Some(req) = req else { break };
                let answer = link.call(&req.cmd, req.args, None, CALL_TIMEOUT).await;
                let lost = matches!(answer, Err(Fail::NotRunning));
                let moved = req.cmd == "SELECT_VOICE_CHANNEL";
                let _ = req.reply.send(answer);
                if lost {
                    return Err(Fail::NotRunning);
                }
                if moved {
                    link.read_channel().await?;
                }
            }
            f = link.frames.recv() => {
                let Some(Ok((op, v))) = f else { return Err(Fail::NotRunning) };
                if link.other(op, v).await? {
                    link.read_channel().await?;
                }
            }
            _ = tick.tick() => {
                if Instant::now() > *lease.lock().unwrap() {
                    break;
                }
            }
        }
    }
    let _ = write_frame(&mut link.w, OP_CLOSE, &json!({})).await;
    Ok(())
}

// ── The one connection, while wanted ──────────────────────────────────────────

struct Hub {
    tx: mpsc::Sender<Req>,
    lease: std::sync::Arc<Mutex<Instant>>,
    generation: u64,
}

static HUB: Mutex<Option<Hub>> = Mutex::new(None);
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

trait Pipe: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Pipe for T {}

#[cfg(windows)]
async fn open_pipe() -> Result<Box<dyn Pipe>, Fail> {
    use tokio::net::windows::named_pipe::ClientOptions;
    for i in 0..10 {
        if let Ok(p) = ClientOptions::new().open(format!(r"\\?\pipe\discord-ipc-{i}")) {
            return Ok(Box::new(p));
        }
    }
    Err(Fail::NotRunning)
}

#[cfg(unix)]
async fn open_pipe() -> Result<Box<dyn Pipe>, Fail> {
    let mut dirs: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(std::env::var_os).map(Into::into).collect();
    dirs.push("/tmp".into());
    for dir in dirs {
        for i in 0..10 {
            if let Ok(s) = tokio::net::UnixStream::connect(dir.join(format!("discord-ipc-{i}"))).await {
                return Ok(Box::new(s));
            }
        }
    }
    Err(Fail::NotRunning)
}

pub fn client_id() -> Option<String> {
    crate::db::conn().ok().and_then(|c| crate::music::kv_get(&c, KV_CLIENT_ID)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tokens {
    access: String,
    refresh: Option<String>,
    expires_at: i64,
}

fn tokens() -> Option<Tokens> {
    crate::secrets::get_internal(TOKENS_KEY).and_then(|s| serde_json::from_str(&s).ok())
}

fn save_tokens(tk: &Tokens) -> Result<(), String> {
    crate::secrets::set_internal(TOKENS_KEY, &serde_json::to_string(tk).map_err(|e| e.to_string())?)
}

async fn fresh_access(client_id: &str) -> Result<String, Fail> {
    let tk = tokens().ok_or(Fail::NeedsAuth)?;
    if tk.expires_at > crate::db::now() + 60 {
        return Ok(tk.access);
    }
    let refresh = tk.refresh.clone().ok_or(Fail::NeedsAuth)?;
    let new = exchange(client_id, &[("grant_type", "refresh_token"), ("refresh_token", &refresh)]).await.map_err(|_| Fail::NeedsAuth)?;
    let _ = save_tokens(&new);
    Ok(new.access)
}

/// The running session, or a new one, with its lease pushed to `lease` from now.
async fn ensure(lease: Duration) -> Result<mpsc::Sender<Req>, Fail> {
    let until = Instant::now() + lease;
    if let Some(h) = HUB.lock().unwrap().as_ref() {
        if !h.tx.is_closed() {
            let mut l = h.lease.lock().unwrap();
            *l = (*l).max(until);
            return Ok(h.tx.clone());
        }
    }
    let client_id = client_id().ok_or(Fail::NotSetUp)?;
    let access = fresh_access(&client_id).await?;
    let pipe = open_pipe().await?;
    let (tx, rx) = mpsc::channel(8);
    let shared = std::sync::Arc::new(Mutex::new(until));
    let generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    *HUB.lock().unwrap() = Some(Hub { tx: tx.clone(), lease: shared.clone(), generation });
    tauri::async_runtime::spawn(async move {
        let end = session(pipe, client_id, access, rx, shared).await;
        {
            let mut hub = HUB.lock().unwrap();
            if hub.as_ref().is_some_and(|h| h.generation == generation) {
                *hub = None;
            }
        }
        match end {
            Ok(()) => {
                with_state(|s| {
                    s.status = "off".into();
                    s.members.iter_mut().for_each(|m| m.speaking = false);
                });
                publish();
            }
            Err(f) => {
                if f == Fail::NeedsAuth {
                    // The stored token no longer works: the next Connect asks again.
                    let _ = crate::secrets::set_internal(TOKENS_KEY, "");
                }
                set_fail(&f);
            }
        }
    });
    Ok(tx)
}

/// Runs one RPC command through the (lazily opened) connection.
pub async fn command(cmd: &str, args: Value) -> Result<Value, Fail> {
    let tx = ensure(Duration::from_secs(5)).await.inspect_err(set_fail)?;
    let (reply, wait) = oneshot::channel();
    tx.send(Req { cmd: cmd.into(), args, reply }).await.map_err(|_| Fail::NotRunning)?;
    let out = tokio::time::timeout(CALL_TIMEOUT * 4, wait).await.map_err(|_| Fail::Other(t("Discord didn't answer.")))?.map_err(|_| Fail::NotRunning)?;
    out.inspect_err(set_fail)
}

/// What a voice or chat command asks of Discord.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Voice {
    /// None: toggle.
    Mute(Option<bool>),
    Deaf(Option<bool>),
    Who,
    Leave,
}

/// Does it over RPC; the sentence Groovy says back.
pub async fn act(v: Voice) -> Result<String, Fail> {
    match v {
        Voice::Mute(want) | Voice::Deaf(want) => {
            let mute = matches!(v, Voice::Mute(_));
            let on = match want {
                Some(b) => b,
                None => {
                    let cur = command("GET_VOICE_SETTINGS", json!({})).await?;
                    !cur[if mute { "mute" } else { "deaf" }].as_bool().unwrap_or(false)
                }
            };
            let mut args = serde_json::Map::new();
            args.insert(if mute { "mute" } else { "deaf" }.into(), json!(on));
            let data = command("SET_VOICE_SETTINGS", Value::Object(args)).await?;
            with_state(|s| {
                s.mute = data["mute"].as_bool().unwrap_or(s.mute);
                s.deaf = data["deaf"].as_bool().unwrap_or(s.deaf);
            });
            publish();
            Ok(match (mute, on) {
                (true, true) => t("Discord: muted."),
                (true, false) => t("Discord: unmuted."),
                (false, true) => t("Discord: deafened."),
                (false, false) => t("Discord: undeafened."),
            })
        }
        Voice::Who => {
            let data = command("GET_SELECTED_VOICE_CHANNEL", json!({})).await?;
            Ok(with_state(|s| {
                let speaking: Vec<String> = s.members.iter().filter(|m| m.speaking).map(|m| m.id.clone()).collect();
                let me = s.members.iter().find(|m| m.me).map(|m| m.id.clone()).unwrap_or_default();
                apply_channel(s, &data, &me);
                s.members.iter_mut().for_each(|m| m.speaking = speaking.contains(&m.id));
                who_sentence(s)
            }))
        }
        Voice::Leave => {
            if snapshot().status == "ready" && snapshot().channel.is_none() {
                return Ok(t("You're not in a Discord call."));
            }
            command("SELECT_VOICE_CHANNEL", json!({ "channel_id": Value::Null })).await?;
            with_state(|s| {
                s.channel = None;
                s.members.clear();
            });
            publish();
            Ok(t("Left the Discord call."))
        }
    }
}

// ── Authorising Groovy once ───────────────────────────────────────────────────

fn random_verifier() -> Result<String, String> {
    use base64::Engine;
    use ring::rand::SecureRandom;
    let mut b = [0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "no randomness".to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&")
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The token endpoint: with the owner's secret when saved, else PKCE alone.
async fn exchange(client_id: &str, extra: &[(&str, &str)]) -> Result<Tokens, String> {
    let secret = crate::secrets::get_internal(SECRET_KEY).filter(|s| !s.is_empty());
    let url = reqwest::Url::parse(TOKEN_URL).map_err(|e| e.to_string())?;
    let mut last = String::new();
    // Discord may want the redirect the app registered; the setup steps register http://127.0.0.1.
    for redirect in [None, Some("http://127.0.0.1")] {
        let mut pairs: Vec<(&str, &str)> = vec![("client_id", client_id)];
        if let Some(s) = secret.as_deref() {
            pairs.push(("client_secret", s));
        }
        if let Some(r) = redirect {
            pairs.push(("redirect_uri", r));
        }
        pairs.extend_from_slice(extra);
        let response = crate::net::client(&url, Duration::from_secs(30))?
            .post(url.clone())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form(&pairs))
            .send()
            .await
            .map_err(|e| tf("Network error: {error}", &[("error", &e.to_string())]))?;
        let ok = response.status().is_success();
        let body = crate::net::read_capped(response, 64 * 1024).await?;
        if ok {
            #[derive(Deserialize)]
            struct Answer {
                access_token: String,
                refresh_token: Option<String>,
                expires_in: Option<i64>,
            }
            let a: Answer = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
            return Ok(Tokens { access: a.access_token, refresh: a.refresh_token, expires_at: crate::db::now() + a.expires_in.unwrap_or(604_800) - 60 });
        }
        last = crate::net::error_detail(&body);
    }
    crate::log::line(format!("discord: token exchange refused ({last})"));
    Err(if secret.is_none() {
        t("Discord wants your app's client secret: paste it in Settings → Discord, then Connect again.")
    } else {
        t("Discord refused the sign-in. Check the client ID and secret, then Connect again.")
    })
}

/// Opens Discord's own "Authorise Groovy?" window, then keeps the tokens.
async fn authorise() -> Result<(), String> {
    let client_id = client_id().ok_or_else(|| Fail::NotSetUp.say())?;
    // A running session holds the voice settings; start again after.
    *HUB.lock().unwrap() = None;
    let pipe = open_pipe().await.map_err(|f| f.say())?;
    let (r, w) = tokio::io::split(pipe);
    let mut link = Link { w, frames: spawn_reader(r), nonce: 0, me: String::new() };
    handshake(&mut link, &client_id).await.map_err(|f| f.say())?;
    let secret = crate::secrets::get_internal(SECRET_KEY).filter(|s| !s.is_empty());
    let verifier = random_verifier()?;
    let mut args = json!({ "client_id": client_id, "scopes": SCOPES });
    if secret.is_none() {
        // Not documented for RPC; Discord's OAuth takes a PKCE code exchange with no secret.
        args["code_challenge"] = json!(crate::mail::oauth::challenge(&verifier));
        args["code_challenge_method"] = json!("S256");
    }
    let data = link.call("AUTHORIZE", args, None, AUTHORISE_TIMEOUT).await.map_err(|f| f.say())?;
    let _ = write_frame(&mut link.w, OP_CLOSE, &json!({})).await;
    let code = data["code"].as_str().ok_or_else(|| t("Discord didn't approve Groovy."))?.to_string();
    let mut extra = vec![("grant_type", "authorization_code"), ("code", code.as_str())];
    if secret.is_none() {
        extra.push(("code_verifier", verifier.as_str()));
    }
    let tk = exchange(&client_id, &extra).await?;
    save_tokens(&tk)
}

// ── Messages: Discord's Windows notifications (round 6 item 4) ───────────────

/// A new Discord notification (island_tiles.rs's watch): the newest five, in memory only.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn note(id: u32, title: &str, body: &str) {
    let n = Note { id, title: title.chars().take(120).collect(), body: body.chars().take(400).collect() };
    with_state(|s| {
        s.notes.retain(|x| x.id != id);
        s.notes.insert(0, n);
        s.notes.truncate(5);
    });
    publish();
}

/// What to type in Discord's quick switcher (Ctrl+K) to reach a notification's
/// conversation: "Sam" for a DM, "#general" for "Sam (#general, Server)", the
/// group's name for "Sam (Weekend plans)".
pub fn switcher_query(title: &str) -> String {
    let title = title.trim();
    if let Some(open) = title.rfind(" (") {
        if let Some(inner) = title[open + 2..].strip_suffix(')') {
            let first = inner.split(", ").next().unwrap_or(inner).trim();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    title.to_string()
}

/// The latest notification's conversation, for "reply" and "message this person".
pub fn latest_query(id: Option<u32>) -> Option<String> {
    with_state(|s| s.notes.iter().find(|n| id.is_none_or(|i| n.id == i)).map(|n| switcher_query(&n.title))).filter(|q| !q.is_empty())
}

/// The tile's Reply: opens the conversation in Discord (the user types and presses Enter).
#[tauri::command]
pub async fn discord_reply(id: u32) -> Result<String, String> {
    let to = latest_query(Some(id)).ok_or_else(|| t("That Discord message is gone."))?;
    tauri::async_runtime::spawn_blocking(move || crate::pc::discord_message(Some(to), String::new())).await.map_err(|e| e.to_string())?
}

// ── Commands for the island and Settings ──────────────────────────────────────

fn with_setup(mut s: State) -> State {
    s.set_up = client_id().is_some();
    s.authorised = tokens().is_some();
    if !s.set_up {
        s.status = "not-set-up".into();
        s.message = Fail::NotSetUp.say();
    } else if !s.authorised && s.status != "not-running" {
        s.status = "needs-auth".into();
        s.message = Fail::NeedsAuth.say();
    }
    s
}

/// What the tile shows, without connecting.
#[tauri::command]
pub async fn discord_state() -> State {
    tauri::async_runtime::spawn_blocking(|| with_setup(snapshot())).await.unwrap_or_default()
}

/// The island shows the Discord tile: connect (or keep the connection) for
/// another LEASE. With nothing set up nothing is opened.
#[tauri::command]
pub async fn discord_watch() -> State {
    let s = discord_state().await;
    if s.set_up && s.authorised {
        if let Err(f) = ensure(LEASE).await {
            set_fail(&f);
        }
    }
    with_setup(snapshot())
}

/// Settings → Discord: the app's client ID and, optionally, its secret.
#[tauri::command]
pub fn discord_configure(client_id: String, client_secret: Option<String>) -> Result<(), String> {
    let id = client_id.trim();
    if !id.is_empty() && (id.len() > 32 || !id.bytes().all(|b| b.is_ascii_digit())) {
        return Err(t("A Discord client ID is a long number (Developer Portal → OAuth2)."));
    }
    let conn = crate::db::conn()?;
    if crate::music::kv_get(&conn, KV_CLIENT_ID).unwrap_or_default() != id {
        // Another app: its tokens are not this one's.
        let _ = crate::secrets::set_internal(TOKENS_KEY, "");
    }
    crate::music::kv_set(&conn, KV_CLIENT_ID, id);
    if let Some(s) = client_secret.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        crate::secrets::set_internal(SECRET_KEY, &s)?;
    }
    Ok(())
}

/// Whether a client secret is saved (it is never sent back).
#[tauri::command]
pub fn discord_has_secret() -> bool {
    crate::secrets::get_internal(SECRET_KEY).is_some_and(|s| !s.is_empty())
}

#[tauri::command]
pub async fn discord_authorise() -> Result<State, String> {
    authorise().await?;
    with_state(|s| {
        s.status = "off".into();
        s.message.clear();
    });
    Ok(discord_watch().await)
}

/// Forget the tokens and the secret (Settings → Discord → Disconnect).
#[tauri::command]
pub fn discord_forget() -> Result<(), String> {
    *HUB.lock().unwrap() = None;
    crate::secrets::set_internal(TOKENS_KEY, "")?;
    crate::secrets::set_internal(SECRET_KEY, "")?;
    with_state(|s| *s = State { notes: std::mem::take(&mut s.notes), ..State::default() });
    publish();
    Ok(())
}

/// The tile's buttons: "mute", "unmute", "deafen", "undeafen", "leave".
#[tauri::command]
pub async fn discord_voice(action: String) -> Result<String, String> {
    let v = match action.as_str() {
        "mute" => Voice::Mute(Some(true)),
        "unmute" => Voice::Mute(Some(false)),
        "deafen" => Voice::Deaf(Some(true)),
        "undeafen" => Voice::Deaf(Some(false)),
        "leave" => Voice::Leave,
        _ => return Err("unknown action".into()),
    };
    act(v).await.map_err(|f| f.say())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    /// Discord's side of the pipe: answers each command from a script.
    async fn fake_discord<S: AsyncRead + AsyncWrite + Unpin>(mut s: S, seen: std::sync::Arc<Mutex<Vec<String>>>) {
        let (op, hello) = read_frame(&mut s).await.unwrap();
        assert_eq!(op, OP_HANDSHAKE);
        assert_eq!(hello["v"], 1);
        write_frame(&mut s, OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1 } })).await.unwrap();
        while let Ok((op, f)) = read_frame(&mut s).await {
            if op == OP_CLOSE {
                break;
            }
            let cmd = f["cmd"].as_str().unwrap_or_default().to_string();
            seen.lock().unwrap().push(format!("{cmd}{}", f["evt"].as_str().map(|e| format!(":{e}")).unwrap_or_default()));
            let nonce = f["nonce"].clone();
            let data = match cmd.as_str() {
                "AUTHENTICATE" if f["args"]["access_token"] != "good" => {
                    write_frame(&mut s, OP_FRAME, &json!({ "cmd": cmd, "evt": "ERROR", "nonce": nonce, "data": { "code": 4006, "message": "Not authenticated" } })).await.unwrap();
                    continue;
                }
                "AUTHENTICATE" => json!({ "user": { "id": "1" } }),
                "GET_VOICE_SETTINGS" => json!({ "mute": false, "deaf": false }),
                "GET_SELECTED_VOICE_CHANNEL" => json!({ "id": "c1", "name": "General", "voice_states": [
                    { "user": { "id": "1", "username": "me" }, "voice_state": {} },
                    { "nick": "Sam", "user": { "id": "2", "username": "sam_x" }, "voice_state": { "self_mute": true } }
                ] }),
                "SET_VOICE_SETTINGS" => {
                    // An event first, as Discord does, then the answer.
                    write_frame(&mut s, OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "SPEAKING_START", "data": { "user_id": "2" } })).await.unwrap();
                    json!({ "mute": f["args"]["mute"].as_bool().unwrap_or(false), "deaf": false })
                }
                _ => json!({}),
            };
            write_frame(&mut s, OP_FRAME, &json!({ "cmd": cmd, "nonce": nonce, "data": data })).await.unwrap();
        }
    }

    #[tokio::test]
    async fn frames_round_trip() {
        let (mut a, mut b) = duplex(4096);
        write_frame(&mut a, OP_FRAME, &json!({ "cmd": "X", "n": "é" })).await.unwrap();
        let (op, v) = read_frame(&mut b).await.unwrap();
        assert_eq!((op, v["cmd"].as_str(), v["n"].as_str()), (OP_FRAME, Some("X"), Some("é")));
        // The header is little-endian opcode then length.
        write_frame(&mut a, OP_PING, &json!({})).await.unwrap();
        let mut raw = [0u8; 10];
        b.read_exact(&mut raw).await.unwrap();
        assert_eq!(&raw[..8], &[3, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[tokio::test]
    async fn oversized_frames_are_refused() {
        let (mut a, mut b) = duplex(64);
        a.write_all(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]).await.unwrap();
        assert!(read_frame(&mut b).await.is_err());
    }

    #[tokio::test]
    async fn a_session_over_a_fake_pipe() {
        let (ours, theirs) = duplex(1 << 16);
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        tokio::spawn(fake_discord(theirs, seen.clone()));
        let (tx, rx) = mpsc::channel(4);
        let lease = std::sync::Arc::new(Mutex::new(Instant::now() + Duration::from_secs(30)));
        let run = tokio::spawn(session(ours, "123".into(), "good".into(), rx, lease.clone()));
        let (reply, wait) = oneshot::channel();
        tx.send(Req { cmd: "SET_VOICE_SETTINGS".into(), args: json!({ "mute": true }), reply }).await.unwrap();
        let data = wait.await.unwrap().unwrap();
        assert_eq!(data["mute"], true);
        let s = snapshot();
        assert_eq!(s.status, "ready");
        assert_eq!(s.channel.as_ref().map(|c| c.name.as_str()), Some("General"));
        assert!(s.members.iter().any(|m| m.name == "Sam" && m.muted && m.speaking), "{:?}", s.members);
        assert!(s.members.iter().any(|m| m.me));
        // The lease ends: the session closes the pipe on its own.
        *lease.lock().unwrap() = Instant::now();
        assert!(tokio::time::timeout(Duration::from_secs(5), run).await.unwrap().unwrap().is_ok());
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen[0], "AUTHENTICATE");
        assert!(seen.contains(&"SUBSCRIBE:SPEAKING_START".to_string()), "{seen:?}");
        assert_eq!(seen.last().map(String::as_str), Some("SET_VOICE_SETTINGS"));
    }

    #[tokio::test]
    async fn a_bad_token_fails_every_waiting_request() {
        let (ours, theirs) = duplex(1 << 16);
        tokio::spawn(fake_discord(theirs, Default::default()));
        let (tx, rx) = mpsc::channel(4);
        let (reply, wait) = oneshot::channel();
        tx.send(Req { cmd: "GET_VOICE_SETTINGS".into(), args: json!({}), reply }).await.unwrap();
        let lease = std::sync::Arc::new(Mutex::new(Instant::now() + Duration::from_secs(30)));
        let end = session(ours, "123".into(), "stale".into(), rx, lease).await;
        assert_eq!(end, Err(Fail::NeedsAuth));
        assert_eq!(wait.await.unwrap(), Err(Fail::NeedsAuth));
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(error_of(&json!({ "evt": "ERROR", "data": { "code": 4006, "message": "x" } })), Some(Fail::NeedsAuth));
        assert!(matches!(error_of(&json!({ "evt": "ERROR", "data": { "code": 5000, "message": "OAuth2 Error" } })), Some(Fail::Refused(_))));
        assert_eq!(error_of(&json!({ "evt": null, "data": {} })), None);
        assert!(!Fail::NotRunning.say().is_empty());
    }

    #[test]
    fn events_change_the_state() {
        let mut s = State::default();
        apply_channel(&mut s, &json!({ "id": "c", "name": "Raid", "voice_states": [{ "user": { "id": "9", "global_name": "Alex" }, "voice_state": {} }] }), "1");
        assert!(!apply_event(&mut s, "SPEAKING_START", &json!({ "user_id": "9" }), "1"));
        assert!(s.members[0].speaking);
        apply_event(&mut s, "VOICE_STATE_CREATE", &json!({ "user": { "id": "1", "username": "me" }, "voice_state": { "self_deaf": true } }), "1");
        assert!(s.members.iter().any(|m| m.me && m.deafened));
        apply_event(&mut s, "VOICE_SETTINGS_UPDATE", &json!({ "mute": true, "deaf": true }), "1");
        assert!(s.mute && s.deaf);
        assert!(who_sentence(&s).contains("Alex"));
        apply_event(&mut s, "VOICE_STATE_DELETE", &json!({ "user": { "id": "9" } }), "1");
        assert_eq!(s.members.len(), 1);
        assert!(apply_event(&mut s, "VOICE_CHANNEL_SELECT", &json!({ "channel_id": "other" }), "1"), "moved: read again");
        assert!(!apply_event(&mut s, "VOICE_CHANNEL_SELECT", &json!({ "channel_id": null }), "1"));
        assert!(s.channel.is_none() && s.members.is_empty());
        assert_eq!(who_sentence(&s), t("You're not in a Discord call."));
    }

    #[test]
    fn the_quick_switcher_finds_the_conversation() {
        assert_eq!(switcher_query("Sam"), "Sam");
        assert_eq!(switcher_query("Sam (#general, Groovy fans)"), "#general");
        assert_eq!(switcher_query("Alex (Weekend plans)"), "Weekend plans");
        assert_eq!(switcher_query("  Jo  "), "Jo");
        assert_eq!(switcher_query("Odd ()"), "Odd ()");
    }

    #[test]
    fn the_pkce_challenge_is_s256() {
        let v = random_verifier().unwrap();
        assert!(v.len() >= 43);
        assert_eq!(crate::mail::oauth::challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(form(&[("a", "b c"), ("d", "é")]), "a=b%20c&d=%C3%A9");
    }
}
