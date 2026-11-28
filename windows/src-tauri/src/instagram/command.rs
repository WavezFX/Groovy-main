// Round 7 item 2: "message Sam on Instagram saying I'm late", "reply on my
// way", "open my Instagram messages", understood with no AI (round 4D item 7's
// command path: tried before any brain, from voice, chat and calls).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// A message to someone: their name as said, and the text ("" opens the conversation only).
    Message { to: String, text: String, write: bool },
    /// An answer to the newest DM.
    Reply { text: String, write: bool },
    /// The DMs list.
    Inbox,
}

/// "on Instagram" and the ways it is said.
const MARKERS: &[&str] = &[" on instagram", " on insta", " on ig", " via instagram", " in instagram", " over instagram", " through instagram"];
const LEADS: &[&str] = &["please ", "can you ", "could you ", "would you ", "will you ", "hey groovy ", "yo groovy ", "ok groovy ", "okay groovy ", "groovy ", "go ahead and ", "i want to ", "i'd like to "];
/// Before the name.
const VERBS: &[&str] = &[
    "send a message to ",
    "send message to ",
    "send a dm to ",
    "send a text to ",
    "write a message to ",
    "write to ",
    "message ",
    "msg ",
    "dm ",
    "text ",
    "tell ",
    "ask ",
    "send ",
];
/// Between the name (or the marker) and the text.
const SAYING: &[&str] = &[" saying ", " and say ", " to say ", " that ", " say ", ": ", ", ", " - "];
/// The AI writes the message only when asked to.
const WRITE: &[&str] = &[" and write it for me", " write it for me", " and write it", " for me in a nice way", " in a nice way", " nicely", " and make it sound nice"];

/// The sentence without fillers or end punctuation, in its own case, and its ASCII lower case (same length).
fn tidy(text: &str) -> (String, String) {
    let mut o = text.trim().replace('’', "'");
    loop {
        let before = o.len();
        o = o.trim_start_matches([',', ' ']).to_string();
        let l = o.to_ascii_lowercase();
        // "hey groovy " or "hey groovy, ".
        if let Some(n) = LEADS.iter().find_map(|p| (l.starts_with(p) || l.starts_with(&format!("{},", p.trim_end()))).then_some(p.len())) {
            o = o[n..].to_string();
        }
        o = o.trim_end_matches(['.', '!', '?', ' ']).to_string();
        for tail in [" please", " for me", " groovy"] {
            if o.to_ascii_lowercase().ends_with(tail) && !WRITE.iter().any(|w| o.to_ascii_lowercase().ends_with(w)) {
                o.truncate(o.len() - tail.len());
            }
        }
        if o.len() == before {
            break;
        }
    }
    let l = o.to_ascii_lowercase();
    (o, l)
}

/// Takes the "write it for me" part off the text.
fn wants_writing(text: &str) -> (String, bool) {
    let p = format!(" {text}");
    let l = p.to_ascii_lowercase();
    match WRITE.iter().find_map(|w| l.find(w).map(|at| (at, w.len()))) {
        Some((at, n)) => (format!("{}{}", &p[..at], &p[at + n..]).trim().to_string(), true),
        None => (text.trim().to_string(), false),
    }
}

/// The text after "saying", "that", ":"…
fn after_saying(text: &str) -> String {
    let t = text.trim();
    let l = t.to_ascii_lowercase();
    for lead in ["saying ", "and say ", "to say ", "that ", "say ", ": ", ":", ", ", "- ", "and tell them ", "and tell him ", "and tell her "] {
        if l.starts_with(lead) {
            return t[lead.len()..].trim().to_string();
        }
    }
    t.to_string()
}

/// "Sam a message", "to Sam" → "Sam". None when nothing like a name is left.
fn clean_name(name: &str) -> Option<String> {
    let mut n = name.trim().trim_matches(|c: char| c == '"' || c == ',' || c == ':').trim().to_string();
    let l = n.to_ascii_lowercase();
    for tail in [" a message", " a dm", " a text", " a msg", " a note"] {
        if l.ends_with(tail) {
            n.truncate(n.len() - tail.len());
            break;
        }
    }
    if n.to_ascii_lowercase().starts_with("to ") {
        n = n[3..].to_string();
    }
    let n = n.trim().trim_start_matches('@').to_string();
    let words = n.split_whitespace().count();
    let l = n.to_ascii_lowercase();
    if words == 0 || words > 4 || n.chars().count() > 40 || ["a message", "a photo", "it", "this", "that", "something", "my messages", "messages"].contains(&l.as_str()) {
        return None;
    }
    Some(n)
}

/// Splits "Sam saying hi" / "Sam: hi" at the first "saying" word: (name, text).
fn split_saying(s: &str) -> (String, String) {
    let l = s.to_ascii_lowercase();
    match SAYING.iter().filter_map(|w| l.find(w).map(|at| (at, w.len()))).min() {
        Some((at, n)) => (s[..at].to_string(), s[at + n..].trim().to_string()),
        None => (s.to_string(), String::new()),
    }
}

/// Reads one sentence. `last_sender` is who sent the newest DM, when one came
/// in the last few minutes and Instagram is on: only then does a bare "reply"
/// mean Instagram.
pub fn parse(text: &str, last_sender: Option<&str>) -> Option<Command> {
    let (o, l) = tidy(text);
    if l.is_empty() {
        return None;
    }
    // "open my Instagram messages", "check my Instagram DMs", "show Instagram inbox".
    let opens = ["open ", "check ", "show ", "show me ", "go to ", "read "].iter().any(|v| l.starts_with(v));
    if opens && (l.contains("instagram") || l.contains("insta ") || l.ends_with("insta")) && ["message", "dm", "inbox", "chats"].iter().any(|w| l.contains(w)) && !l.contains(" to ") {
        return Some(Command::Inbox);
    }
    // The first one, and the longest there (" on instagram", not " on insta").
    let marker = MARKERS.iter().filter_map(|m| l.find(m).map(|at| (at, m.len()))).min_by_key(|&(at, n)| (at, std::cmp::Reverse(n)));
    // "reply …": the newest DM, or anyone with "on Instagram".
    for lead in ["reply to ", "reply ", "respond to ", "respond "] {
        if !(l.starts_with(lead) || l == lead.trim_end()) {
            continue;
        }
        if marker.is_none() && last_sender.is_none() {
            return None;
        }
        let rest = o.get(lead.len()..).unwrap_or("").to_string();
        // Without the marker.
        let rest = match marker {
            Some((at, n)) => format!("{} {}", &o[lead.len().min(at)..at], &o[at + n..]).trim().to_string(),
            None => rest,
        };
        let (rest, write) = wants_writing(&rest);
        if lead.ends_with(" to ") {
            if let Some(who) = last_sender.filter(|w| rest.to_ascii_lowercase().starts_with(&w.to_ascii_lowercase())) {
                return Some(Command::Reply { text: after_saying(&rest[who.len()..]), write });
            }
            let (name, text) = split_saying(&rest);
            if let Some(to) = clean_name(&name) {
                return Some(Command::Message { to, text, write });
            }
            return None;
        }
        return Some(Command::Reply { text: after_saying(&rest), write });
    }
    let (at, n) = match marker {
        Some(m) => m,
        None => {
            // "Instagram Sam: running late", "on Instagram, message Sam saying hi".
            let lead = ["instagram ", "on instagram ", "on instagram, ", "insta "].iter().find(|p| l.starts_with(**p))?;
            let rest = o[lead.len()..].to_string();
            let rest_l = rest.to_ascii_lowercase();
            let rest = VERBS.iter().find(|v| rest_l.starts_with(**v)).map_or(rest.clone(), |v| rest[v.len()..].to_string());
            let (rest, write) = wants_writing(&rest);
            let (name, text) = split_saying(&rest);
            return clean_name(&name).map(|to| Command::Message { to, text, write });
        }
    };
    let head = &o[..at];
    let head_l = &l[..at];
    let tail = &o[at + n..];
    let verb = VERBS.iter().find(|v| head_l.starts_with(**v))?;
    let asks = *verb == "ask ";
    let (name, text) = split_saying(&head[verb.len()..]);
    let text = if text.is_empty() { after_saying(tail) } else { format!("{} {}", text, tail.trim()).trim().to_string() };
    let (text, write) = wants_writing(&text);
    let to = clean_name(&name)?;
    let write = write || (asks && !text.is_empty());
    Some(Command::Message { to, text, write })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(to: &str, text: &str) -> Option<Command> {
        Some(Command::Message { to: to.into(), text: text.into(), write: false })
    }

    #[test]
    fn many_ways_to_say_it() {
        let cases: &[(&str, Option<Command>)] = &[
            ("message Sam on Instagram", msg("Sam", "")),
            ("Message Sam on Instagram saying I'm 10 minutes late", msg("Sam", "I'm 10 minutes late")),
            ("message sam on instagram I'm 10 minutes late.", msg("sam", "I'm 10 minutes late")),
            ("Hey Groovy, message Sam on Instagram that I'm on my way", msg("Sam", "I'm on my way")),
            ("please message Mia on insta saying see you at 6", msg("Mia", "see you at 6")),
            ("can you DM sam.k on Instagram: running late", msg("sam.k", "running late")),
            ("dm @mia_r on ig saying hi", msg("mia_r", "hi")),
            ("send a message to Sam on Instagram saying hello", msg("Sam", "hello")),
            ("Send Sam a message on Instagram saying hello", msg("Sam", "hello")),
            ("send Sam a DM on Instagram", msg("Sam", "")),
            ("text Jo on Instagram, are you coming?", msg("Jo", "are you coming")),
            ("tell Jo on Instagram that dinner's at 8", msg("Jo", "dinner's at 8")),
            ("message Sam saying I'm late on Instagram", msg("Sam", "I'm late")),
            ("write to Sam Kay on Instagram saying thanks", msg("Sam Kay", "thanks")),
            ("message Sam via Instagram saying ok", msg("Sam", "ok")),
            ("Instagram Sam: running late", msg("Sam", "running late")),
            ("on Instagram, message Sam saying hi", msg("Sam", "hi")),
            ("could you message Sam on Instagram for me", msg("Sam", "")),
            ("msg sam on insta yo", msg("sam", "yo")),
            ("message Sam on Instagram saying I'm late and write it for me", Some(Command::Message { to: "Sam".into(), text: "I'm late".into(), write: true })),
            ("ask Sam on Instagram if he's free tonight", Some(Command::Message { to: "Sam".into(), text: "if he's free tonight".into(), write: true })),
            ("open my Instagram messages", Some(Command::Inbox)),
            ("check my Instagram DMs", Some(Command::Inbox)),
            ("show me my insta messages", Some(Command::Inbox)),
            ("reply on Instagram on my way", Some(Command::Reply { text: "on my way".into(), write: false })),
            ("reply to Sam on Instagram saying yes", msg("Sam", "yes")),
            // Not Instagram: left for the other commands and the brain.
            ("message Sam saying hi", None),
            ("send a photo", None),
            ("open Instagram", None),
            ("what's on Instagram today", None),
            ("send it on Instagram", None),
            ("play music", None),
            ("reply saying ok", None),
            ("", None),
        ];
        for (said, want) in cases {
            assert_eq!(&parse(said, None), want, "{said}");
        }
    }

    #[test]
    fn a_bare_reply_needs_a_recent_dm() {
        assert_eq!(parse("reply on my way", Some("sam.k")), Some(Command::Reply { text: "on my way".into(), write: false }));
        assert_eq!(parse("Reply saying ok, see you", Some("sam.k")), Some(Command::Reply { text: "ok, see you".into(), write: false }));
        assert_eq!(parse("reply to sam.k saying yes", Some("sam.k")), Some(Command::Reply { text: "yes".into(), write: false }));
        assert_eq!(parse("reply", Some("sam.k")), Some(Command::Reply { text: String::new(), write: false }));
        assert_eq!(parse("reply nicely that I can't make it", Some("jo")), Some(Command::Reply { text: "I can't make it".into(), write: true }));
        assert_eq!(parse("reply on my way", None), None, "no DM came in: reply means nothing here");
    }
}
