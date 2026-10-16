// What « OK Groovy » does with no AI at all (FIXES_ROUND4 item 1): music,
// volume, the time and date, timers, and what tools.rs already understands
// from a plain sentence (open an app, a website or a folder, reminders,
// to-dos, the system status). These are tried first; only what they don't
// cover goes to the AI, so "OK Groovy, play music" works with zero keys.

use tauri::AppHandle;

use crate::i18n::{t, tf};
use crate::tools::{self, Call, When};

#[derive(Debug, Clone, PartialEq)]
pub enum Basic {
    Media(Media),
    Volume(Volume),
    Time,
    Date,
    Tool(Call),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    Play,
    Pause,
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volume {
    Up,
    Down,
    Mute,
}

// Windows' virtual keys for the media and volume keys.
const VK_VOLUME_MUTE: u16 = 0xAD;
const VK_VOLUME_DOWN: u16 = 0xAE;
const VK_VOLUME_UP: u16 = 0xAF;
const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;
/// Each volume key press is 2%: five make a step you can hear.
const VOLUME_PRESSES: usize = 5;

/// The sentence in lower case without "please", "can you"… or end punctuation.
fn plain(text: &str) -> String {
    let mut t = text.trim().trim_end_matches(['.', '!', '?', ',']).to_lowercase().replace('’', "'");
    for lead in ["please ", "can you ", "could you ", "would you ", "hey groovy ", "ok groovy ", "okay groovy ", "groovy "] {
        if let Some(rest) = t.strip_prefix(lead) {
            t = rest.trim_start_matches(", ").to_string();
        }
    }
    t.trim_end_matches(" please").trim().to_string()
}

fn any(t: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|p| t == *p)
}

/// A timer: "set a timer for 5 minutes", "timer for ten minutes", "a 10 minute timer".
fn timer(t: &str) -> Option<When> {
    for lead in ["set a timer for ", "set timer for ", "start a timer for ", "timer for ", "set a timer of "] {
        if let Some(rest) = t.strip_prefix(lead) {
            return tools::parse_when(&format!("in {rest}"));
        }
    }
    let rest = t.strip_prefix("set a ").or_else(|| t.strip_prefix("start a ")).or_else(|| t.strip_prefix("a "))?;
    let length = rest.strip_suffix(" timer")?.replace('-', " ");
    let mut words = length.split_whitespace();
    let (n, unit) = (words.next()?, words.next()?);
    tools::parse_when(&format!("in {n} {unit}s"))
}

pub fn parse(text: &str) -> Option<Basic> {
    let t = plain(text);
    let t = t.as_str();
    if t.is_empty() {
        return None;
    }
    if any(t, &["what time is it", "what's the time", "whats the time", "what is the time", "tell me the time", "time", "what time is it now"]) {
        return Some(Basic::Time);
    }
    if any(t, &["what's the date", "whats the date", "what is the date", "what day is it", "what's today's date", "what is today's date", "what day is it today", "what's the date today"]) {
        return Some(Basic::Date);
    }
    let music = ["", " music", " the music", " my music", " some music", " spotify", " the song", " this song", " it"];
    let with = |verb: &str| music.iter().any(|m| t == format!("{verb}{m}"));
    if with("pause") || (with("stop") && t != "stop") || any(t, &["stop playing"]) {
        return Some(Basic::Media(Media::Pause));
    }
    if with("play") || with("resume") || with("unpause") || (with("continue") && t != "continue") {
        return Some(Basic::Media(Media::Play));
    }
    if any(t, &["next", "next song", "next track", "skip", "skip it", "skip this song", "skip this one", "skip the song", "play the next song"]) {
        return Some(Basic::Media(Media::Next));
    }
    if any(t, &["previous", "previous song", "previous track", "last song", "go back a song", "play the previous song", "play the last song"]) {
        return Some(Basic::Media(Media::Previous));
    }
    if any(t, &["volume up", "turn it up", "turn the volume up", "turn up the volume", "louder", "increase the volume", "raise the volume", "make it louder"]) {
        return Some(Basic::Volume(Volume::Up));
    }
    if any(t, &["volume down", "turn it down", "turn the volume down", "turn down the volume", "quieter", "lower the volume", "decrease the volume", "make it quieter"]) {
        return Some(Basic::Volume(Volume::Down));
    }
    if any(t, &["mute", "unmute", "mute it", "mute the sound", "unmute the sound", "mute the volume", "mute sound"]) {
        return Some(Basic::Volume(Volume::Mute));
    }
    if let Some(when) = timer(t) {
        return Some(Basic::Tool(Call::Remind { text: crate::i18n::t("Timer"), when }));
    }
    tools::parse(text).map(Basic::Tool)
}

/// Does it, and what Groovy says back.
pub fn run(app: &AppHandle, basic: &Basic) -> String {
    match basic {
        Basic::Time => {
            let now = crate::platform::local_time();
            tf("It's {time}.", &[("time", &format!("{}:{:02}", now.hour, now.minute))])
        }
        Basic::Date => {
            let now = crate::platform::local_time();
            tf("Today is {date}.", &[("date", &format!("{}-{:02}-{:02}", now.year, now.month, now.day))])
        }
        Basic::Media(m) => media(app, *m),
        Basic::Volume(v) => {
            let (key, presses, said) = match v {
                Volume::Up => (VK_VOLUME_UP, VOLUME_PRESSES, t("Volume up.")),
                Volume::Down => (VK_VOLUME_DOWN, VOLUME_PRESSES, t("Volume down.")),
                Volume::Mute => (VK_VOLUME_MUTE, 1, t("Sound muted or back on.")),
            };
            match crate::flow::os::send_keys(&vec![key; presses]) {
                Ok(()) => said,
                Err(_) => t("I couldn't change the volume here."),
            }
        }
        Basic::Tool(call) => {
            let (answer, pending) = tools::handle(call.clone());
            match pending {
                // Asked first (Settings → Permissions): the island shows the question; the voice says where.
                Some(_) => tf("{question} Allow it in Settings → Permissions first, then ask me again.", &[("question", &answer)]),
                None => answer,
            }
        }
    }
}

fn media(app: &AppHandle, m: Media) -> String {
    let playing = crate::spotify::playing();
    match (m, playing) {
        (Media::Play, Some(true)) => return t("It's already playing."),
        (Media::Pause, Some(false)) => return t("It's already paused."),
        _ => {}
    }
    let (action, key) = match m {
        Media::Play | Media::Pause => ("playPause", VK_MEDIA_PLAY_PAUSE),
        Media::Next => ("next", VK_MEDIA_NEXT_TRACK),
        Media::Previous => ("previous", VK_MEDIA_PREV_TRACK),
    };
    // Spotify when Groovy follows it; otherwise the media key, for whatever plays.
    let done = playing.is_some() && tauri::async_runtime::block_on(crate::spotify::spotify_control(app.clone(), action.into(), None))
        || crate::flow::os::send_keys(&[key]).is_ok();
    if !done {
        return t("Nothing is playing that I can control.");
    }
    match m {
        Media::Play => t("Playing."),
        Media::Pause => t("Paused."),
        Media::Next => t("Next song."),
        Media::Previous => t("Previous song."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_volume_and_time_need_no_ai() {
        assert_eq!(parse("Play music."), Some(Basic::Media(Media::Play)));
        assert_eq!(parse("please pause the music"), Some(Basic::Media(Media::Pause)));
        assert_eq!(parse("Stop the music"), Some(Basic::Media(Media::Pause)));
        assert_eq!(parse("Next song"), Some(Basic::Media(Media::Next)));
        assert_eq!(parse("skip this song"), Some(Basic::Media(Media::Next)));
        assert_eq!(parse("previous track"), Some(Basic::Media(Media::Previous)));
        assert_eq!(parse("Turn it up"), Some(Basic::Volume(Volume::Up)));
        assert_eq!(parse("volume down"), Some(Basic::Volume(Volume::Down)));
        assert_eq!(parse("Mute."), Some(Basic::Volume(Volume::Mute)));
        assert_eq!(parse("What time is it?"), Some(Basic::Time));
        assert_eq!(parse("What’s the date today?"), Some(Basic::Date));
    }

    #[test]
    fn timers_and_tools_rs_sentences() {
        assert!(matches!(parse("Set a timer for 5 minutes"), Some(Basic::Tool(Call::Remind { when: When::In(300), .. }))));
        assert!(matches!(parse("timer for ten minutes"), Some(Basic::Tool(Call::Remind { when: When::In(600), .. }))));
        assert!(matches!(parse("set a 20 minute timer"), Some(Basic::Tool(Call::Remind { when: When::In(1200), .. }))));
        assert_eq!(parse("Open notepad"), Some(Basic::Tool(Call::OpenApp("notepad".into()))));
        assert!(matches!(parse("open github.com"), Some(Basic::Tool(Call::OpenUrl(_)))));
        assert!(matches!(parse("remind me to call Sam in 10 minutes"), Some(Basic::Tool(Call::Remind { .. }))));
        // Round 4 item 5: "play X" that isn't plain music is a Spotify search.
        assert!(matches!(parse("play the latest Taylor Swift album"), Some(Basic::Tool(Call::Pc(crate::pc::Action::SpotifyPlay(_))))));
    }

    #[test]
    fn real_questions_still_go_to_the_ai() {
        for q in [
            "what's the time in Tokyo",
            "stop",
            "continue",
            "why is the sky blue",
            "what's on my calendar today",
            "pause for a moment and tell me a joke",
        ] {
            assert_eq!(parse(q), None, "{q}");
        }
    }
}
