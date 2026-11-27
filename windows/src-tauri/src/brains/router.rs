// The router slot (round 4D item 5, docs/ROUTER_PLUGIN.md).
//
// A router ranks the brains that could answer a job. Groovy's own order
// (`DefaultRouter`) is today's: cloud and subscription first, this PC last,
// the user's order and a job's own brain on top. A router plugin, later, may
// send light jobs to small models and heavy ones to big models (`Job::weight`,
// `Caps::tier`). Whatever a router answers, the switcher appends every brain it
// left out (`complete`), so a bad plugin can never leave the user with none.
//
// The plugin transport is defined here but not enabled: a program chosen in
// Settings, started on demand, one JSON line in (the job, what it needs, the
// candidates; never the prompt, never a key), one line out (the ranked brain
// ids), 300 ms, then Groovy's own order.

// The plugin transport is defined but not enabled yet (item 5).
#![allow(dead_code)]

use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use super::switch::Job;
use super::{Auth, Caps};

/// One brain that could answer, as a router sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub brain_id: &'static str,
    /// The model it would use; empty for the brain's own default.
    pub model: String,
    pub auth: Auth,
    pub caps: Caps,
    /// Unix seconds until which it said its limit is reached, when known.
    pub limited_until: Option<u64>,
    pub last_latency_ms: Option<u64>,
}

pub trait Router: Sync {
    /// Indexes into `candidates`, best first. May leave some out.
    fn rank(&self, job: Job, need: Caps, candidates: &[Candidate]) -> Vec<usize>;
}

/// Groovy's own order: the candidates come already in it.
pub struct DefaultRouter;

impl Router for DefaultRouter {
    fn rank(&self, _job: Job, _need: Caps, candidates: &[Candidate]) -> Vec<usize> {
        (0..candidates.len()).collect()
    }
}

/// A router's answer made safe: no index twice, none out of range, and every
/// candidate it left out appended in Groovy's own order.
pub fn complete(ranked: Vec<usize>, n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(n);
    for i in ranked.into_iter().chain(0..n) {
        if i < n && !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

/// How long a plugin may take before Groovy's own order is used.
pub const PLUGIN_TIMEOUT: Duration = Duration::from_millis(300);

/// The one line a plugin gets: never the prompt, never a key.
pub fn plugin_request(job: Job, need: Caps, candidates: &[Candidate]) -> String {
    json!({ "job": job, "need": need, "candidates": candidates }).to_string()
}

/// The one line a plugin answers, `{"ranked": ["brain id", …]}`, as indexes.
/// Anything else, or no known id at all: None (Groovy's own order).
pub fn parse_plugin_answer(line: &str, candidates: &[Candidate]) -> Option<Vec<usize>> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let ids = v.get("ranked")?.as_array()?;
    let mut out = Vec::new();
    for id in ids {
        let id = id.as_str()?;
        if let Some(i) = candidates.iter().position(|c| c.brain_id == id) {
            out.push(i);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// A router plugin: a program the user chose. Started on demand for one
/// ranking and stopped after it. Not enabled yet (`router` never returns it).
pub struct PluginRouter {
    pub program: std::path::PathBuf,
}

impl Router for PluginRouter {
    fn rank(&self, job: Job, need: Caps, candidates: &[Candidate]) -> Vec<usize> {
        let fallback = || DefaultRouter.rank(job, need, candidates);
        let line = plugin_request(job, need, candidates);
        match ask_plugin(&self.program, &line, PLUGIN_TIMEOUT) {
            Some(answer) => parse_plugin_answer(&answer, candidates).unwrap_or_else(fallback),
            None => fallback(),
        }
    }
}

/// Runs the plugin for one line in and one line out, killed at `timeout`.
fn ask_plugin(program: &std::path::Path, line: &str, timeout: Duration) -> Option<String> {
    use std::io::{BufRead, Write};
    let mut cmd = std::process::Command::new(program);
    cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let mut child = crate::platform::no_console(&mut cmd).spawn().ok()?;
    {
        let mut stdin = child.stdin.take()?;
        let _ = writeln!(stdin, "{line}");
    }
    let stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut answer = String::new();
        let _ = std::io::BufReader::new(stdout).read_line(&mut answer);
        let _ = tx.send(answer);
    });
    let answer = rx.recv_timeout(timeout).ok();
    let _ = child.kill();
    let _ = child.wait();
    answer
}

/// The router in use. The plugin is defined but not enabled (round 4D item
/// 5): Groovy's own order always, whatever the setting holds.
pub fn router(_prefs: &crate::assistant::AssistantPrefs) -> &'static dyn Router {
    &DefaultRouter
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brains::switch::{Kind, Weight};
    use crate::brains::Tier;

    fn caps(tier: Tier) -> Caps {
        Caps { tools: true, vision: false, streaming: false, web_search: false, local: false, tier }
    }

    fn candidates() -> Vec<Candidate> {
        ["claude-code", "anthropic", "google", "ollama"]
            .into_iter()
            .map(|id| Candidate { brain_id: id, model: String::new(), auth: Auth::None, caps: caps(Tier::Large), limited_until: None, last_latency_ms: None })
            .collect()
    }

    const JOB: Job = Job { kind: Kind::Commands, weight: Weight::Light };

    #[test]
    fn the_default_router_keeps_groovys_own_order() {
        let c = candidates();
        assert_eq!(DefaultRouter.rank(JOB, caps(Tier::Small), &c), vec![0, 1, 2, 3]);
        assert_eq!(DefaultRouter.rank(JOB, caps(Tier::Small), &[]), Vec::<usize>::new());
    }

    #[test]
    fn a_router_that_leaves_brains_out_still_leaves_the_user_a_brain() {
        assert_eq!(complete(vec![2], 4), vec![2, 0, 1, 3]);
        assert_eq!(complete(vec![], 3), vec![0, 1, 2]);
        assert_eq!(complete(vec![9, 1, 1], 3), vec![1, 0, 2], "out of range and repeats are dropped");
    }

    #[test]
    fn an_empty_or_garbage_plugin_answer_means_groovys_own_order() {
        let c = candidates();
        assert_eq!(parse_plugin_answer(r#"{"ranked": ["google", "nobody", "anthropic"]}"#, &c), Some(vec![2, 1]));
        for bad in ["", "garbage", "{}", r#"{"ranked": []}"#, r#"{"ranked": "google"}"#, r#"{"ranked": [1, 2]}"#, r#"{"ranked": ["nobody"]}"#] {
            assert_eq!(parse_plugin_answer(bad, &c), None, "{bad}");
        }
    }

    #[test]
    fn the_plugin_never_gets_the_prompt_or_a_key() {
        let line = plugin_request(JOB, caps(Tier::Small), &candidates());
        let v: Value = serde_json::from_str(&line).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 3);
        assert!(v["job"]["weight"] == "light" && v["need"]["tier"] == "small");
        assert_eq!(v["candidates"][0]["brainId"], "claude-code");
        assert!(!line.contains("\n"), "one line");
    }

    #[test]
    fn a_missing_plugin_falls_back_to_the_default_order() {
        let r = PluginRouter { program: std::path::PathBuf::from("/nonexistent/groovy-router") };
        assert_eq!(r.rank(JOB, caps(Tier::Small), &candidates()), vec![0, 1, 2, 3]);
    }
}
