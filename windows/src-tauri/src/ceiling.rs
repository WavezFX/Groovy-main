// Groovy's memory ceiling (round 3D item 7): never more than 400 MB, even at
// peak, counting every process Groovy owns as Task Manager's "Memory" column
// does (the private working set): groovy.exe, its WebView2 processes, and the
// helpers it starts (llama-server, ffmpeg). Apps the user installed
// themselves (Ollama, LM Studio, Spotify) are their own and don't count, even
// when Groovy started them.
//
// A watcher reads the total every few seconds. Near the ceiling (380 MB) it
// frees what is idle (speech models, the voice), asks the pages to drop their
// caches, sets hidden webviews to WebView2's Low memory target and closes the
// panel window used longest ago. Loading a model first makes room for it
// (`make_room`): speech recognition and the voice are never in memory
// together when both would cross the line.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// The owner's rule: never above this, even at peak.
pub const CEILING_MB: f64 = 400.0;
/// Where Groovy starts giving memory back.
pub const PRESSURE_MB: f64 = 380.0;

/// About what each model adds once loaded, measured on Windows (MB).
#[allow(dead_code)] // used only with the "voice" feature
/// Whisper while it transcribes (its buffers too), as CI's peak test saw it.
pub const WHISPER_MB: f64 = 240.0;
pub const KOKORO_MB: f64 = 230.0;

/// How often the watcher reads the total.
const EVERY: Duration = Duration::from_secs(3);
/// At most one round of giving back per this long.
const COOLDOWN: Duration = Duration::from_secs(20);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// "groovy.exe", "webview2", "built-in model", or the helper's file name.
    pub kind: String,
    pub mb: f64,
}

/// What a process is, for the System page and the log.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn kind_of(exe: &str, ours: bool) -> Option<String> {
    let e = exe.to_ascii_lowercase();
    if ours {
        return Some("groovy.exe".into());
    }
    // The user's own apps, whoever started them.
    if ["ollama", "lm studio", "lms.exe", "spotify"].iter().any(|n| e.contains(n)) {
        return None;
    }
    Some(match e.as_str() {
        "msedgewebview2.exe" => "webview2".into(),
        "llama-server.exe" | "llama-server" => "built-in model".into(),
        _ => e,
    })
}

/// Groovy's own processes and their private working set.
pub fn parts() -> Vec<Part> {
    os::parts()
}

/// The parts added up by kind, largest first.
pub fn by_kind(parts: &[Part]) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    for p in parts {
        match out.iter_mut().find(|o| o.kind == p.kind) {
            Some(o) => o.mb += p.mb,
            None => out.push(p.clone()),
        }
    }
    for o in &mut out {
        o.mb = (o.mb * 10.0).round() / 10.0;
    }
    out.sort_by(|a, b| b.mb.total_cmp(&a.mb));
    out
}

pub fn total_mb() -> f64 {
    parts().iter().map(|p| p.mb).sum()
}

/// Whether `extra_mb` more still fits under the pressure line, given `now`.
pub fn fits(now: f64, extra_mb: f64) -> bool {
    now + extra_mb <= PRESSURE_MB
}

/// Before a model loads: if it wouldn't fit, the other speech model goes
/// first ("one model at a time"), its memory is handed back to Windows at
/// once, and if it still wouldn't fit the panel window used longest ago
/// closes. `loading` is "speech" or "voice".
pub fn make_room(loading: &str, extra_mb: f64) {
    let now = total_mb();
    if now <= 0.0 || fits(now, extra_mb) {
        return;
    }
    match loading {
        "voice" => {
            crate::speech::unload();
            crate::voice::free_now();
        }
        _ => crate::speech::tts::free(),
    }
    give_back();
    let after = total_mb();
    crate::log::line(format!("memory: {now:.0} MB, making room for {loading} (+{extra_mb:.0} MB) by freeing the other model: {after:.0} MB"));
    if !fits(after, extra_mb) {
        if let Some(app) = APP.get() {
            close_oldest_panel(app);
        }
    }
}

/// Hands memory a freed model leaves behind back to Windows straight away:
/// the heaps' free blocks are released and the pages left are trimmed from
/// the working set (they come back on use). Without it a freed model's
/// memory stayed counted for seconds, long enough to cross the ceiling when
/// the next model loaded (CI's peak test, 446 MB).
pub fn give_back() {
    os::give_back();
}

static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// When each panel window was last focused, to close the oldest first.
static USED: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

pub fn panel_used(label: &str) {
    let mut used = USED.lock().unwrap();
    used.retain(|(l, _)| l != label);
    used.push((label.to_string(), Instant::now()));
}

/// The panel windows besides the island: Settings, the Coding Studio and the desktop Groovy.
pub const PANELS: &[&str] = &[crate::HUB_LABEL, crate::studio::LABEL, crate::desktop::LABEL];

fn close_panel(app: &AppHandle, label: &str) {
    if label == crate::desktop::LABEL {
        crate::desktop::put_away(app);
    } else if let Some(win) = app.get_webview_window(label) {
        let _ = win.close();
    }
}

/// At most one extra WebView besides the island: opening `label` closes the
/// other panel windows (the desktop Groovy goes back into the island).
pub fn one_panel(app: &AppHandle, label: &str) {
    panel_used(label);
    for other in PANELS.iter().filter(|l| **l != label) {
        if app.get_webview_window(other).is_some() {
            crate::log::line(format!("memory: {other} closed, {label} opens (one panel window at a time)"));
            close_panel(app, other);
        }
    }
}

/// The panel to close under pressure: the open one used longest ago, never
/// one that has the focus.
pub fn oldest_panel(open: &[String], focused: Option<&str>, used: &[(String, Instant)]) -> Option<String> {
    open.iter()
        .filter(|l| Some(l.as_str()) != focused)
        .min_by_key(|l| used.iter().find(|(u, _)| u == *l).map(|(_, t)| *t))
        .cloned()
}

/// Near the ceiling: give back what can be given back.
fn relieve(app: &AppHandle, now: f64) {
    crate::log::line(format!("memory: {now:.0} MB, near the {CEILING_MB:.0} MB ceiling: dropping caches and idle models"));
    crate::speech::unload_idle_now();
    crate::speech::tts::free();
    let _ = app.emit("memory-pressure", now);
    let mut open = Vec::new();
    let mut focused = None;
    for (label, win) in app.webview_windows() {
        let visible = win.is_visible().unwrap_or(false);
        if !visible {
            crate::webview_memory::set_low(&win, true);
        }
        if PANELS.contains(&label.as_str()) {
            if win.is_focused().unwrap_or(false) {
                focused = Some(label.clone());
            }
            open.push(label);
        }
    }
    close_oldest(app, open, focused);
    give_back();
}

fn close_oldest(app: &AppHandle, open: Vec<String>, focused: Option<String>) {
    let used = USED.lock().unwrap().clone();
    if let Some(label) = oldest_panel(&open, focused.as_deref(), &used) {
        crate::log::line(format!("memory: closing {label}, the panel used longest ago"));
        close_panel(app, &label);
    }
}

fn close_oldest_panel(app: &AppHandle) {
    let mut open = Vec::new();
    let mut focused = None;
    for (label, win) in app.webview_windows() {
        if PANELS.contains(&label.as_str()) {
            if win.is_focused().unwrap_or(false) {
                focused = Some(label.clone());
            }
            open.push(label);
        }
    }
    close_oldest(app, open, focused);
}

/// The highest total seen since Groovy started, for the System page.
static PEAK: Mutex<f64> = Mutex::new(0.0);

pub fn peak_mb() -> f64 {
    *PEAK.lock().unwrap()
}

pub fn start(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let app = app.clone();
    let _ = std::thread::Builder::new().name("groovy-memory".into()).spawn(move || {
        let mut last: Option<Instant> = None;
        loop {
            std::thread::sleep(EVERY);
            let now = total_mb();
            {
                let mut peak = PEAK.lock().unwrap();
                if now > *peak {
                    *peak = now;
                }
            }
            if now >= PRESSURE_MB && last.is_none_or(|t| t.elapsed() >= COOLDOWN) {
                last = Some(Instant::now());
                relieve(&app, now);
            }
        }
    });
}

#[cfg(windows)]
mod os {
    use super::{kind_of, Part};
    use std::collections::HashMap;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    fn private_working_set(pid: u32) -> Option<f64> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut c = PROCESS_MEMORY_COUNTERS_EX2 { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32, ..Default::default() };
            let r = GetProcessMemoryInfo(h, &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb);
            let _ = CloseHandle(h);
            r.ok()?;
            Some(c.PrivateWorkingSetSize as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn give_back() {
        use windows::Win32::System::Memory::{GetProcessHeaps, HeapCompact, HEAP_FLAGS};
        use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows::Win32::System::Threading::GetCurrentProcess;
        unsafe {
            let mut heaps = vec![Default::default(); 64];
            let n = (GetProcessHeaps(&mut heaps) as usize).min(heaps.len());
            for heap in &heaps[..n] {
                HeapCompact(*heap, HEAP_FLAGS(0));
            }
            let _ = EmptyWorkingSet(GetCurrentProcess());
        }
    }

    pub fn parts() -> Vec<Part> {
        let me = std::process::id();
        let mut table: HashMap<u32, (u32, String)> = HashMap::new();
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return Vec::new() };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut more = Process32FirstW(snap, &mut e).is_ok();
            while more {
                let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
                table.insert(e.th32ProcessID, (e.th32ParentProcessID, String::from_utf16_lossy(&e.szExeFile[..len])));
                more = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        // Groovy and everything under it, however deep.
        let mut tree = vec![me];
        let mut i = 0;
        while i < tree.len() {
            let parent = tree[i];
            tree.extend(table.iter().filter(|(pid, (pp, _))| *pp == parent && **pid != me && !tree.contains(pid)).map(|(pid, _)| *pid).collect::<Vec<_>>());
            i += 1;
        }
        tree.into_iter()
            .filter_map(|pid| {
                let exe = table.get(&pid).map(|(_, e)| e.clone()).unwrap_or_default();
                let kind = kind_of(&exe, pid == me)?;
                Some(Part { kind, mb: private_working_set(pid)? })
            })
            .collect()
    }
}

#[cfg(not(windows))]
mod os {
    use super::Part;

    /// Elsewhere: Groovy's own process (its resident memory).
    pub fn parts() -> Vec<Part> {
        let mut sys = sysinfo::System::new();
        let pid = sysinfo::Pid::from_u32(std::process::id());
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        sys.process(pid).map(|p| vec![Part { kind: "groovy".into(), mb: p.memory() as f64 / (1024.0 * 1024.0) }]).unwrap_or_default()
    }

    pub fn give_back() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_own_apps_dont_count_and_helpers_do() {
        assert_eq!(kind_of("groovy.exe", true).as_deref(), Some("groovy.exe"));
        assert_eq!(kind_of("msedgewebview2.exe", false).as_deref(), Some("webview2"));
        assert_eq!(kind_of("llama-server.exe", false).as_deref(), Some("built-in model"));
        assert_eq!(kind_of("ffmpeg.exe", false).as_deref(), Some("ffmpeg.exe"));
        for theirs in ["ollama.exe", "ollama app.exe", "LM Studio.exe", "Spotify.exe"] {
            assert_eq!(kind_of(theirs, false), None, "{theirs} is the user's own app");
        }
    }

    #[test]
    fn parts_add_up_by_kind() {
        let parts = [
            Part { kind: "webview2".into(), mb: 40.0 },
            Part { kind: "groovy.exe".into(), mb: 60.04 },
            Part { kind: "webview2".into(), mb: 30.0 },
        ];
        let k = by_kind(&parts);
        assert_eq!(k[0], Part { kind: "webview2".into(), mb: 70.0 });
        assert_eq!(k[1], Part { kind: "groovy.exe".into(), mb: 60.0 });
    }

    #[test]
    fn a_model_loads_only_when_it_fits_under_the_line() {
        assert!(fits(130.0, WHISPER_MB), "the island alone and Whisper fit");
        assert!(!fits(200.0, WHISPER_MB), "with Settings open too, Settings closes first");
        assert!(!fits(300.0, KOKORO_MB), "the voice would cross 380 MB: speech goes first");
        assert!(PRESSURE_MB < CEILING_MB);
    }

    #[test]
    fn under_pressure_the_panel_used_longest_ago_closes_never_the_focused_one() {
        let t0 = Instant::now();
        let used = vec![("studio".to_string(), t0), ("settings".to_string(), t0 + Duration::from_secs(5))];
        let open = vec!["settings".to_string(), "studio".to_string()];
        assert_eq!(oldest_panel(&open, None, &used).as_deref(), Some("studio"));
        assert_eq!(oldest_panel(&open, Some("studio"), &used).as_deref(), Some("settings"));
        assert_eq!(oldest_panel(&["settings".into()], Some("settings"), &used), None);
    }

    #[test]
    fn this_process_is_measured() {
        let p = parts();
        assert!(!p.is_empty() && p[0].mb > 0.0, "{p:?}");
    }
}
