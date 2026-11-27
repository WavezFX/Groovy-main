// Round 7 item 2: opening Instagram where the message goes, through what the
// user already has (the Instagram app, or instagram.com in their browser), and
// leaving the draft ready. Only Instagram's public web addresses; Groovy never
// presses Send.

use std::time::Duration;

/// Where to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Straight to the conversation with this username.
    Chat(String),
    /// The new-message search.
    NewMessage,
    /// The DMs list.
    Inbox,
}

/// The web address for a place. A username only ever goes in as one
/// (letters, digits, dots, underscores), so nothing else reaches the link.
pub fn link(place: &Place) -> String {
    match place {
        Place::Chat(u) => match super::notes::username(u) {
            Some(u) => format!("https://ig.me/m/{u}"),
            None => "https://www.instagram.com/direct/new/".into(),
        },
        Place::NewMessage => "https://www.instagram.com/direct/new/".into(),
        Place::Inbox => "https://www.instagram.com/direct/inbox/".into(),
    }
}

/// An AppUserModelID Explorer can open ("Facebook.InstagramBeta_8xx…!App").
pub fn app_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() < 200 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '!' | '-'))
}

/// What opens it: "app" (the Instagram app, by the id Windows gave its
/// notification), a browser id ("edge", "chrome"), or "" for the default.
pub fn how(choice: &str, app_id: Option<&str>) -> String {
    match choice {
        "app" | "auto" if app_id.is_some_and(app_id_ok) => "app".into(),
        "edge" | "chrome" => choice.into(),
        _ => String::new(),
    }
}

/// Opens the place. Ok(what it opened in).
pub fn open(choice: &str, app_id: Option<&str>, place: &Place) -> Result<String, String> {
    let url = link(place);
    match how(choice, app_id).as_str() {
        "app" => {
            // The app opens on its own DMs; the draft waits on the clipboard.
            #[cfg(windows)]
            crate::platform::reveal_folder(&format!("shell:AppsFolder\\{}", app_id.unwrap_or_default()));
            Ok("Instagram".into())
        }
        "" => crate::browser::open(&url),
        browser => crate::browser::open_in(browser, &url),
    }
}

/// With "Type for me" on: waits until Instagram's window is in front (at most
/// `wait`), then types `text` there like a keyboard. Never Enter. False when
/// Instagram never came to the front, so nothing was typed.
pub fn type_when_ready(text: &str, wait: Duration) -> bool {
    let until = std::time::Instant::now() + wait;
    while std::time::Instant::now() < until {
        if crate::flow::os::foreground_title().is_some_and(|t| t.to_ascii_lowercase().contains("instagram")) {
            // The page needs a moment after the window shows to focus its box.
            std::thread::sleep(Duration::from_millis(1500));
            let still = crate::flow::os::foreground_title().is_some_and(|t| t.to_ascii_lowercase().contains("instagram"));
            return still && crate::flow::os::send_text(text.trim_end_matches(['\r', '\n'])).is_ok();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_instagram_addresses_with_a_username_only() {
        assert_eq!(link(&Place::Chat("Sam.K_92".into())), "https://ig.me/m/sam.k_92");
        assert_eq!(link(&Place::Chat("@mia_r".into())), "https://ig.me/m/mia_r");
        // A display name, or anything odd, opens the search instead.
        assert_eq!(link(&Place::Chat("Sam Kay".into())), "https://www.instagram.com/direct/new/");
        assert_eq!(link(&Place::Chat("x/../../evil?a=1".into())), "https://www.instagram.com/direct/new/");
        assert_eq!(link(&Place::Inbox), "https://www.instagram.com/direct/inbox/");
    }

    #[test]
    fn the_app_opens_only_by_a_real_app_id() {
        assert_eq!(how("auto", Some("Facebook.InstagramBeta_8xx8rvfyw5nnt!App")), "app");
        assert_eq!(how("app", None), "", "no app seen yet: the browser");
        assert_eq!(how("app", Some("x; del *")), "");
        assert_eq!(how("auto", None), "");
        assert_eq!(how("edge", Some("Facebook.InstagramBeta!App")), "edge");
        assert_eq!(how("chrome", None), "chrome");
        assert_eq!(how("firefox", None), "", "only the choices Settings offers");
    }
}
