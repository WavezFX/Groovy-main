// One switcher for chat and the background jobs (round 4D item 2).
//
// Who answers, in which order, and when the next one stands in. Cloud and
// subscription first, this PC last and only when local AI was chosen
// (FIXES_ROUND1 item 2); the user's own order and a job's own brain on top
// (`arrange`). A failed brain hands over to the next only when switching may
// fix it: a limit (and only when "Switch when a limit is hit" is on), the
// network, or a missing sign-in or key. A refusal or a bad request stops.
// Local-only mode builds no cloud request at all.

use serde_json::Value;
use tauri::AppHandle;

use super::{BrainError, Ctx, Fault, Request};
use crate::assistant::AssistantPrefs;
use crate::settings::Settings;

/// What the answer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Chat,
    Writing,
    Commands,
    Vision,
}

/// A quick small job or a big one: what a router plugin sorts by (item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Weight {
    Light,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub kind: Kind,
    pub weight: Weight,
}

impl Job {
    pub const WRITING: Job = Job { kind: Kind::Writing, weight: Weight::Light };
    pub const COMMANDS: Job = Job { kind: Kind::Commands, weight: Weight::Light };

    /// The key "Model per job" (`job_brains`) stores it under.
    pub fn key(&self) -> &'static str {
        match self.kind {
            Kind::Chat => "chat",
            Kind::Writing => "writing",
            Kind::Commands => "commands",
            Kind::Vision => "vision",
        }
    }
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
    super::cli::SUBSCRIPTIONS.into_iter().find(|c| *c == id)
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
        // Sign in first (round 4D item 3): a signed-in account goes before
        // a key for the same brain; the others after the keys.
        for c in super::cli::SUBSCRIPTIONS {
            if has_cli(c) {
                let who = Who::Cli(cli_id(c).unwrap());
                let same = match c {
                    "claude-code" => Who::Claude,
                    "gemini-cli" => Who::Cloud("google"),
                    _ => Who::Cloud("openai"),
                };
                if v.contains(&who) {
                    continue;
                }
                match v.iter().position(|w| *w == same) {
                    Some(i) => v.insert(i, who),
                    None => v.push(who),
                }
            }
        }
    }
    if local_ok && prefs.ai_mode != "online" && !v.contains(&Who::Local) {
        v.push(Who::Local);
    }
    v
}

pub fn current_order(s: &Settings) -> Vec<Who> {
    current_order_for(s, "")
}

/// The order for one job ("writing", "commands"): the job's own brain first
/// when one was chosen, then the Auto order.
pub fn current_order_for(s: &Settings, job: &str) -> Vec<Who> {
    let prefs = s.assistant.clone().sanitised();
    let ready = |id: &str| crate::brains::get(id).is_some_and(|b| b.auth().ready());
    let mut v = order(&prefs, &s.chat_provider, ready, ready);
    if !crate::brain_ai::local_configured(s, &prefs) {
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

/// The brain behind a place in the order. This PC's is whichever server
/// answers (Ollama, LM Studio, the built-in model, a custom address).
async fn brain_of(who: Who, s: &Settings, prefs: &AssistantPrefs) -> Option<&'static dyn crate::brains::Brain> {
    match who {
        Who::Local => crate::brains::get(&crate::local_chat::resolve(s, &prefs.local_provider).await),
        other => crate::brains::get(other.id()),
    }
}


/// Which kind of brain a chat provider id is: the chat keeps its own history
/// and streaming per kind, but which one it is is decided here, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyed {
    /// Claude with the user's key.
    Claude,
    /// An OpenAI-compatible cloud service with the user's key.
    Compat,
    /// Groovy's own local model.
    ThisPc,
    /// Anything else: a model server on this PC, a subscription, or unknown.
    Other,
}

pub fn keyed(provider: &str) -> Keyed {
    match provider {
        "anthropic" => Keyed::Claude,
        "llamacpp" => Keyed::ThisPc,
        p if cloud_id(p).is_some() => Keyed::Compat,
        _ => Keyed::Other,
    }
}

/// After a failed brain: whether the next one may answer. Only a limit (when
/// the user allows it), the network or a missing sign-in or key hand over.
pub fn may_switch(prefs: &AssistantPrefs, e: &BrainError) -> bool {
    match e.fault {
        Fault::Limit => prefs.switch_on_limit,
        Fault::Network | Fault::Auth => true,
        Fault::Other => false,
    }
}

/// The order for one job, with what the request needs: brains that can use
/// tools first when there are tools; only brains that can see for a picture.
pub fn order_for(s: &Settings, job: Job, with_tools: bool) -> Vec<Who> {
    let mut v = current_order_for(s, job.key());
    if job.kind == Kind::Vision {
        v.retain(|w| !matches!(w, Who::Local) && crate::brains::get(w.id()).is_some_and(|b| b.caps().vision));
    }
    if with_tools {
        v = tools_first(v);
    }
    v
}

/// Asks each brain in the order until one answers. Returns the answer in the
/// Messages shape and the id of the brain that gave it.
pub async fn run(app: &AppHandle, s: &Settings, job: Job, req: Request<'_>) -> Result<(Value, &'static str), String> {
    let prefs = s.assistant.clone().sanitised();
    let mut order = ranked(&prefs, job, !req.tools.is_empty(), order_for(s, job, !req.tools.is_empty()));
    // Round 4C item 3: a picture of the screen goes only to a brain that can see.
    if crate::screen::has_image(req.messages) {
        order.retain(|w| crate::brains::get(w.id()).is_some_and(|b| b.caps().vision));
    }
    ask(app, s, &prefs, order, req).await
}

/// One turn of the chat's own conversation (round 4D item 2): the brain picked
/// above the chat box, then its stand-in (`assistant::route`: the model on this
/// PC in hybrid mode), each through `Brain::chat`. The stand-in answers only
/// when the first failed for a reason it can fix (`worth_falling_back`), as
/// before. Returns the answer and the place in `order` that gave it, or every
/// error in order.
pub async fn chat(
    app: Option<&AppHandle>,
    s: &Settings,
    prefs: &AssistantPrefs,
    chat: &crate::chat::Chat,
    order: &[String],
    model: Option<&str>,
    query: String,
    context: Option<crate::chat::ChatContext>,
    on_delta: &super::Delta<'_>,
) -> Result<(crate::chat::ChatReply, usize), Vec<String>> {
    let mut errors = Vec::new();
    for (i, id) in order.iter().enumerate() {
        let result = if id == crate::assistant::NONE {
            Err(crate::i18n::t("Today's online limit is reached. Raise it in Groovy → Assistant, or choose local AI there."))
        } else {
            let id = crate::chat::this_pc(s, prefs, id).await;
            match super::get(&id) {
                None => Err(format!("Unknown chat provider: {id}")),
                Some(brain) => {
                    let model = model.map(str::to_string).unwrap_or_else(|| crate::chat::model_for(s, &id));
                    let turn = super::ChatTurn { app, settings: s, prefs, chat, model, query: query.clone(), context: context.clone(), on_delta };
                    brain.chat(turn).await.map_err(|e| e.message)
                }
            }
        };
        match result {
            Ok(reply) => return Ok((reply, i)),
            Err(e) => {
                let next = i + 1 < order.len() && crate::assistant::worth_falling_back(&e);
                if next {
                    crate::log::line(format!("chat: {id} unavailable, answering with {}", order[i + 1]));
                }
                errors.push(e);
                if !next {
                    break;
                }
            }
        }
    }
    Err(errors)
}

/// What a job needs of a brain, for the router.
pub fn need(job: Job, with_tools: bool) -> super::Caps {
    super::Caps {
        tools: with_tools,
        vision: job.kind == Kind::Vision,
        streaming: false,
        web_search: false,
        local: false,
        tier: if job.weight == Weight::Light { super::Tier::Small } else { super::Tier::Large },
    }
}

/// The order through the router (item 5); any brain it leaves out is appended.
pub fn ranked(prefs: &AssistantPrefs, job: Job, with_tools: bool, order: Vec<Who>) -> Vec<Who> {
    use super::router::{complete, router, Candidate};
    let candidates: Vec<Candidate> = order
        .iter()
        .map(|w| {
            let brain = crate::brains::get(w.id());
            Candidate {
                brain_id: w.id(),
                model: String::new(),
                // Only brains that are set up are in the order, so no file is looked at again.
                auth: match w {
                    Who::Cli(_) => super::Auth::Account { signed_in: true },
                    Who::Local => super::Auth::None,
                    _ => super::Auth::ApiKey { present: true },
                },
                caps: brain.map(|b| b.caps()).unwrap_or(super::Caps { tools: true, vision: false, streaming: false, web_search: false, local: true, tier: super::Tier::Small }),
                limited_until: None,
                last_latency_ms: None,
            }
        })
        .collect();
    let picks = complete(router(prefs).rank(job, need(job, with_tools), &candidates), order.len());
    picks.into_iter().map(|i| order[i]).collect()
}

async fn ask(app: &AppHandle, s: &Settings, prefs: &AssistantPrefs, order: Vec<Who>, req: Request<'_>) -> Result<(Value, &'static str), String> {
    let ctx = Ctx { app, settings: s, prefs };
    let mut last = crate::i18n::t("No AI is set up yet: choose one in Groovy → Assistant → Groovy's AI.");
    let mut tally = CloudTally::default();
    for who in order {
        let Some(brain) = brain_of(who, s, prefs).await else {
            crate::log::line(format!("brain ({who:?}): No local model configured."));
            last = "No local model configured.".into();
            continue;
        };
        match brain.turn(ctx, req).await {
            Ok(v) => {
                tally.answered(app, who);
                return Ok((v, brain.id()));
            }
            Err(e) => {
                crate::log::line(format!("brain ({who:?}): {e}"));
                tally.failed(who, e.fault);
                let stop = !may_switch(prefs, &e);
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
    let can = |w: &Who| matches!(w, Who::Local) || crate::brains::get(w.id()).is_some_and(|b| b.caps().tools);
    let (mut with, without): (Vec<Who>, Vec<Who>) = order.into_iter().partition(can);
    with.extend(without);
    with
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
        assert!(may_switch(&p, &BrainError::of("HTTP 429: quota")));
        p.switch_on_limit = false;
        assert!(!may_switch(&p, &BrainError::of("HTTP 429: quota")));
        assert!(may_switch(&p, &BrainError::of("connection refused")), "other failures still fall through");
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
    fn a_limit_reaches_the_next_brain_and_a_refusal_does_not() {
        let p = AssistantPrefs::default();
        assert!(may_switch(&p, &BrainError::of("You've hit your usage limit")));
        assert!(may_switch(&p, &BrainError::of("HTTP 401: invalid x-api-key")));
        assert!(may_switch(&p, &BrainError::of("claude-code is not installed.")));
        assert!(!may_switch(&p, &BrainError::of("HTTP 400: bad request")), "a refusal or bad request stops");
        assert!(!may_switch(&p, &BrainError::of("I can't help with that.")));
    }

    #[test]
    fn a_signed_in_account_goes_before_a_key_for_the_same_brain() {
        let p = AssistantPrefs { ai_mode: "hybrid".into(), brain: "auto".into(), ai_setup: "subscription".into(), ..Default::default() };
        let keys = |k: &str| k == "anthropic" || k == "google";
        let all = |_: &str| true;
        assert_eq!(
            order(&p, "anthropic", keys, all),
            vec![Who::Cli("claude-code"), Who::Claude, Who::Cli("gemini-cli"), Who::Cloud("google"), Who::Cli("codex")]
        );
        let none = |_: &str| false;
        assert_eq!(order(&p, "x", none, |t: &str| t == "gemini-cli"), vec![Who::Cli("gemini-cli")], "no key at all: the sign-in answers");
    }

    #[test]
    fn the_switcher_takes_its_order_from_the_router_unchanged_by_default() {
        let p = AssistantPrefs::default();
        let base = vec![Who::Cli("claude-code"), Who::Claude, Who::Cloud("google"), Who::Local];
        assert_eq!(ranked(&p, Job::COMMANDS, true, base.clone()), base);
        assert_eq!(ranked(&p, Job::WRITING, false, vec![]), vec![]);
        assert_eq!(need(Job::COMMANDS, true).tier, crate::brains::Tier::Small);
        assert!(need(Job { kind: Kind::Vision, weight: Weight::Heavy }, false).vision);
    }

    #[test]
    fn each_chat_provider_is_one_kind_of_brain() {
        assert_eq!(keyed("anthropic"), Keyed::Claude);
        assert_eq!(keyed("google"), Keyed::Compat);
        assert_eq!(keyed("openrouter"), Keyed::Compat);
        assert_eq!(keyed("llamacpp"), Keyed::ThisPc);
        assert_eq!(keyed("ollama"), Keyed::Other);
        assert_eq!(keyed("codex"), Keyed::Other);
    }

    #[test]
    fn a_job_keeps_its_model_per_job_key() {
        assert_eq!(Job::WRITING.key(), "writing");
        assert_eq!(Job::COMMANDS.key(), "commands");
        assert_eq!(Job { kind: Kind::Chat, weight: Weight::Heavy }.key(), "chat");
    }
}
