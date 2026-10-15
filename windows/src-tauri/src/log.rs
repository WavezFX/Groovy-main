// Small append-only log at %LOCALAPPDATA%\Groovy\groovy.log (Windows) or
// ~/.local/share/groovy/groovy.log (Linux) — the equivalent of nbLog() in
// HookServer.swift. Nothing leaves the machine.

use std::io::Write;

use crate::{platform, settings};

pub fn line(message: impl AsRef<str>) {
    append(&settings::local_dir(), "groovy.log", message.as_ref());
}

/// Voice calls, stage by stage (heard, brain, spoken, what failed), at
/// %LOCALAPPDATA%\Groovy\logs\voice.log, so a call that goes wrong can be
/// diagnosed. What was said is not written, only its length.
pub fn voice(message: impl AsRef<str>) {
    append(&voice_dir(), "voice.log", message.as_ref());
}

pub fn voice_dir() -> std::path::PathBuf {
    settings::local_dir().join("logs")
}

fn append(dir: &std::path::Path, name: &str, message: &str) {
    let t = platform::local_time();
    let stamp = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    );
    if platform::ensure_private_dir(dir).is_err() {
        return;
    }
    let path = dir.join(name);
    // Keep it from growing forever: start fresh past ~1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    // Readable by us only, like the macOS log.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    if let Ok(mut file) = options.open(path) {
        let _ = writeln!(file, "{stamp} {}", message.replace(['\r', '\n'], " "));
    }
}

/// The release build aborts on panic, so without this a crash leaves no trace.
/// The panic message and where it happened go to the log and to crash.log
/// beside it (the last crash only), then the default hook runs as before.
pub fn install_crash_handler() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = crash_report(&info.to_string(), std::thread::current().name());
        line(&report);
        let dir = settings::local_dir();
        if platform::ensure_private_dir(&dir).is_ok() {
            let _ = std::fs::write(dir.join("crash.log"), format!("{report}\n"));
        }
        default(info);
    }));
}

fn crash_report(panic: &str, thread: Option<&str>) -> String {
    format!("CRASH {} on thread {}: {panic}", env!("CARGO_PKG_VERSION"), thread.unwrap_or("unnamed"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn crash_report_names_the_version_and_thread() {
        let r = super::crash_report("boom at x.rs:1:2", Some("main"));
        assert!(r.starts_with(&format!("CRASH {} on thread main:", env!("CARGO_PKG_VERSION"))));
        assert!(r.ends_with("boom at x.rs:1:2"));
        assert!(super::crash_report("x", None).contains("thread unnamed"));
    }
}
