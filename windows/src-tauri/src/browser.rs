// The browser Groovy uses (round 4 item 4). Every "open a website" and
// "search for…" — from « OK Groovy », the chat, the tools or the AI — goes
// through here, to the browser chosen in Settings → Browser (Windows' default
// unless another is picked) and the search engine chosen there (DuckDuckGo
// by default). Nothing here is hard-wired to Chrome.
//
// DuckDuckGo for Windows is a Microsoft Store app: it is started through its
// AppUserModelID with the address as its argument. It has no extension or
// automation interface, so inside any browser Groovy uses what Windows offers:
// the browser's keyboard shortcuts, and UI Automation to read the tab's title
// and address and to press a visible button or link by its name.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Serialize;

use crate::i18n::{t, tf};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    /// "duckduckgo", "edge", "chrome", "brave", "firefox", "opera".
    pub id: &'static str,
    pub name: &'static str,
    #[serde(skip)]
    pub launch: Launch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Launch {
    Exe(PathBuf),
    /// A Microsoft Store app, by its AppUserModelID.
    App(String),
}

/// (id, name, program file under Program Files, Program Files (x86) or %LOCALAPPDATA%).
const KNOWN: &[(&str, &str, &[&str])] = &[
    ("edge", "Microsoft Edge", &["Microsoft\\Edge\\Application\\msedge.exe"]),
    ("chrome", "Google Chrome", &["Google\\Chrome\\Application\\chrome.exe"]),
    ("brave", "Brave", &["BraveSoftware\\Brave-Browser\\Application\\brave.exe"]),
    ("firefox", "Firefox", &["Mozilla Firefox\\firefox.exe"]),
    ("opera", "Opera", &["Programs\\Opera\\opera.exe", "Opera\\opera.exe"]),
];

/// The program names of browser windows, to know a browser is in front.
const PROGRAMS: &[&str] = &["msedge", "chrome", "brave", "firefox", "opera", "duckduckgo"];

/// Search engines: (id, name, address the words are added to).
pub const ENGINES: &[(&str, &str, &str)] = &[
    ("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q="),
    ("google", "Google", "https://www.google.com/search?q="),
    ("bing", "Bing", "https://www.bing.com/search?q="),
    ("brave", "Brave Search", "https://search.brave.com/search?q="),
    ("ecosia", "Ecosia", "https://www.ecosia.org/search?q="),
    ("startpage", "Startpage", "https://www.startpage.com/do/search?q="),
];

/// The browsers on this PC, found once per run.
pub fn installed() -> &'static [Browser] {
    static FOUND: OnceLock<Vec<Browser>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let mut v = Vec::new();
        if let Some(aumid) = os::duckduckgo_aumid() {
            v.push(Browser { id: "duckduckgo", name: "DuckDuckGo", launch: Launch::App(aumid) });
        }
        let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
            .collect();
        for (id, name, paths) in KNOWN {
            let exe = roots.iter().flat_map(|r| paths.iter().map(move |p| r.join(p))).find(|p| p.is_file());
            if let Some(exe) = exe {
                v.push(Browser { id, name, launch: Launch::Exe(exe) });
            }
        }
        v
    })
}

/// Windows' default browser, from the https handler's ProgId.
pub fn default_id() -> Option<&'static str> {
    id_of_prog_id(&os::https_prog_id()?)
}

pub fn id_of_prog_id(prog_id: &str) -> Option<&'static str> {
    let p = prog_id.to_ascii_lowercase();
    Some(if p.starts_with("msedgehtm") {
        "edge"
    } else if p.starts_with("chromehtml") {
        "chrome"
    } else if p.starts_with("bravehtml") {
        "brave"
    } else if p.starts_with("firefoxurl") {
        "firefox"
    } else if p.starts_with("opera") {
        "opera"
    } else if p.contains("duckduckgo") {
        "duckduckgo"
    } else {
        return None;
    })
}

/// Settings → Browser: (browser, search engine), set at start and on every save.
static CHOICE: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);

pub fn configure(browser: &str, engine: &str) {
    *CHOICE.lock().unwrap() = Some((browser.to_string(), engine.to_string()));
}

fn settings() -> (String, String) {
    CHOICE.lock().unwrap().clone().unwrap_or_else(|| ("default".into(), "duckduckgo".into()))
}

/// The browser Groovy opens pages in: the chosen one if it is installed, else Windows' default.
pub fn chosen() -> Option<&'static Browser> {
    let (choice, _) = settings();
    installed().iter().find(|b| b.id == choice)
}

/// The search address for `query` with the chosen engine.
pub fn search_url(engine: &str, query: &str) -> String {
    let base = ENGINES.iter().find(|e| e.0 == engine).unwrap_or(&ENGINES[0]).2;
    let encoded: String = query
        .trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{base}{encoded}")
}

/// Only web pages go to the browser.
fn is_web(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

/// Opens a web page in the chosen browser.
pub fn open(url: &str) -> Result<String, String> {
    if !is_web(url) {
        return Err(t("Only web pages (http or https) open in the browser."));
    }
    match chosen() {
        Some(b) => {
            os::launch(&b.launch, url)?;
            Ok(b.name.to_string())
        }
        None => {
            crate::platform::open_url(url);
            Ok(t("your browser"))
        }
    }
}

/// Searches the web for `query` with the chosen engine, in the chosen browser.
pub fn search(query: &str) -> Result<String, String> {
    let (_, engine) = settings();
    let name = open(&search_url(&engine, query))?;
    Ok(tf("Searching for “{query}” in {browser}.", &[("query", query.trim()), ("browser", &name)]))
}

/// What can be done in the browser in front.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    NewTab,
    CloseTab,
    Back,
    Forward,
    Reload,
    NextTab,
    PreviousTab,
    /// The tab's title and address.
    Page,
    /// Press a visible button or link by its name.
    Press(String),
}

impl Action {
    pub fn parse(t: &str) -> Option<Action> {
        let t = t.trim().trim_end_matches(['.', '!', '?']).to_ascii_lowercase();
        let t = t.as_str();
        let one = |phrases: &[&str]| phrases.contains(&t);
        if one(&["new tab", "open a new tab", "open new tab"]) {
            return Some(Action::NewTab);
        }
        if one(&["close tab", "close this tab", "close the tab"]) {
            return Some(Action::CloseTab);
        }
        if one(&["go back", "go back a page", "back a page", "previous page"]) {
            return Some(Action::Back);
        }
        if one(&["go forward", "forward a page", "next page"]) {
            return Some(Action::Forward);
        }
        if one(&["reload", "refresh", "reload the page", "refresh the page", "reload this page", "refresh this page"]) {
            return Some(Action::Reload);
        }
        if one(&["next tab", "switch tab", "go to the next tab"]) {
            return Some(Action::NextTab);
        }
        if one(&["previous tab", "last tab", "go to the previous tab"]) {
            return Some(Action::PreviousTab);
        }
        if one(&["what page is this", "what's this page", "whats this page", "what site is this", "what website is this", "read the address", "what's the address", "whats the address"]) {
            return Some(Action::Page);
        }
        for lead in ["click on the ", "click the ", "click on ", "click ", "press the ", "press "] {
            if let Some(rest) = t.strip_prefix(lead) {
                let name = rest.trim_end_matches(" button").trim_end_matches(" link").trim();
                if !name.is_empty() && name.split_whitespace().count() <= 6 {
                    return Some(Action::Press(name.to_string()));
                }
            }
        }
        None
    }
}

// Windows' virtual keys.
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_SHIFT: u16 = 0x10;
const VK_TAB: u16 = 0x09;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_F5: u16 = 0x74;
const VK_T: u16 = 0x54;
const VK_W: u16 = 0x57;

/// Does it in the browser in front, and says what happened.
pub fn act(action: &Action) -> Result<String, String> {
    let front = crate::flow::os::foreground_app().map(|(id, _)| id.to_ascii_lowercase());
    if !front.as_deref().is_some_and(|id| PROGRAMS.iter().any(|p| id.contains(p))) {
        return Err(t("Bring the browser to the front first, then ask again."));
    }
    let keys = crate::flow::os::send_chord;
    match action {
        Action::NewTab => keys(&[VK_CONTROL], VK_T).map(|_| t("New tab.")),
        Action::CloseTab => keys(&[VK_CONTROL], VK_W).map(|_| t("Tab closed.")),
        Action::Back => keys(&[VK_MENU], VK_LEFT).map(|_| t("Back.")),
        Action::Forward => keys(&[VK_MENU], VK_RIGHT).map(|_| t("Forward.")),
        Action::Reload => keys(&[], VK_F5).map(|_| t("Reloaded.")),
        Action::NextTab => keys(&[VK_CONTROL], VK_TAB).map(|_| t("Next tab.")),
        Action::PreviousTab => keys(&[VK_CONTROL, VK_SHIFT], VK_TAB).map(|_| t("Previous tab.")),
        Action::Page => {
            let (title, address) = os::page().ok_or_else(|| t("I couldn't read the page."))?;
            Ok(match address {
                Some(a) => tf("“{title}” — {address}", &[("title", &title), ("address", &a)]),
                None => tf("“{title}”", &[("title", &title)]),
            })
        }
        Action::Press(name) => {
            if os::press(name) {
                Ok(tf("Pressed “{name}”.", &[("name", name)]))
            } else {
                Err(tf("I couldn't find “{name}” on the page.", &[("name", name)]))
            }
        }
    }
}

/// Settings → Browser: what is installed, which is Windows' default, the engines.
#[tauri::command]
pub fn browsers() -> serde_json::Value {
    serde_json::json!({
        "installed": installed(),
        "default": default_id(),
        "engines": ENGINES.iter().map(|(id, name, _)| serde_json::json!({ "id": id, "name": name })).collect::<Vec<_>>(),
    })
}

#[cfg(windows)]
mod os {
    use super::Launch;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// DuckDuckGo's AppUserModelID, from the Start menu's apps (Get-StartApps).
    /// A fixed command with no user text in it.
    pub fn duckduckgo_aumid() -> Option<String> {
        let out = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-StartApps | Where-Object { $_.Name -like 'DuckDuckGo*' } | Select-Object -First 1 -ExpandProperty AppID",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (id.contains('!') && !id.contains(char::is_whitespace)).then_some(id)
    }

    pub fn https_prog_id() -> Option<String> {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
        let mut buf = [0u16; 128];
        let mut size = (buf.len() * 2) as u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\https\\UserChoice"),
                w!("ProgId"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                Some(&mut size),
            )
        };
        r.is_ok().then(|| String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1).min(buf.len())]))
    }

    /// The address is an argument, never through a shell.
    pub fn launch(launch: &Launch, url: &str) -> Result<(), String> {
        match launch {
            Launch::Exe(exe) => Command::new(exe).arg(url).spawn().map(|_| ()).map_err(|e| e.to_string()),
            Launch::App(aumid) => {
                activate(aumid, url)?;
                // A Store app may come up on its last page instead of the
                // address: once it is in front, the address is typed into its
                // bar (Ctrl+L, the address, Enter).
                let url = url.to_string();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let front = crate::flow::os::foreground_app().map(|(id, _)| id.to_ascii_lowercase()).unwrap_or_default();
                    let host = reqwest::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
                    let there = page().and_then(|(_, a)| a).is_some_and(|a| a.contains(&host));
                    if front.contains("duckduckgo") && !there {
                        let _ = crate::flow::os::send_chord(&[0x11], 0x4C);
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        let _ = crate::flow::os::send_text(&url);
                        let _ = crate::flow::os::send_keys(&[0x0D]);
                    }
                });
                Ok(())
            }
        }
    }

    /// Starts a Store app with arguments (IApplicationActivationManager).
    fn activate(aumid: &str, args: &str) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED};
        use windows::Win32::UI::Shell::{ApplicationActivationManager, IApplicationActivationManager, AO_NONE};
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let manager: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER).map_err(|e| e.to_string())?;
            manager.ActivateApplication(&HSTRING::from(aumid), &HSTRING::from(args), AO_NONE).map(|_| ()).map_err(|e| e.to_string())
        }
    }

    fn uia() -> Option<windows::Win32::UI::Accessibility::IUIAutomation> {
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
        use windows::Win32::UI::Accessibility::CUIAutomation;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
        }
    }

    /// The front window's title and, when there is one, its address bar's text.
    pub fn page() -> Option<(String, Option<String>)> {
        use windows::Win32::UI::Accessibility::{IUIAutomationValuePattern, TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId};
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        unsafe {
            let hwnd = GetForegroundWindow();
            let uia = uia()?;
            let window = uia.ElementFromHandle(hwnd).ok()?;
            let title = window.CurrentName().ok()?.to_string();
            let edit = uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &windows::Win32::System::Variant::VARIANT::from(UIA_EditControlTypeId.0)).ok()?;
            let address = window
                .FindFirst(TreeScope_Descendants, &edit)
                .ok()
                .and_then(|e| e.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok())
                .and_then(|v| v.CurrentValue().ok())
                .map(|v| v.to_string())
                .filter(|v| !v.trim().is_empty());
            Some((title, address))
        }
    }

    /// Presses the first visible button or link named `name` (any case).
    pub fn press(name: &str) -> bool {
        use windows::Win32::System::Variant::VARIANT;
        use windows::Win32::UI::Accessibility::{
            PropertyConditionFlags_IgnoreCase, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId, UIA_HyperlinkControlTypeId,
            UIA_IsOffscreenPropertyId, UIA_MenuItemControlTypeId, UIA_NamePropertyId, UIA_TabItemControlTypeId,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        unsafe {
            let Some(uia) = uia() else { return false };
            let Ok(window) = uia.ElementFromHandle(GetForegroundWindow()) else { return false };
            let Ok(named) = uia.CreatePropertyConditionEx(UIA_NamePropertyId, &VARIANT::from(name), PropertyConditionFlags_IgnoreCase) else {
                return false;
            };
            let Ok(visible) = uia.CreatePropertyCondition(UIA_IsOffscreenPropertyId, &VARIANT::from(false)) else { return false };
            let kinds: Vec<_> = [UIA_ButtonControlTypeId, UIA_HyperlinkControlTypeId, UIA_MenuItemControlTypeId, UIA_TabItemControlTypeId]
                .iter()
                .filter_map(|k| uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(k.0)).ok())
                .collect();
            press_with(&uia, &window, &named, &visible, &kinds)
        }
    }

    unsafe fn press_with(
        uia: &windows::Win32::UI::Accessibility::IUIAutomation,
        window: &windows::Win32::UI::Accessibility::IUIAutomationElement,
        named: &windows::Win32::UI::Accessibility::IUIAutomationCondition,
        visible: &windows::Win32::UI::Accessibility::IUIAutomationCondition,
        kinds: &[windows::Win32::UI::Accessibility::IUIAutomationCondition],
    ) -> bool {
        use windows::Win32::UI::Accessibility::{
            IUIAutomationInvokePattern, IUIAutomationLegacyIAccessiblePattern, TreeScope_Descendants, UIA_InvokePatternId, UIA_LegacyIAccessiblePatternId,
        };
        for kind in kinds {
            let Ok(both) = uia.CreateAndCondition(named, visible) else { continue };
            let Ok(cond) = uia.CreateAndCondition(&both, kind) else { continue };
            let Ok(element) = window.FindFirst(TreeScope_Descendants, &cond) else { continue };
            if let Ok(invoke) = element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
                if invoke.Invoke().is_ok() {
                    return true;
                }
            }
            if let Ok(legacy) = element.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(UIA_LegacyIAccessiblePatternId) {
                if legacy.DoDefaultAction().is_ok() {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(not(windows))]
mod os {
    use super::Launch;

    pub fn duckduckgo_aumid() -> Option<String> {
        None
    }
    pub fn https_prog_id() -> Option<String> {
        None
    }
    pub fn launch(launch: &Launch, url: &str) -> Result<(), String> {
        match launch {
            Launch::Exe(exe) => std::process::Command::new(exe).arg(url).spawn().map(|_| ()).map_err(|e| e.to_string()),
            Launch::App(_) => Err("not on this system".into()),
        }
    }
    pub fn page() -> Option<(String, Option<String>)> {
        None
    }
    pub fn press(_name: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duckduckgo_is_the_default_engine_and_words_are_encoded() {
        assert_eq!(search_url("duckduckgo", "best pizza near me"), "https://duckduckgo.com/?q=best+pizza+near+me");
        assert_eq!(search_url("google", "c++ & rust"), "https://www.google.com/search?q=c%2B%2B+%26+rust");
        assert_eq!(search_url("unknown", "x"), "https://duckduckgo.com/?q=x");
        assert_eq!(search_url("bing", "café"), "https://www.bing.com/search?q=caf%C3%A9");
    }

    #[test]
    fn windows_prog_ids_name_the_browser() {
        assert_eq!(id_of_prog_id("MSEdgeHTM"), Some("edge"));
        assert_eq!(id_of_prog_id("ChromeHTML"), Some("chrome"));
        assert_eq!(id_of_prog_id("BraveHTML"), Some("brave"));
        assert_eq!(id_of_prog_id("FirefoxURL-308046B0AF4A39CB"), Some("firefox"));
        assert_eq!(id_of_prog_id("OperaStable"), Some("opera"));
        assert_eq!(id_of_prog_id("AppX9x7duckduckgo.html"), Some("duckduckgo"));
        assert_eq!(id_of_prog_id("SomethingElse"), None);
    }

    #[test]
    fn only_web_pages_open() {
        assert!(open("file:///C:/Windows/system32/calc.exe").is_err());
        assert!(open("javascript:alert(1)").is_err());
    }

    #[test]
    fn browser_commands_are_understood() {
        assert_eq!(Action::parse("New tab."), Some(Action::NewTab));
        assert_eq!(Action::parse("go back"), Some(Action::Back));
        assert_eq!(Action::parse("Refresh the page"), Some(Action::Reload));
        assert_eq!(Action::parse("next tab"), Some(Action::NextTab));
        assert_eq!(Action::parse("What page is this?"), Some(Action::Page));
        assert_eq!(Action::parse("click the sign in button"), Some(Action::Press("sign in".into())));
        assert_eq!(Action::parse("press Accept"), Some(Action::Press("accept".into())));
        assert_eq!(Action::parse("go back to sleep and dream about the beach today please"), None);
        assert_eq!(Action::parse("what time is it"), None);
    }
}
