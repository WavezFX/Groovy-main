// Who the assistant is and where its answers come from.
//
// - Personality (Settings → Assistant): name, preset, humour, answer length,
//   proactivity, language and the user's own instructions. They become part
//   of the system prompt every provider gets, so they change the answers, not
//   just a label.
// - AI mode: local only (nothing is ever sent to a cloud provider), online
//   only, or hybrid (the chosen provider, and the local one when the cloud is
//   unreachable, has no key, or the daily limit is reached).
// - Performance profile: what a local model may use (models.rs).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AssistantPrefs {
    pub name: String,
    /// friendly | professional | sarcastic | energetic | quiet | assistant
    pub personality: String,
    /// 0 (none) … 10 (constant jokes).
    pub humour: u8,
    /// short | balanced | detailed
    pub response_length: String,
    /// low | normal | high
    pub proactivity: String,
    /// A language name ("Spanish") to always answer in; empty = the user's language.
    pub language: String,
    /// The user's own extra instructions, appended as written.
    pub custom_instructions: String,
    /// hybrid | local | online
    pub ai_mode: String,
    /// The local provider used in local mode and as the hybrid fallback:
    /// llamacpp | ollama | lmstudio | custom
    pub local_provider: String,
    /// The GGUF file (in the models folder) the built-in engine runs; empty = the first one found.
    pub local_model: String,
    /// Layers to put on the GPU with the Vulkan engine; 0 = CPU only.
    pub gpu_layers: u32,
    /// Online messages allowed per day; 0 = no limit.
    pub daily_online_limit: u32,
    /// Who answers « OK Groovy », Flow's clean-up and the call notes (brain_ai.rs):
    /// "auto" (the AI chosen at setup: a cloud key, then a subscription CLI, then
    /// local if allowed), or one of BRAINS.
    pub brain: String,
    /// What the user picked in the AI setup: "" (not asked yet), "cloud",
    /// "subscription" or "local". Until it is "local" (or the mode is "This PC
    /// only"), Groovy never starts a local model: on an 8 GB PC llama-server takes
    /// about 2 GB.
    pub ai_setup: String,
    pub memory_enabled: bool,
    pub history_enabled: bool,
    /// Chat history older than this is deleted; 0 = keep forever.
    pub history_retention_days: u32,
    /// lightweight | balanced | performance
    pub performance_profile: String,
}

impl Default for AssistantPrefs {
    fn default() -> Self {
        Self {
            name: "Groovy".into(),
            personality: "friendly".into(),
            humour: 4,
            response_length: "balanced".into(),
            proactivity: "normal".into(),
            language: String::new(),
            custom_instructions: String::new(),
            ai_mode: "hybrid".into(),
            local_provider: "llamacpp".into(),
            local_model: String::new(),
            gpu_layers: 0,
            daily_online_limit: 0,
            brain: "auto".into(),
            ai_setup: String::new(),
            memory_enabled: true,
            history_enabled: true,
            history_retention_days: 90,
            performance_profile: "balanced".into(),
        }
    }
}

/// Who can answer the background features: "auto", a local model, a cloud key, or a subscription CLI.
pub const BRAINS: &[&str] = &["auto", "local", "claude", "google", "openai", "openrouter", "claude-code", "codex"];

pub const PERSONALITIES: &[&str] = &["friendly", "professional", "sarcastic", "energetic", "quiet", "assistant"];
pub const LOCAL_PROVIDERS: &[&str] = &["llamacpp", "ollama", "lmstudio", "custom"];
const MAX_NAME: usize = 32;
const MAX_INSTRUCTIONS: usize = 1500;

impl AssistantPrefs {
    /// Whether a local model may be started: only when the user chose local AI.
    pub fn local_allowed(&self) -> bool {
        self.ai_setup == "local" || self.ai_mode == "local"
    }

    /// Out-of-range values (a hand-edited settings.json) fall back to the defaults.
    pub fn sanitised(mut self) -> Self {
        let d = Self::default();
        let one_of = |v: &mut String, ok: &[&str], dflt: &str| {
            if !ok.contains(&v.as_str()) {
                *v = dflt.to_string();
            }
        };
        one_of(&mut self.personality, PERSONALITIES, &d.personality);
        one_of(&mut self.response_length, &["short", "balanced", "detailed"], &d.response_length);
        one_of(&mut self.proactivity, &["low", "normal", "high"], &d.proactivity);
        one_of(&mut self.ai_mode, &["hybrid", "local", "online"], &d.ai_mode);
        one_of(&mut self.local_provider, LOCAL_PROVIDERS, &d.local_provider);
        one_of(&mut self.brain, BRAINS, &d.brain);
        one_of(&mut self.ai_setup, &["", "cloud", "subscription", "local"], &d.ai_setup);
        one_of(&mut self.performance_profile, &["lightweight", "balanced", "performance"], &d.performance_profile);
        self.humour = self.humour.min(10);
        self.gpu_layers = self.gpu_layers.min(999);
        if self.local_model.contains(['/', '\\']) || self.local_model.contains("..") {
            self.local_model.clear();
        }
        let name: String = self.name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        self.name = if name.is_empty() { d.name } else { name };
        self.language = self.language.trim().chars().filter(|c| c.is_alphabetic() || *c == ' ' || *c == '-').take(40).collect();
        self.custom_instructions = self.custom_instructions.trim().chars().take(MAX_INSTRUCTIONS).collect();
        self
    }
}

/// The personality part of the system prompt.
pub fn personality_prompt(p: &AssistantPrefs) -> String {
    let style = match p.personality.as_str() {
        "professional" => "Your tone is professional, precise and courteous. No slang, no emoji.",
        "sarcastic" => "You are playfully sarcastic and dry-witted, but never mean, and you still give a correct, useful answer.",
        "energetic" => "You are upbeat and enthusiastic, with lively wording — but the substance always comes first.",
        "quiet" => "You are calm and understated. Say only what is needed, in a gentle tone.",
        "assistant" => "You behave like a focused, neutral assistant: no small talk, no persona flourishes, just the help asked for.",
        _ => "You are warm, friendly and encouraging, like a helpful friend who knows a lot.",
    };
    let humour = match p.humour {
        0 => "Do not use humour.",
        1..=3 => "Use humour rarely and lightly.",
        4..=6 => "A light joke now and then is welcome when the moment suits it.",
        _ => "Be playful and funny often, as long as the answer stays useful.",
    };
    let length = match p.response_length.as_str() {
        "short" => "Keep answers short: a few sentences, or a brief list, unless the user explicitly asks for more.",
        "detailed" => "Be thorough and complete — use as much detail as the task requires.",
        _ => "Match the length to the question: brief for simple things, complete for complex tasks.",
    };
    let proactive = match p.proactivity.as_str() {
        "low" => "Answer only what is asked; do not suggest extra steps unless asked.",
        "high" => "Be proactive: point out problems you notice and suggest useful next steps.",
        _ => "Suggest a next step only when it is clearly helpful.",
    };
    let language = if p.language.is_empty() {
        "Respond in the user's language.".to_string()
    } else {
        format!("Always respond in {}, whatever language the user writes in.", p.language)
    };
    let mut out = format!("{style} {humour} {length} {proactive} {language}");
    if !p.custom_instructions.is_empty() {
        out.push_str("\n\nThe user's own instructions for you:\n");
        out.push_str(&p.custom_instructions);
    }
    out
}

// ── Where a message goes ──────────────────────────────────────────────────────

/// True for a provider that runs on this computer. The custom server counts as
/// local only when its address is this machine.
pub fn is_local(provider: &str, custom_url: &str) -> bool {
    match provider {
        "llamacpp" | "ollama" | "lmstudio" => true,
        "custom" => crate::net::normalise_server_url(custom_url)
            .map(|u| crate::net::is_loopback_url(&u))
            .unwrap_or(false),
        _ => false,
    }
}

/// No one may answer (the online limit is reached and local AI is off).
pub const NONE: &str = "none";

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Who gets the message first.
    pub primary: String,
    /// Who gets it when the primary fails for a reason the fallback can fix.
    pub fallback: Option<String>,
    /// Why the primary is not the provider the user picked, when it isn't.
    pub note: Option<&'static str>,
}

/// Where a message goes, given the mode, the provider picked in the chat, and
/// whether the daily online limit is already used up.
pub fn route(prefs: &AssistantPrefs, picked: &str, custom_url: &str, limit_reached: bool) -> Route {
    let local = prefs.local_provider.clone();
    let picked_local = is_local(picked, custom_url);
    // Local AI not chosen: no local model stands in for an online provider.
    if !prefs.local_allowed() && !picked_local {
        if limit_reached && prefs.ai_mode != "online" {
            return Route { primary: NONE.into(), fallback: None, note: None };
        }
        return Route { primary: picked.into(), fallback: None, note: None };
    }
    match prefs.ai_mode.as_str() {
        "local" if !picked_local => Route {
            primary: local,
            fallback: None,
            note: Some(crate::i18n::n_("Local-only mode: answered by the model on this computer.")),
        },
        "local" => Route { primary: picked.into(), fallback: None, note: None },
        "online" => Route { primary: picked.into(), fallback: None, note: None },
        _ if picked_local => Route { primary: picked.into(), fallback: None, note: None },
        _ if limit_reached => Route {
            primary: local,
            fallback: None,
            note: Some(crate::i18n::n_("Today's online limit is reached: answered by the model on this computer.")),
        },
        _ => Route { primary: picked.into(), fallback: Some(local), note: None },
    }
}

/// Whether an error from an online provider is one a local model can stand in
/// for: no connection, a timeout, no key, the provider down or rate-limiting.
/// A refusal or a bad request is not: the user should see it.
pub fn worth_falling_back(err: &str) -> bool {
    let lower = err.to_lowercase();
    let has_code = |code: &str| err.split(|c: char| !c.is_ascii_digit()).any(|w| w == code);
    ["429", "500", "502", "503", "504", "529"].iter().any(|c| has_code(c))
        || [
            "cannot reach",
            "could not reach",
            "timed out",
            "timeout",
            "connect",
            "dns",
            "network",
            "api key missing",
            "no api key",
            "offline",
            "error sending request",
        ]
        .iter()
        .any(|w| lower.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs(mode: &str) -> AssistantPrefs {
        AssistantPrefs { ai_mode: mode.into(), ..Default::default() }
    }

    #[test]
    fn personality_changes_the_prompt() {
        let base = personality_prompt(&AssistantPrefs::default());
        assert!(base.contains("warm, friendly"));
        assert!(base.contains("Respond in the user's language."));
        let p = AssistantPrefs {
            personality: "sarcastic".into(),
            humour: 0,
            response_length: "short".into(),
            proactivity: "high".into(),
            language: "Spanish".into(),
            custom_instructions: "Call me Captain.".into(),
            ..Default::default()
        };
        let s = personality_prompt(&p);
        assert!(s.contains("sarcastic"));
        assert!(s.contains("Do not use humour."));
        assert!(s.contains("Keep answers short"));
        assert!(s.contains("Be proactive"));
        assert!(s.contains("Always respond in Spanish"));
        assert!(s.ends_with("Call me Captain."));
        for preset in PERSONALITIES {
            let s = personality_prompt(&AssistantPrefs { personality: preset.to_string(), ..Default::default() });
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn sanitising_fixes_hand_edited_values() {
        let p = AssistantPrefs {
            name: "  \u{7}  ".into(),
            personality: "evil".into(),
            humour: 99,
            ai_mode: "cloud".into(),
            local_provider: "openai".into(),
            language: "Deutsch<script>".into(),
            custom_instructions: "x".repeat(5000),
            ..Default::default()
        }
        .sanitised();
        assert_eq!(p.name, "Groovy");
        assert_eq!(p.personality, "friendly");
        assert_eq!(p.humour, 10);
        assert_eq!(p.ai_mode, "hybrid");
        assert_eq!(p.local_provider, "llamacpp");
        assert_eq!(p.language, "Deutschscript");
        assert_eq!(p.custom_instructions.len(), MAX_INSTRUCTIONS);
    }

    #[test]
    fn local_is_this_computer_only() {
        assert!(is_local("ollama", ""));
        assert!(is_local("llamacpp", ""));
        assert!(!is_local("openai", ""));
        assert!(!is_local("anthropic", ""));
        assert!(is_local("custom", "http://127.0.0.1:8080"));
        assert!(!is_local("custom", "https://llm.example.com"));
    }

    #[test]
    fn local_mode_never_sends_to_the_cloud() {
        let r = route(&prefs("local"), "anthropic", "", false);
        assert_eq!(r.primary, "llamacpp");
        assert_eq!(r.fallback, None);
        assert!(r.note.is_some());
        assert_eq!(route(&prefs("local"), "ollama", "", false).primary, "ollama");
        let r = route(&prefs("local"), "custom", "https://remote.example", false);
        assert_eq!(r.primary, "llamacpp", "a remote custom server is not local");
    }

    #[test]
    fn online_mode_has_no_fallback() {
        let r = route(&prefs("online"), "openai", "", false);
        assert_eq!((r.primary.as_str(), r.fallback), ("openai", None));
    }

    #[test]
    fn hybrid_falls_back_to_local_once_local_is_chosen() {
        let local = AssistantPrefs { ai_mode: "hybrid".into(), ai_setup: "local".into(), ..Default::default() };
        let r = route(&local, "openai", "", false);
        assert_eq!(r.primary, "openai");
        assert_eq!(r.fallback.as_deref(), Some("llamacpp"));
        let r = route(&local, "openai", "", true);
        assert_eq!(r.primary, "llamacpp", "limit reached goes local");
        assert_eq!(route(&local, "ollama", "", false).fallback, None);
    }

    #[test]
    fn without_local_ai_chosen_no_local_model_stands_in() {
        // The default: setup not done, hybrid mode.
        let p = prefs("hybrid");
        assert!(!p.local_allowed());
        let r = route(&p, "openai", "", false);
        assert_eq!((r.primary.as_str(), r.fallback), ("openai", None));
        assert_eq!(route(&p, "openai", "", true).primary, NONE, "the limit is a limit, not a local start");
        // Picking a local provider in the chat is still an explicit choice.
        assert_eq!(route(&p, "llamacpp", "", false).primary, "llamacpp");
        let cloud = AssistantPrefs { ai_setup: "cloud".into(), ..Default::default() };
        assert!(!cloud.local_allowed());
        assert!(AssistantPrefs { ai_setup: "local".into(), ..Default::default() }.local_allowed());
    }

    #[test]
    fn brains_and_setup_are_checked() {
        let p = AssistantPrefs { brain: "gpt-9".into(), ai_setup: "everything".into(), ..Default::default() }.sanitised();
        assert_eq!((p.brain.as_str(), p.ai_setup.as_str()), ("auto", ""));
        let p = AssistantPrefs { brain: "codex".into(), ai_setup: "subscription".into(), ..Default::default() }.sanitised();
        assert_eq!((p.brain.as_str(), p.ai_setup.as_str()), ("codex", "subscription"));
    }

    #[test]
    fn which_errors_fall_back() {
        assert!(worth_falling_back("Cannot reach https://api.openai.com. Is the server running?"));
        assert!(worth_falling_back("error sending request for url (https://api.anthropic.com/v1/messages)"));
        assert!(worth_falling_back("API key missing. Open settings."));
        assert!(worth_falling_back("OpenAI: HTTP 503 overloaded"));
        assert!(worth_falling_back("rate limited (429)"));
        assert!(!worth_falling_back("HTTP 400: bad request"));
        assert!(!worth_falling_back("The model refused to answer"));
        assert!(!worth_falling_back("HTTP 4290 nonsense"));
    }
}
