// Groovy's three operating states (round 3D item 10), one state machine:
//
// - Standby (the default): the island, the wake-word spotter when « OK
//   Groovy » is on, the tiles' slow updates. Nothing else is loaded.
// - Conversation: the wake word, the dictation hotkey, the Talk button or a
//   live call. The cloud speech engine, the brain and Fish Audio are warmed
//   at once and in parallel; about 8 s after the last reply with nothing said
//   (or at once on "thanks" / "stop") everything it held is released.
// - Offline: no network, every cloud brain out of its limit, or the user
//   chose a brain on this PC. The offline engines load per request only.
//
// The island shows it subtly and Settings → System in full (`runtime_state`).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// After the last reply, this long with nothing said and the conversation ends.
pub const FOLLOW_UP: Duration = Duration::from_secs(8);
/// While offline, the cloud is looked for again this often.
pub const PROBE_EVERY: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    Standby,
    Conversation,
    Offline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The wake word, the dictation hotkey, the Talk button.
    Wake,
    CallStarted,
    CallEnded,
    /// Groovy finished answering (or the dictation was typed).
    ReplyDone,
    /// FOLLOW_UP passed since the reply with nothing said.
    Quiet,
    /// "Thanks" or "stop".
    Dismissed,
    NetworkDown,
    /// Every cloud brain is out of its limit.
    CloudExhausted,
    /// The cloud answered again.
    NetworkUp,
    /// The user chose a brain on this PC (true) or a cloud one again (false).
    ChoseLocal(bool),
}

/// Why Groovy is offline: any one keeps it there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OfflineWhy {
    pub network: bool,
    pub limits: bool,
    pub chose_local: bool,
}

impl OfflineWhy {
    pub fn any(&self) -> bool {
        self.network || self.limits || self.chose_local
    }
}

/// What a transition asks for besides the new state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Open the cloud speech, brain and voice connections now.
    pub prewarm: bool,
    /// Release what the conversation held.
    pub release: bool,
    /// Start the countdown to Quiet.
    pub count_down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    pub state: RunState,
    pub offline: OfflineWhy,
    /// A call is on: it never ends by silence.
    pub in_call: bool,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { state: RunState::Standby, offline: OfflineWhy::default(), in_call: false }
    }
}

impl Machine {
    /// The resting state: Offline when any reason holds, else Standby.
    fn resting(&self) -> RunState {
        if self.offline.any() {
            RunState::Offline
        } else {
            RunState::Standby
        }
    }

    /// One event: the new state and what to do about it.
    pub fn apply(&mut self, e: Event) -> Effects {
        let mut fx = Effects::default();
        match e {
            Event::Wake | Event::CallStarted => {
                if e == Event::CallStarted {
                    self.in_call = true;
                }
                // Offline, the offline engines load per request: nothing to warm.
                if self.offline.any() {
                    self.state = RunState::Offline;
                } else {
                    fx.prewarm = self.state != RunState::Conversation;
                    self.state = RunState::Conversation;
                }
            }
            Event::ReplyDone => {
                fx.count_down = self.state == RunState::Conversation && !self.in_call;
            }
            Event::CallEnded => {
                self.in_call = false;
                fx.count_down = self.state == RunState::Conversation;
            }
            Event::Quiet => {
                if self.state == RunState::Conversation && !self.in_call {
                    self.state = self.resting();
                    fx.release = true;
                }
            }
            Event::Dismissed => {
                if self.state == RunState::Conversation {
                    self.in_call = false;
                    self.state = self.resting();
                    fx.release = true;
                }
            }
            Event::NetworkDown => self.offline.network = true,
            Event::CloudExhausted => self.offline.limits = true,
            Event::NetworkUp => {
                self.offline.network = false;
                self.offline.limits = false;
            }
            Event::ChoseLocal(on) => self.offline.chose_local = on,
        }
        // Going offline or coming back changes the resting state, not a conversation.
        if matches!(e, Event::NetworkDown | Event::CloudExhausted | Event::NetworkUp | Event::ChoseLocal(_)) && self.state != RunState::Conversation {
            self.state = self.resting();
        }
        fx
    }
}

/// "Thanks", "thank you", "stop", "that's all": the conversation is over.
pub fn is_dismissal(text: &str) -> bool {
    let t: String = text.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '\'').collect();
    let t = t.trim();
    ["thanks", "thank you", "thank you groovy", "thanks groovy", "stop", "that's all", "thats all", "never mind", "nevermind"].contains(&t)
}

// ── The running machine ───────────────────────────────────────────────────────

struct Live {
    m: Machine,
    since: Instant,
    /// Bumped by every event: a countdown that finds it changed was overtaken.
    epoch: u64,
    probing: bool,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    let mut g = LIVE.lock().unwrap();
    let live = g.get_or_insert_with(|| Live { m: Machine::default(), since: Instant::now(), epoch: 0, probing: false });
    f(live)
}

pub fn state() -> RunState {
    with(|l| l.m.state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub state: RunState,
    pub seconds: u64,
    pub offline_network: bool,
    pub offline_limits: bool,
    pub offline_chose_local: bool,
    pub warm_clients: usize,
    pub webviews: Vec<String>,
}

/// For Settings → System and CI: the state, for how long, and why if offline.
#[tauri::command]
pub fn runtime_state(app: AppHandle) -> Report {
    let webviews = webviews(&app);
    with(|l| Report {
        state: l.m.state,
        seconds: l.since.elapsed().as_secs(),
        offline_network: l.m.offline.network,
        offline_limits: l.m.offline.limits,
        offline_chose_local: l.m.offline.chose_local,
        warm_clients: crate::net::shared_clients(),
        webviews,
    })
}

/// Something happened: move the machine and do what the move asks.
pub fn fire(app: &AppHandle, e: Event) {
    let (before, after, fx, epoch, probe) = with(|l| {
        let before = l.m.state;
        let fx = l.m.apply(e);
        l.epoch += 1;
        if l.m.state != before {
            l.since = Instant::now();
        }
        let probe = l.m.state == RunState::Offline && (l.m.offline.network || l.m.offline.limits) && !l.probing;
        if probe {
            l.probing = true;
        }
        (before, l.m.state, fx, l.epoch, probe)
    });
    if before != after {
        crate::log::line(format!("state: {before:?} → {after:?} ({e:?})"));
        let _ = app.emit("runtime-state", after);
        if after == RunState::Offline {
            crate::flow::live::announce(app, "bolt", "#F5B84B", crate::i18n::t("Offline: thinking on this PC"), None, 6);
        }
    }
    if fx.prewarm {
        prewarm(app);
    }
    if fx.release {
        release(app);
    }
    if fx.count_down {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(FOLLOW_UP).await;
            // Only if nothing else happened meanwhile (a follow-up question, a new wake word).
            if with(|l| l.epoch) == epoch {
                fire(&app, Event::Quiet);
            }
        });
    }
    if probe {
        spawn_probe(app.clone());
    }
}

/// The wake word fired: the cloud speech engine, the brain and Fish Audio are
/// reached now, all at once, so none of them makes the first answer wait.
fn prewarm(app: &AppHandle) {
    let urls = warm_targets(app);
    if urls.is_empty() {
        return;
    }
    let started = Instant::now();
    tauri::async_runtime::spawn(async move {
        let n = urls.len();
        crate::net::warm(urls, Duration::from_secs(90)).await;
        crate::log::line(format!("state: {n} connection(s) warmed in {} ms", started.elapsed().as_millis()));
    });
}

/// The services this conversation will use, only those the user set up.
fn warm_targets(app: &AppHandle) -> Vec<reqwest::Url> {
    let mut out: Vec<reqwest::Url> = Vec::new();
    let mut add = |u: &str| {
        if let Ok(u) = reqwest::Url::parse(u) {
            if !out.iter().any(|x| x.host_str() == u.host_str()) {
                out.push(u);
            }
        }
    };
    if let Some(s) = crate::speech::cloud::available().first() {
        add(s.host());
    }
    if let Some(u) = crate::brain_ai::first_cloud_url(app) {
        add(&u);
    }
    if crate::secrets::present(crate::speech::tts::FISH_KEY) {
        add(crate::speech::tts::FISH_URL);
    }
    out
}

/// Back to Standby: what the conversation held goes.
fn release(app: &AppHandle) {
    crate::net::release();
    // The WebView2 audit: which windows are alive now, and what each is for.
    crate::log::line(format!("state: webviews open: {}", webviews(app).join(", ")));
    // The page drops what it no longer shows (artwork, grown tiles' data).
    let _ = app.emit("runtime-release", ());
}

/// Every WebView window alive, with its purpose: in Standby, only the island.
pub fn webviews(app: &AppHandle) -> Vec<String> {
    use tauri::Manager;
    let mut v: Vec<String> = app
        .webview_windows()
        .keys()
        .map(|label| {
            let what = match label.as_str() {
                "island" => "the island",
                "settings" => "Settings",
                "studio" => "the Coding Studio",
                "mochi" => "Groovy on the desktop",
                _ => "other",
            };
            format!("{label} ({what})")
        })
        .collect();
    v.sort();
    v
}

/// While offline (not by choice), the cloud is looked for every PROBE_EVERY.
fn spawn_probe(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(PROBE_EVERY).await;
            let still = with(|l| l.m.state == RunState::Offline && (l.m.offline.network || l.m.offline.limits));
            if !still {
                break;
            }
            // Only a service the user set up is asked, and only whether it answers.
            let Some(url) = crate::brain_ai::first_cloud_url(&app) else { continue };
            let Ok(u) = reqwest::Url::parse(&url) else { continue };
            let Ok(c) = crate::net::client(&u, Duration::from_secs(5)) else { continue };
            if c.head(u).send().await.is_ok() {
                // Reachable: the limits get another try too.
                fire(&app, Event::NetworkUp);
                break;
            }
        }
        with(|l| l.probing = false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m() -> Machine {
        Machine::default()
    }

    #[test]
    fn standby_to_conversation_warms_once() {
        let mut x = m();
        assert_eq!(x.state, RunState::Standby);
        let fx = x.apply(Event::Wake);
        assert_eq!(x.state, RunState::Conversation);
        assert!(fx.prewarm && !fx.release);
        assert!(!x.apply(Event::Wake).prewarm, "already warm in the conversation");
    }

    #[test]
    fn the_reply_then_quiet_releases() {
        let mut x = m();
        x.apply(Event::Wake);
        let fx = x.apply(Event::ReplyDone);
        assert!(fx.count_down, "the 8 s follow-up window starts");
        assert_eq!(x.state, RunState::Conversation, "a follow-up can still come");
        let fx = x.apply(Event::Quiet);
        assert_eq!(x.state, RunState::Standby);
        assert!(fx.release);
        assert_eq!(x.apply(Event::Quiet), Effects::default(), "quiet in Standby does nothing");
    }

    #[test]
    fn thanks_or_stop_ends_it_at_once() {
        let mut x = m();
        x.apply(Event::Wake);
        assert!(x.apply(Event::Dismissed).release);
        assert_eq!(x.state, RunState::Standby);
        for t in ["Thanks!", "thank you", "Stop.", "That's all"] {
            assert!(is_dismissal(t), "{t}");
        }
        for t in ["stop the music", "thanks for the summary, now send it"] {
            assert!(!is_dismissal(t), "{t}");
        }
    }

    #[test]
    fn a_call_never_ends_by_silence() {
        let mut x = m();
        assert!(x.apply(Event::CallStarted).prewarm);
        assert!(!x.apply(Event::ReplyDone).count_down);
        x.apply(Event::Quiet);
        assert_eq!(x.state, RunState::Conversation);
        assert!(x.apply(Event::CallEnded).count_down);
        x.apply(Event::Quiet);
        assert_eq!(x.state, RunState::Standby);
    }

    #[test]
    fn offline_and_back() {
        let mut x = m();
        x.apply(Event::NetworkDown);
        assert_eq!(x.state, RunState::Offline);
        let fx = x.apply(Event::Wake);
        assert_eq!(x.state, RunState::Offline, "talking offline stays Offline");
        assert!(!fx.prewarm, "no cloud to warm");
        x.apply(Event::NetworkUp);
        assert_eq!(x.state, RunState::Standby);
        x.apply(Event::CloudExhausted);
        assert_eq!(x.state, RunState::Offline);
        x.apply(Event::NetworkUp);
        assert_eq!(x.state, RunState::Standby, "a working cloud tries the limits again");
    }

    #[test]
    fn choosing_local_is_offline_until_unchosen() {
        let mut x = m();
        x.apply(Event::ChoseLocal(true));
        assert_eq!(x.state, RunState::Offline);
        x.apply(Event::NetworkUp);
        assert_eq!(x.state, RunState::Offline, "the network coming back doesn't undo a choice");
        x.apply(Event::ChoseLocal(false));
        assert_eq!(x.state, RunState::Standby);
    }

    #[test]
    fn going_offline_mid_conversation_waits_for_it_to_end() {
        let mut x = m();
        x.apply(Event::Wake);
        x.apply(Event::NetworkDown);
        assert_eq!(x.state, RunState::Conversation);
        x.apply(Event::ReplyDone);
        x.apply(Event::Quiet);
        assert_eq!(x.state, RunState::Offline, "it rests offline");
    }
}
