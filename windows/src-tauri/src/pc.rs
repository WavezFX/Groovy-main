// More control of the PC (round 4 item 5). Each action is a tool (tools.rs):
// the same permission (allow / ask / deny), the same audit log, and both the
// AI and the no-AI command parser can ask for it. Anything that closes an app,
// changes a system setting, sets the clipboard or types for you asks first by
// default. Groovy acts only on the user's own words, never on instructions it
// reads on screen or in a web page.
//
// Windows only: what Windows offers to apps (windows and their shortcuts, the
// audio mixer, the radios API, the registry's theme, ms-settings: links). Where
// Windows has no public switch (the default output device, Do Not Disturb,
// Night light), the right Settings page opens and Groovy says so.

use crate::i18n::{t, tf};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SwitchTo(String),
    Close(String),
    Window { app: Option<String>, how: WindowHow },
    /// The master volume, 0–100.
    Volume(u8),
    AppVolume { app: String, change: Change },
    OutputDevice,
    Discord(Discord),
    SpotifyPlay(String),
    /// The most recent file, or the most recent one whose name has these words.
    OpenRecent(Option<String>),
    Theme { dark: bool },
    DoNotDisturb,
    NightLight,
    Radio { wifi: bool, on: bool },
    Brightness(Change),
    SetClipboard(String),
    Type(String),
    MemoryUse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowHow {
    Minimise,
    Maximise,
    SnapLeft,
    SnapRight,
    OtherMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    Up,
    Down,
    Mute,
    Unmute,
    Set(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Discord {
    Open,
    Mute,
    Deafen,
    /// A discord:// link to a server, channel or DM.
    Link(String),
}

impl Action {
    /// The tool (and so the permission) this action belongs to.
    pub fn tool(&self) -> &'static str {
        match self {
            Action::SwitchTo(_) | Action::Window { .. } => "windows",
            Action::Close(_) => "close_app",
            Action::Volume(_) | Action::AppVolume { .. } | Action::OutputDevice => "sound",
            Action::Discord(_) => "discord",
            Action::SpotifyPlay(_) => "spotify_play",
            Action::OpenRecent(_) => "recent_files",
            Action::Theme { .. } | Action::DoNotDisturb | Action::NightLight | Action::Radio { .. } | Action::Brightness(_) => "system_settings",
            Action::SetClipboard(_) => "set_clipboard",
            Action::Type(_) => "type_text",
            Action::MemoryUse => "system_status",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Action::SwitchTo(a) => tf("Switch to {app}?", &[("app", a)]),
            Action::Close(a) => tf("Close {app}? Anything not saved in it may be lost.", &[("app", a)]),
            Action::Window { .. } => t("Move or resize the window?"),
            Action::Volume(v) => tf("Set the volume to {n}%?", &[("n", &v.to_string())]),
            Action::AppVolume { app, .. } => tf("Change {app}'s volume?", &[("app", app)]),
            Action::OutputDevice => t("Open the sound output settings?"),
            Action::Discord(_) => t("Use Discord?"),
            Action::SpotifyPlay(q) => tf("Play “{query}” on Spotify?", &[("query", q)]),
            Action::OpenRecent(_) => t("Open a recent file?"),
            Action::Theme { dark: true } => t("Switch Windows to dark mode?"),
            Action::Theme { dark: false } => t("Switch Windows to light mode?"),
            Action::DoNotDisturb => t("Open Do not disturb in Settings?"),
            Action::NightLight => t("Open Night light in Settings?"),
            Action::Radio { wifi: true, on } => if *on { t("Turn Wi-Fi on?") } else { t("Turn Wi-Fi off?") },
            Action::Radio { wifi: false, on } => if *on { t("Turn Bluetooth on?") } else { t("Turn Bluetooth off?") },
            Action::Brightness(_) => t("Change the screen brightness?"),
            Action::SetClipboard(_) => t("Put this text on your clipboard?"),
            Action::Type(text) => tf("Type “{text}” where the cursor is?", &[("text", text)]),
            Action::MemoryUse => t("Read what's using memory?"),
        }
    }
}

fn number(s: &str) -> Option<u8> {
    let n: u32 = s.trim().trim_end_matches('%').trim_end_matches(" percent").trim().parse().ok()?;
    (n <= 100).then_some(n as u8)
}

/// What the user said, without "please"… (`t` lower case, `cased` as typed, same offsets).
pub fn parse(t: &str, cased: &str) -> Option<Action> {
    let typed = |part: &str| -> String {
        let start = part.as_ptr() as usize - t.as_ptr() as usize;
        cased.get(start..start + part.len()).unwrap_or(part).trim().to_string()
    };
    let one = |phrases: &[&str]| phrases.contains(&t);

    // Windows settings.
    if one(&["dark mode", "turn on dark mode", "switch to dark mode", "use dark mode", "make it dark"]) {
        return Some(Action::Theme { dark: true });
    }
    if one(&["light mode", "turn on light mode", "switch to light mode", "use light mode", "turn off dark mode"]) {
        return Some(Action::Theme { dark: false });
    }
    if one(&["do not disturb", "turn on do not disturb", "turn off do not disturb", "focus mode", "turn on focus", "don't disturb me"]) {
        return Some(Action::DoNotDisturb);
    }
    if one(&["night light", "turn on night light", "turn off night light", "turn on the night light", "turn off the night light"]) {
        return Some(Action::NightLight);
    }
    for (words, wifi) in [("wifi", true), ("wi-fi", true), ("wi fi", true), ("bluetooth", false)] {
        for (phrase, on) in [(format!("turn on {words}"), true), (format!("turn on the {words}"), true), (format!("{words} on"), true), (format!("turn {words} on"), true),
            (format!("turn off {words}"), false), (format!("turn off the {words}"), false), (format!("{words} off"), false), (format!("turn {words} off"), false)]
        {
            if t == phrase {
                return Some(Action::Radio { wifi, on });
            }
        }
    }
    if let Some(rest) = t.strip_prefix("set the brightness to ").or_else(|| t.strip_prefix("set brightness to ")).or_else(|| t.strip_prefix("brightness ")) {
        return number(rest).map(|n| Action::Brightness(Change::Set(n)));
    }
    if one(&["brighter", "turn the brightness up", "turn up the brightness", "brightness up", "make the screen brighter"]) {
        return Some(Action::Brightness(Change::Up));
    }
    if one(&["dimmer", "turn the brightness down", "turn down the brightness", "brightness down", "make the screen darker", "dim the screen"]) {
        return Some(Action::Brightness(Change::Down));
    }

    // Sound.
    if let Some(rest) = t.strip_prefix("set the volume to ").or_else(|| t.strip_prefix("set volume to ")).or_else(|| t.strip_prefix("volume to ")) {
        return number(rest).map(Action::Volume);
    }
    if one(&["change the sound output", "change the audio output", "switch the audio output", "pick the output device", "change the output device", "switch audio output", "change speakers"]) {
        return Some(Action::OutputDevice);
    }

    // Discord.
    if one(&["mute me on discord", "mute my mic on discord", "mute discord mic", "discord mute", "unmute me on discord", "unmute my mic on discord", "discord unmute"]) {
        return Some(Action::Discord(Discord::Mute));
    }
    if one(&["deafen me on discord", "deafen on discord", "discord deafen", "deafen me", "undeafen me", "undeafen me on discord"]) {
        return Some(Action::Discord(Discord::Deafen));
    }
    if one(&["open discord", "launch discord", "start discord"]) {
        return Some(Action::Discord(Discord::Open));
    }
    if t.starts_with("open discord://") {
        return Some(Action::Discord(Discord::Link(typed(&t[5..]))));
    }

    // Per-app volume: "turn Discord down", "mute Spotify", "set Chrome volume to 30".
    for (lead, tail, change) in [("turn ", " down", Change::Down), ("turn ", " up", Change::Up), ("mute ", "", Change::Mute), ("unmute ", "", Change::Unmute)] {
        if let Some(app) = t.strip_prefix(lead).and_then(|r| r.strip_suffix(tail).or(if tail.is_empty() { Some(r) } else { None })) {
            let app = app.trim();
            let plain = ["it", "the volume", "the music", "the sound", "volume", "sound", "me", "my mic", "the mic", "this", "everything"];
            if !app.is_empty() && !plain.contains(&app) && app.split_whitespace().count() <= 3 && !app.contains("discord mic") {
                return Some(Action::AppVolume { app: typed(app), change });
            }
        }
    }
    if let Some(rest) = t.strip_prefix("set ") {
        if let Some((app, n)) = rest.split_once(" volume to ") {
            if let Some(n) = number(n) {
                return Some(Action::AppVolume { app: typed(app.trim()), change: Change::Set(n) });
            }
        }
    }

    // Spotify: "play X on Spotify", and "play X" for anything but the plain music commands.
    if let Some(rest) = t.strip_prefix("play ") {
        let what = rest.strip_suffix(" on spotify").unwrap_or(rest).trim();
        if !what.is_empty() && what.split_whitespace().count() <= 10 {
            return Some(Action::SpotifyPlay(typed(what)));
        }
    }

    // Files.
    if one(&["open my last file", "open the last file", "open my recent file", "open the file i was working on", "open my most recent file"]) {
        return Some(Action::OpenRecent(None));
    }
    if let Some(name) = t.strip_prefix("open my recent ").or_else(|| t.strip_prefix("open recent ")).or_else(|| t.strip_prefix("open the recent ")) {
        let name = name.trim_end_matches(" file").trim();
        if !name.is_empty() {
            return Some(Action::OpenRecent(Some(typed(name))));
        }
    }

    // Clipboard and typing.
    if let Some(rest) = t.strip_prefix("copy ") {
        for tail in [" to the clipboard", " to my clipboard", " to clipboard"] {
            if let Some(text) = rest.strip_suffix(tail) {
                return Some(Action::SetClipboard(typed(text)));
            }
        }
    }
    if let Some(text) = t.strip_prefix("type ") {
        if !text.trim().is_empty() {
            return Some(Action::Type(typed(text)));
        }
    }
    if one(&["what's using my memory", "whats using my memory", "what's using memory", "what is using my memory", "what's using my ram", "what's using ram", "what's eating my memory"]) {
        return Some(Action::MemoryUse);
    }

    // Windows and apps.
    if let Some(app) = t.strip_prefix("switch to ") {
        return (!app.trim().is_empty()).then(|| Action::SwitchTo(typed(app.trim())));
    }
    if let Some(app) = t.strip_prefix("close ").or_else(|| t.strip_prefix("quit ")).or_else(|| t.strip_prefix("exit ")) {
        let app = app.trim();
        if !app.is_empty() && !["tab", "this tab", "the tab", "the window", "this window", "it"].contains(&app) && app.split_whitespace().count() <= 3 {
            return Some(Action::Close(typed(app)));
        }
    }
    let windowed = |lead: &str, how: WindowHow| -> Option<Action> {
        let rest = t.strip_prefix(lead)?;
        let rest = rest.trim();
        let app = match rest {
            "" | "this" | "it" | "the window" | "this window" => None,
            other => Some(typed(other)),
        };
        Some(Action::Window { app, how })
    };
    for (lead, how) in [("minimise", WindowHow::Minimise), ("minimize", WindowHow::Minimise), ("maximise", WindowHow::Maximise), ("maximize", WindowHow::Maximise)] {
        if t == lead || t.starts_with(&format!("{lead} ")) {
            return windowed(lead, how);
        }
    }
    for (side, how) in [(" to the left", WindowHow::SnapLeft), (" to the right", WindowHow::SnapRight), (" left", WindowHow::SnapLeft), (" right", WindowHow::SnapRight)] {
        if let Some(rest) = t.strip_prefix("snap").and_then(|r| r.strip_suffix(side)) {
            let app = rest.trim();
            return Some(Action::Window { app: (!["", "this", "it", "the window", "this window"].contains(&app)).then(|| typed(app)), how });
        }
    }
    for tail in [" to the other monitor", " to the other screen", " to the other display", " to my other screen", " to my other monitor"] {
        if let Some(rest) = t.strip_prefix("move").and_then(|r| r.strip_suffix(tail)) {
            let app = rest.trim();
            return Some(Action::Window { app: (!["", "this", "it", "the window", "this window"].contains(&app)).then(|| typed(app)), how: WindowHow::OtherMonitor });
        }
    }
    None
}

/// Does it, and says what happened.
pub fn run(action: &Action) -> Result<String, String> {
    match action {
        Action::SwitchTo(app) => os::switch_to(app).then(|| tf("Switched to {app}.", &[("app", app)])).ok_or_else(|| not_open(app)),
        Action::Close(app) => {
            let n = os::close(app);
            if n > 0 {
                Ok(tf("Asked {app} to close.", &[("app", app)]))
            } else {
                Err(not_open(app))
            }
        }
        Action::Window { app, how } => {
            if let Some(a) = app {
                if !os::switch_to(a) {
                    return Err(not_open(a));
                }
            }
            os::window(*how)?;
            Ok(match how {
                WindowHow::Minimise => t("Minimised."),
                WindowHow::Maximise => t("Maximised."),
                WindowHow::SnapLeft => t("Snapped to the left."),
                WindowHow::SnapRight => t("Snapped to the right."),
                WindowHow::OtherMonitor => t("Moved to the other screen."),
            })
        }
        Action::Volume(n) => os::master_volume(*n).map(|_| tf("Volume set to {n}%.", &[("n", &n.to_string())])),
        Action::AppVolume { app, change } => {
            let level = os::app_volume(app, *change).ok_or_else(|| tf("{app} isn't playing any sound right now.", &[("app", app)]))?;
            Ok(match change {
                Change::Mute => tf("{app} muted.", &[("app", app)]),
                Change::Unmute => tf("{app} unmuted.", &[("app", app)]),
                _ => tf("{app} volume: {n}%.", &[("app", app), ("n", &level.to_string())]),
            })
        }
        Action::OutputDevice => {
            crate::platform::open_url("ms-settings:sound");
            Ok(t("Windows has no switch apps may use for the output device: Sound settings are open, pick it under Output."))
        }
        Action::Discord(d) => discord(d),
        Action::SpotifyPlay(q) => {
            let encoded: String = q
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                    _ => format!("%{b:02X}"),
                })
                .collect();
            crate::platform::open_url(&format!("spotify:search:{encoded}"));
            Ok(tf("Looking for “{query}” in Spotify.", &[("query", q)]))
        }
        Action::OpenRecent(name) => {
            let file = recent_file(name.as_deref()).ok_or_else(|| t("I couldn't find a recent file like that."))?;
            os::open_file(&file)?;
            let shown = file.file_stem().map(|s| s.to_string_lossy().trim_end_matches(".lnk").to_string()).unwrap_or_default();
            Ok(tf("Opened {file}.", &[("file", shown.trim_end_matches(".lnk"))]))
        }
        Action::Theme { dark } => os::theme(*dark).map(|_| if *dark { t("Dark mode on.") } else { t("Light mode on.") }),
        Action::DoNotDisturb => {
            crate::platform::open_url("ms-settings:notifications");
            Ok(t("Windows has no switch apps may use for Do not disturb: its setting is open at the top of Notifications."))
        }
        Action::NightLight => {
            crate::platform::open_url("ms-settings:nightlight");
            Ok(t("Windows has no switch apps may use for Night light: its setting is open."))
        }
        Action::Radio { wifi, on } => os::radio(*wifi, *on).map(|_| match (wifi, on) {
            (true, true) => t("Wi-Fi on."),
            (true, false) => t("Wi-Fi off."),
            (false, true) => t("Bluetooth on."),
            (false, false) => t("Bluetooth off."),
        }),
        Action::Brightness(change) => os::brightness(*change).map(|n| tf("Brightness {n}%.", &[("n", &n.to_string())])),
        Action::SetClipboard(text) => {
            let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
            cb.set_text(text.clone()).map_err(|e| e.to_string())?;
            Ok(t("It's on your clipboard."))
        }
        Action::Type(text) => crate::flow::os::send_text(text).map(|_| t("Typed.")),
        Action::MemoryUse => {
            let hogs = crate::models::memory_hogs();
            if hogs.is_empty() {
                return Err(t("I couldn't read what's using memory."));
            }
            let lines: Vec<String> = hogs.iter().map(|h| format!("- {}: {} MB", h.name, h.bytes / 1_000_000)).collect();
            Ok(format!("{}\n{}", t("Using the most memory:"), lines.join("\n")))
        }
    }
}

fn not_open(app: &str) -> String {
    tf("{app} isn't open.", &[("app", app)])
}

fn discord(d: &Discord) -> Result<String, String> {
    match d {
        Discord::Open => {
            crate::platform::open_url("discord://");
            Ok(t("Opening Discord."))
        }
        Discord::Link(link) => {
            if !link.starts_with("discord://") || link.chars().any(char::is_whitespace) {
                return Err(t("Only discord:// links open in Discord."));
            }
            crate::platform::open_url(link);
            Ok(t("Opening it in Discord."))
        }
        // Discord's own shortcuts, while its window is in front.
        Discord::Mute | Discord::Deafen => {
            if !os::switch_to("discord") {
                return Err(not_open("Discord"));
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            let key = if *d == Discord::Mute { 0x4D } else { 0x44 };
            crate::flow::os::send_chord(&[0x11, 0x10], key)?;
            Ok(if *d == Discord::Mute { t("Discord: microphone muted or back on.") } else { t("Discord: deafened or back on.") })
        }
    }
}

/// Windows' Recent folder: the newest shortcut to a file (a name with an
/// extension), whose name has `words` if given.
fn recent_file(words: Option<&str>) -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("APPDATA")?).join("Microsoft\\Windows\\Recent");
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let stem = name.strip_suffix(".lnk")?;
            // "report.docx.lnk" is a file; "Downloads.lnk" is a folder.
            if !stem.contains('.') {
                return None;
            }
            if let Some(w) = words {
                if !w.to_lowercase().split_whitespace().all(|w| stem.contains(w)) {
                    return None;
                }
            }
            Some((e.metadata().ok()?.modified().ok()?, e.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().next().map(|(_, p)| p)
}

#[cfg(windows)]
mod os {
    use super::{Change, WindowHow};
    use crate::i18n::t;
    use windows::core::Interface;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW,
        SetForegroundWindow, ShowWindow, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
    };

    fn program(pid: u32) -> Option<String> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let named = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(process);
            named.ok()?;
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            Some(path.rsplit(['\\', '/']).next()?.trim_end_matches(".exe").trim_end_matches(".EXE").to_lowercase())
        }
    }

    /// Visible top-level windows whose program or title has `name` in it.
    fn windows_of(name: &str) -> Vec<HWND> {
        struct Search {
            name: String,
            found: Vec<HWND>,
        }
        unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            let s = &mut *(lparam.0 as *mut Search);
            if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                let mut buf = [0u16; 256];
                let n = GetWindowTextW(hwnd, &mut buf);
                let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]).to_lowercase();
                let program = program(pid).unwrap_or_default();
                let squashed = s.name.replace(' ', "");
                if program == "groovy" {
                    return windows::core::BOOL(1);
                }
                if program.contains(&squashed) || title.contains(&s.name) {
                    s.found.push(hwnd);
                }
            }
            windows::core::BOOL(1)
        }
        let mut search = Search { name: name.trim().to_lowercase(), found: Vec::new() };
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut search as *mut _ as isize));
        }
        search.found
    }

    pub fn switch_to(app: &str) -> bool {
        let Some(&hwnd) = windows_of(app).first() else { return false };
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            // Windows lets the app that just had input bring another to the front:
            // a tap of Alt counts.
            let _ = crate::flow::os::send_keys(&[0x12]);
            SetForegroundWindow(hwnd).as_bool()
        }
    }

    pub fn close(app: &str) -> usize {
        let found = windows_of(app);
        for hwnd in &found {
            unsafe {
                let _ = PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        found.len()
    }

    pub fn window(how: WindowHow) -> Result<(), String> {
        const VK_LWIN: u16 = 0x5B;
        const VK_SHIFT: u16 = 0x10;
        const VK_LEFT: u16 = 0x25;
        const VK_RIGHT: u16 = 0x27;
        let hwnd = unsafe { GetForegroundWindow() };
        match how {
            WindowHow::Minimise => unsafe {
                let _ = ShowWindow(hwnd, SW_MINIMIZE);
                Ok(())
            },
            WindowHow::Maximise => unsafe {
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                Ok(())
            },
            WindowHow::SnapLeft => crate::flow::os::send_chord(&[VK_LWIN], VK_LEFT),
            WindowHow::SnapRight => crate::flow::os::send_chord(&[VK_LWIN], VK_RIGHT),
            WindowHow::OtherMonitor => crate::flow::os::send_chord(&[VK_LWIN, VK_SHIFT], VK_RIGHT),
        }
    }

    fn com() {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    pub fn master_volume(percent: u8) -> Result<(), String> {
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
        com();
        unsafe {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let device = devices.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| e.to_string())?;
            let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
            volume.SetMasterVolumeLevelScalar(percent as f32 / 100.0, std::ptr::null()).map_err(|e| e.to_string())
        }
    }

    /// Changes an app's volume in the mixer; its new level (0–100), or None when it has no sound.
    pub fn app_volume(app: &str, change: Change) -> Option<u8> {
        use windows::Win32::Media::Audio::{eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
        com();
        let wanted = app.trim().to_lowercase().replace(' ', "");
        let mut level = None;
        unsafe {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let device = devices.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
            let manager = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None).ok()?;
            let sessions = manager.GetSessionEnumerator().ok()?;
            for i in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(control) = sessions.GetSession(i) else { continue };
                let Ok(control) = control.cast::<IAudioSessionControl2>() else { continue };
                let pid = control.GetProcessId().unwrap_or(0);
                if pid == 0 || !program(pid).is_some_and(|p| p.contains(&wanted)) {
                    continue;
                }
                let Ok(volume) = control.cast::<ISimpleAudioVolume>() else { continue };
                let now = volume.GetMasterVolume().unwrap_or(1.0);
                let next = match change {
                    Change::Up => (now + 0.2).min(1.0),
                    Change::Down => (now - 0.2).max(0.05),
                    Change::Set(n) => n as f32 / 100.0,
                    Change::Mute | Change::Unmute => now,
                };
                match change {
                    Change::Mute => {
                        let _ = volume.SetMute(true, std::ptr::null());
                    }
                    Change::Unmute => {
                        let _ = volume.SetMute(false, std::ptr::null());
                    }
                    _ => {
                        let _ = volume.SetMasterVolume(next, std::ptr::null());
                    }
                }
                level = Some((next * 100.0).round() as u8);
            }
        }
        level
    }

    pub fn open_file(path: &std::path::Path) -> Result<(), String> {
        std::process::Command::new("explorer.exe").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    pub fn theme(dark: bool) -> Result<(), String> {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD};
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
        let light: u32 = if dark { 0 } else { 1 };
        let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        unsafe {
            for name in [w!("AppsUseLightTheme"), w!("SystemUsesLightTheme")] {
                let r = RegSetKeyValueW(HKEY_CURRENT_USER, key, name, REG_DWORD.0, Some(&light as *const u32 as *const _), 4);
                if r.is_err() {
                    return Err(t("Windows didn't take the change."));
                }
            }
            let topic = w!("ImmersiveColorSet");
            let _ = SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(topic.as_ptr() as isize), SMTO_ABORTIFHUNG, 1000, None);
        }
        Ok(())
    }

    /// Wi-Fi or Bluetooth on or off, with Windows' radios API (it may ask the user once).
    pub fn radio(wifi: bool, on: bool) -> Result<(), String> {
        use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
        com();
        let access = Radio::RequestAccessAsync().and_then(|a| a.get()).map_err(|e| e.to_string())?;
        if access != RadioAccessStatus::Allowed {
            return Err(t("Windows didn't let Groovy change the radios: allow it in Settings → Privacy → Radios."));
        }
        let radios = Radio::GetRadiosAsync().and_then(|r| r.get()).map_err(|e| e.to_string())?;
        let kind = if wifi { RadioKind::WiFi } else { RadioKind::Bluetooth };
        let mut found = false;
        for radio in radios {
            if radio.Kind().ok() == Some(kind) {
                found = true;
                radio.SetStateAsync(if on { RadioState::On } else { RadioState::Off }).and_then(|s| s.get()).map_err(|e| e.to_string())?;
            }
        }
        if found {
            Ok(())
        } else if wifi {
            Err(t("This PC has no Wi-Fi."))
        } else {
            Err(t("This PC has no Bluetooth."))
        }
    }

    /// The built-in screen's brightness through WMI (laptops); its new level.
    /// A fixed PowerShell command: the only thing put in it is a number.
    pub fn brightness(change: Change) -> Result<u8, String> {
        use std::os::windows::process::CommandExt;
        let read = "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness | Select-Object -First 1).CurrentBrightness";
        let run = |script: &str| {
            std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .creation_flags(0x0800_0000)
                .output()
                .map_err(|e| e.to_string())
        };
        let now: i32 = String::from_utf8_lossy(&run(read)?.stdout).trim().parse().map_err(|_| t("This screen's brightness can't be changed by apps (an external monitor?)."))?;
        let target = match change {
            Change::Up => (now + 20).min(100),
            Change::Down => (now - 20).max(5),
            Change::Set(n) => n as i32,
            Change::Mute | Change::Unmute => now,
        } as u8;
        let set = format!(
            "Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods | Invoke-CimMethod -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness={target}}} | Out-Null"
        );
        let out = run(&set)?;
        if out.status.success() {
            Ok(target)
        } else {
            Err(t("This screen's brightness can't be changed by apps (an external monitor?)."))
        }
    }
}

#[cfg(not(windows))]
mod os {
    use super::{Change, WindowHow};

    fn no() -> String {
        crate::i18n::t("Only on Windows for now.")
    }
    pub fn switch_to(_app: &str) -> bool {
        false
    }
    pub fn close(_app: &str) -> usize {
        0
    }
    pub fn window(_how: WindowHow) -> Result<(), String> {
        Err(no())
    }
    pub fn master_volume(_percent: u8) -> Result<(), String> {
        Err(no())
    }
    pub fn app_volume(_app: &str, _change: Change) -> Option<u8> {
        None
    }
    pub fn open_file(path: &std::path::Path) -> Result<(), String> {
        crate::platform::open_url(&path.to_string_lossy());
        Ok(())
    }
    pub fn theme(_dark: bool) -> Result<(), String> {
        Err(no())
    }
    pub fn radio(_wifi: bool, _on: bool) -> Result<(), String> {
        Err(no())
    }
    pub fn brightness(_change: Change) -> Result<u8, String> {
        Err(no())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<Action> {
        let lower = s.to_ascii_lowercase();
        parse(&lower, s)
    }

    #[test]
    fn apps_and_windows() {
        assert_eq!(p("switch to Outlook"), Some(Action::SwitchTo("Outlook".into())));
        assert_eq!(p("close Spotify"), Some(Action::Close("Spotify".into())));
        assert_eq!(p("close tab"), None);
        assert_eq!(p("minimise"), Some(Action::Window { app: None, how: WindowHow::Minimise }));
        assert_eq!(p("maximize Chrome"), Some(Action::Window { app: Some("Chrome".into()), how: WindowHow::Maximise }));
        assert_eq!(p("snap this to the left"), Some(Action::Window { app: None, how: WindowHow::SnapLeft }));
        assert_eq!(p("move Teams to the other monitor"), Some(Action::Window { app: Some("Teams".into()), how: WindowHow::OtherMonitor }));
    }

    #[test]
    fn sound_discord_and_spotify() {
        assert_eq!(p("set the volume to 30%"), Some(Action::Volume(30)));
        assert_eq!(p("set the volume to 300"), None);
        assert_eq!(p("turn Discord down"), Some(Action::AppVolume { app: "Discord".into(), change: Change::Down }));
        assert_eq!(p("mute Spotify"), Some(Action::AppVolume { app: "Spotify".into(), change: Change::Mute }));
        assert_eq!(p("turn it down"), None);
        assert_eq!(p("set Chrome volume to 40"), Some(Action::AppVolume { app: "Chrome".into(), change: Change::Set(40) }));
        assert_eq!(p("mute me on Discord"), Some(Action::Discord(Discord::Mute)));
        assert_eq!(p("deafen me"), Some(Action::Discord(Discord::Deafen)));
        assert_eq!(p("Open Discord"), Some(Action::Discord(Discord::Open)));
        assert_eq!(p("play Daft Punk on Spotify"), Some(Action::SpotifyPlay("Daft Punk".into())));
        assert_eq!(p("play the latest Taylor Swift album"), Some(Action::SpotifyPlay("the latest Taylor Swift album".into())));
        assert_eq!(p("change the sound output"), Some(Action::OutputDevice));
    }

    #[test]
    fn settings_files_clipboard_and_typing() {
        assert_eq!(p("dark mode"), Some(Action::Theme { dark: true }));
        assert_eq!(p("switch to light mode"), Some(Action::Theme { dark: false }));
        assert_eq!(p("turn off wifi"), Some(Action::Radio { wifi: true, on: false }));
        assert_eq!(p("bluetooth on"), Some(Action::Radio { wifi: false, on: true }));
        assert_eq!(p("set the brightness to 70"), Some(Action::Brightness(Change::Set(70))));
        assert_eq!(p("dim the screen"), Some(Action::Brightness(Change::Down)));
        assert_eq!(p("do not disturb"), Some(Action::DoNotDisturb));
        assert_eq!(p("open my last file"), Some(Action::OpenRecent(None)));
        assert_eq!(p("open recent budget"), Some(Action::OpenRecent(Some("budget".into()))));
        assert_eq!(p("copy Hello World to the clipboard"), Some(Action::SetClipboard("Hello World".into())));
        assert_eq!(p("type Dear Sam,"), Some(Action::Type("Dear Sam,".into())));
        assert_eq!(p("what's using my memory"), Some(Action::MemoryUse));
    }

    #[test]
    fn whatever_closes_or_changes_the_system_asks_first() {
        use crate::tools::{def, Perm};
        for a in [Action::Close("x".into()), Action::Theme { dark: true }, Action::Radio { wifi: true, on: false }, Action::SetClipboard("x".into()), Action::Type("x".into()), Action::Brightness(Change::Up)] {
            assert_eq!(def(a.tool()).map(|d| d.default), Some(Perm::Ask), "{a:?}");
        }
    }
}
