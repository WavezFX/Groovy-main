// Round 7 item 1: Instagram's DMs from Windows' notifications. The mail
// listener (island_tiles.rs) hands each new notification here while Instagram
// is on; this decides whether it is an Instagram DM and pulls out who sent it
// and a short preview. Pure text work: nothing here talks to Instagram.

use serde::Serialize;

/// The longest preview kept, in characters.
pub const PREVIEW_CHARS: usize = 120;

/// A new Instagram DM, as the island's tile shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dm {
    pub id: u32,
    /// Who sent it, as the notification names them.
    pub sender: String,
    /// Their Instagram username, when the name is one ("sam.k_92").
    pub username: Option<String>,
    /// The start of the message ("" for "sent you a photo" and the like).
    pub preview: String,
    /// "app" (the Instagram app or an installed Instagram web app) or "web" (a browser tab's notification).
    pub source: &'static str,
    /// The app that showed it (its AppUserModelID), to open it again.
    pub app_id: String,
}

/// What Instagram's notifications say that isn't a DM.
const NOT_DMS: &[&str] = &[
    "liked your",
    "started following you",
    "requested to follow",
    "commented",
    "mentioned you",
    "tagged you",
    "is live",
    "went live",
    "started a live",
    "posted",
    "shared a post",
    "added to their story",
    "new login",
    "suggested for you",
    "follow request",
];

fn has_domain(s: &str) -> bool {
    s.to_ascii_lowercase().contains("instagram.com")
}

/// Whether a notification is Instagram's: from the app (or its installed web
/// app), or a browser's site notification from instagram.com.
pub fn is_instagram(app: &str, app_id: &str, title: &str, body: &str) -> bool {
    app.to_ascii_lowercase().contains("instagram") || app_id.to_ascii_lowercase().contains("instagram") || has_domain(title) || has_domain(body)
}

/// A real Instagram username: letters, digits, dots and underscores, at most 30.
pub fn username(name: &str) -> Option<String> {
    let n = name.trim().trim_start_matches('@');
    (!n.is_empty() && n.len() <= 30 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') && !n.starts_with('.') && !n.ends_with('.'))
        .then(|| n.to_ascii_lowercase())
}

fn cut(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        return s;
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// The DM in a notification, or None when it isn't Instagram's or isn't a DM.
/// `body` is the notification's texts after the first, joined with " · "
/// (island_tiles.rs); a browser adds "www.instagram.com" as one of them.
pub fn from_notification(id: u32, app: &str, app_id: &str, title: &str, body: &str) -> Option<Dm> {
    if !is_instagram(app, app_id, title, body) {
        return None;
    }
    let app_lower = app.to_ascii_lowercase();
    let source = if app_lower.contains("instagram") || app_id.to_ascii_lowercase().contains("instagram") { "app" } else { "web" };
    // The browser's attribution line ("www.instagram.com") is no part of the message.
    let parts: Vec<&str> = body.split(" · ").map(str::trim).filter(|p| !p.is_empty() && !(has_domain(p) && p.split_whitespace().count() == 1)).collect();
    let mut text = parts.join(" ");
    let mut title = title.trim().to_string();
    if has_domain(&title) && title.split_whitespace().count() == 1 {
        title.clear();
    }
    // "Instagram" as the title: the sender is in the body, "sam: hey" or "sam sent you a message".
    if title.is_empty() || title.eq_ignore_ascii_case("instagram") {
        if let Some((who, rest)) = text.split_once(": ") {
            title = who.trim().to_string();
            text = rest.trim().to_string();
        } else {
            let at = text.to_ascii_lowercase().find(" sent you")?;
            title = text[..at].trim().to_string();
            text = String::new();
        }
    }
    let lower = text.to_ascii_lowercase();
    let says_dm = lower.contains("sent you") || lower.contains("sent a message");
    if !says_dm && NOT_DMS.iter().any(|p| lower.contains(p) || title.to_ascii_lowercase().contains(p)) {
        return None;
    }
    // "sent you a message" says nothing more than that a message came.
    if lower.starts_with("sent you") || lower.contains("sent you a message") {
        text = String::new();
    }
    if title.is_empty() {
        return None;
    }
    // "Sam Kay (sam.k)": the username is the bracketed part.
    let username = title
        .rsplit_once(" (")
        .and_then(|(_, u)| u.strip_suffix(')'))
        .and_then(username)
        .or_else(|| username(&title));
    Some(Dm { id, sender: cut(&title, 60), username, preview: cut(&text, PREVIEW_CHARS), source, app_id: app_id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dm(app: &str, app_id: &str, title: &str, body: &str) -> Option<(String, Option<String>, String, &'static str)> {
        from_notification(1, app, app_id, title, body).map(|d| (d.sender, d.username, d.preview, d.source))
    }

    #[test]
    fn fake_notifications_become_dms_or_are_skipped() {
        let app = "Facebook.InstagramBeta_8xx8rvfyw5nnt!App";
        // The Instagram app: the sender is the title.
        assert_eq!(dm("Instagram", app, "sam.k", "on my way"), Some(("sam.k".into(), Some("sam.k".into()), "on my way".into(), "app")));
        // An installed Edge web app named Instagram.
        assert_eq!(dm("Instagram", "MSEdge._crx_abc", "Sam Kay (sam.k)", "are you coming?").map(|d| d.1), Some(Some("sam.k".into())));
        // A Chrome tab: "Instagram" as the title, "name: text", the site as the last line.
        assert_eq!(
            dm("Google Chrome", "Chrome", "Instagram", "mia_r: see you at 6 · www.instagram.com"),
            Some(("mia_r".into(), Some("mia_r".into()), "see you at 6".into(), "web"))
        );
        // Edge, the sender as the title and the site after the text.
        assert_eq!(dm("Microsoft Edge", "MSEdge", "jo", "lol yes · www.instagram.com"), Some(("jo".into(), Some("jo".into()), "lol yes".into(), "web")));
        // "sent you a message": who, with no text.
        assert_eq!(dm("Instagram", app, "Instagram", "dan.p sent you a message."), Some(("dan.p".into(), Some("dan.p".into()), String::new(), "app")));
        assert_eq!(dm("Instagram", app, "dan.p", "sent you a reel").map(|d| d.2), Some(String::new()));
        // Not DMs.
        assert_eq!(dm("Instagram", app, "Instagram", "sam.k liked your photo."), None);
        assert_eq!(dm("Instagram", app, "mia_r", "started following you."), None);
        assert_eq!(dm("Microsoft Edge", "MSEdge", "Instagram", "jo commented: nice · www.instagram.com"), None);
        assert_eq!(dm("Instagram", app, "Instagram", "jo is live now"), None);
        // Not Instagram at all: mail stays mail.
        assert_eq!(dm("Outlook", "Microsoft.Office.OUTLOOK.EXE.15", "Sam Kay", "Lunch tomorrow?"), None);
        assert_eq!(dm("Google Chrome", "Chrome", "Discord", "sam: hi · discord.com"), None);
        // Nothing to show.
        assert_eq!(dm("Instagram", app, "", ""), None);
        assert_eq!(dm("Instagram", app, "Instagram", "You have new activity"), None);
    }

    #[test]
    fn a_long_message_is_cut_and_a_display_name_is_no_username() {
        let long = "word ".repeat(60);
        let d = from_notification(2, "Instagram", "x!App", "Sam Kay", &long).unwrap();
        assert_eq!(d.preview.chars().count(), PREVIEW_CHARS);
        assert!(d.preview.ends_with('…'));
        assert_eq!(d.username, None, "a name with a space isn't a username");
    }

    #[test]
    fn usernames_are_only_what_instagram_allows() {
        assert_eq!(username("@Sam.K_92"), Some("sam.k_92".into()));
        assert_eq!(username("sam kay"), None);
        assert_eq!(username("sam/../x"), None);
        assert_eq!(username(".sam"), None);
        assert_eq!(username(&"a".repeat(31)), None);
        assert_eq!(username(""), None);
    }
}
