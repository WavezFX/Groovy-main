// Groovy's offline brain: Ollama on this computer. One click sets it up —
// installs Ollama if it isn't there (Windows), starts it, and downloads a model —
// so the chat and voice calls work with no API key and no internet afterwards.
// Progress goes to the island as "ollama-setup" events.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::platform;

/// The model the one-click setup downloads on an ordinary 16 GB laptop (about
/// 2 GB): Qwen, which ran smoothly through Ollama in the owner's earlier
/// assistant (round 3D item 5). See `recommended` for other PCs.
pub const DEFAULT_MODEL: &str = "qwen2.5:3b";

const GB: u64 = 1024 * 1024 * 1024;

/// The Qwen model sized for this PC: 7B only with 32 GB or a GPU the user
/// gave the local model (gpu_layers), 3B on 12 GB or more, 1.5B below that.
pub fn recommended(total_ram: u64, gpu: bool) -> &'static str {
    if gpu || total_ram >= 30 * GB {
        "qwen2.5:7b"
    } else if total_ram >= 12 * GB {
        DEFAULT_MODEL
    } else {
        "qwen2.5:1.5b"
    }
}

/// For this PC, from the settings.
pub fn recommended_here(gpu_layers: u32) -> &'static str {
    recommended(crate::models::memory_info().total, gpu_layers > 0)
}

/// Which of the installed models to use when none was picked: the one sized
/// for this PC, then any Qwen, then the first.
pub fn pick(installed: &[String], recommended: &str) -> Option<String> {
    let bare = |n: &str| n.trim_end_matches(":latest").to_string();
    installed
        .iter()
        .find(|n| bare(n) == recommended)
        .or_else(|| installed.iter().find(|n| n.to_ascii_lowercase().contains("qwen")))
        .or_else(|| installed.first())
        .map(|n| bare(n))
}

/// Where to get Ollama when it isn't on this PC.
pub const DOWNLOAD_PAGE: &str = "https://ollama.com/download/windows";

const BASE: &str = "http://127.0.0.1:11434";
const INSTALLER_URL: &str = "https://ollama.com/download/OllamaSetup.exe";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    /// Ollama answers on its usual address.
    pub running: bool,
    /// Ollama is on this computer (running or not).
    pub installed: bool,
    /// The chat models it has.
    pub models: Vec<String>,
    /// This platform can install it with one click.
    pub can_install: bool,
    /// The Qwen model sized for this PC.
    pub recommended: String,
    /// Where to get Ollama.
    pub download_page: &'static str,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Step {
    /// "install", "start", "model" or "done".
    stage: &'static str,
    done: u64,
    total: u64,
}

fn emit(app: &AppHandle, stage: &'static str, done: u64, total: u64) {
    let _ = app.emit("ollama-setup", Step { stage, done, total });
}

/// For Ollama on this computer: never through a proxy.
fn client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder().no_proxy().timeout(timeout).build().unwrap_or_default()
}

/// The Ollama server Groovy started itself (setup), so it can be stopped
/// again when Groovy's own engine takes over; an Ollama the user runs is left alone.
static STARTED: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

/// One local engine at a time: before llama-server starts, the models Ollama
/// holds are unloaded (keep_alive 0) and a server Groovy started is stopped.
pub async fn unload_all() {
    if let Some(mut child) = STARTED.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
        crate::log::line("ollama: stopped the server Groovy started (Groovy's own engine is starting)");
        return;
    }
    let c = client(Duration::from_secs(3));
    let Ok(resp) = c.get(format!("{BASE}/api/ps")).send().await else { return };
    let Ok(v) = resp.json::<Value>().await else { return };
    for name in v["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str()) {
        let _ = c.post(format!("{BASE}/api/generate")).json(&serde_json::json!({ "model": name, "keep_alive": 0 })).send().await;
        crate::log::line(format!("ollama: unloaded {name} (Groovy's own engine is starting)"));
    }
}

/// Whether Ollama is on this PC (installed by its own installer or on the PATH).
pub fn installed() -> bool {
    exe().is_some()
}

/// Where Ollama's own installer puts it, or wherever it is on the PATH.
fn exe() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Programs").join("Ollama").join("ollama.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }
    let name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).map(|d| d.join(name)).find(|p| p.exists())
    })
}

/// The chat models Ollama has (embedding models left out), or None if it isn't answering.
async fn tags() -> Option<Vec<String>> {
    tags_at(BASE).await
}

/// The same, from Ollama at `base`.
pub async fn tags_at(base: &str) -> Option<Vec<String>> {
    let v: Value = client(Duration::from_secs(3)).get(format!("{}/api/tags", base.trim_end_matches('/'))).send().await.ok()?.json().await.ok()?;
    Some(chat_models(&v))
}

/// The chat models in an /api/tags answer (embedding models left out).
fn chat_models(v: &Value) -> Vec<String> {
    v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["name"].as_str())
        .filter(|n| !n.contains("embed"))
        .map(|n| n.trim_end_matches(":latest").to_string())
        .collect()
}

pub async fn status(gpu_layers: u32) -> OllamaStatus {
    let models = tags().await;
    OllamaStatus {
        running: models.is_some(),
        installed: models.is_some() || exe().is_some(),
        models: models.unwrap_or_default(),
        can_install: cfg!(windows),
        recommended: recommended_here(gpu_layers).to_string(),
        download_page: DOWNLOAD_PAGE,
    }
}

/// Installs (if needed), starts and fills Ollama. Returns the model to use:
/// `wanted` when given (downloaded if missing), else an installed Qwen, else
/// the Qwen sized for this PC, downloaded.
pub async fn setup(app: &AppHandle, wanted: Option<&str>, gpu_layers: u32) -> Result<String, String> {
    if tags().await.is_none() {
        if exe().is_none() {
            install(app).await?;
        }
        start(app).await?;
    }
    let models = tags().await.unwrap_or_default();
    let recommended = recommended_here(gpu_layers);
    let model = match wanted.map(str::trim).filter(|w| !w.is_empty()) {
        Some(w) => w.trim_end_matches(":latest").to_string(),
        None => models.iter().find(|m| m.to_ascii_lowercase().contains("qwen")).cloned().unwrap_or_else(|| recommended.to_string()),
    };
    if !models.contains(&model) {
        pull(app, &model).await?;
    }
    emit(app, "done", 1, 1);
    Ok(model)
}

#[cfg(windows)]
async fn install(app: &AppHandle) -> Result<(), String> {
    let dir = platform::local_dir().join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("OllamaSetup.exe");
    let mut resp = reqwest::Client::builder()
        .timeout(Duration::from_secs(60 * 60))
        .build()
        .unwrap_or_default()
        .get(INSTALLER_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Couldn't download Ollama ({e}). Check your internet connection and try again."))?;
    let total = resp.content_length().unwrap_or(0);
    let mut out = std::fs::File::create(&file).map_err(|e| e.to_string())?;
    let (mut done, mut last) = (0u64, 0u64);
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("The Ollama download stopped ({e})."))? {
        std::io::Write::write_all(&mut out, &chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if done - last > 4 * 1024 * 1024 {
            last = done;
            emit(app, "install", done, total);
        }
    }
    drop(out);
    emit(app, "install", done, done);
    // Ollama's installer (Inno Setup), without questions, for this user only.
    let status = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&file);
        cmd.args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"]);
        let result = platform::no_console(&mut cmd).status();
        let _ = std::fs::remove_file(&file);
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Ollama's installer didn't run ({e})."))?;
    if !status.success() {
        return Err("Ollama's installer stopped before finishing. Try again, or install it from ollama.com.".into());
    }
    Ok(())
}

#[cfg(not(windows))]
async fn install(_app: &AppHandle) -> Result<(), String> {
    Err("Install Ollama from ollama.com (curl -fsSL https://ollama.com/install.sh | sh), then try again.".into())
}

/// Starts Ollama's server in the background and waits for it to answer.
async fn start(app: &AppHandle) -> Result<(), String> {
    emit(app, "start", 0, 1);
    let Some(path) = exe() else {
        return Err("Ollama isn't installed. Install it from ollama.com and try again.".into());
    };
    let mut cmd = std::process::Command::new(path);
    cmd.arg("serve").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Ok(child) = platform::no_console(&mut cmd).spawn() {
        // Ends with Groovy (Windows job object): it was started for Groovy.
        platform::contain(&child);
        *STARTED.lock().unwrap() = Some(child);
    }
    for _ in 0..60 {
        if tags().await.is_some() {
            emit(app, "start", 1, 1);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err("Ollama is installed but didn't start. Open Ollama from the Start menu, then try again.".into())
}

/// Downloads a model through Ollama's own API, reporting its progress.
async fn pull(app: &AppHandle, model: &str) -> Result<(), String> {
    emit(app, "model", 0, 0);
    let mut resp = client(Duration::from_secs(6 * 60 * 60))
        .post(format!("{BASE}/api/pull"))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Ollama couldn't download {model} ({e})."))?;
    let mut buffer = Vec::<u8>::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("The model download stopped ({e})."))? {
        buffer.extend_from_slice(&chunk);
        while let Some(i) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=i).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if let Some(err) = v["error"].as_str() {
                return Err(format!("Ollama couldn't download {model}: {err}"));
            }
            if let (Some(done), Some(total)) = (v["completed"].as_u64(), v["total"].as_u64()) {
                emit(app, "model", done, total);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::serve_once;

    fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    #[test]
    fn qwen_is_sized_for_the_pc() {
        assert_eq!(recommended(8 * GB, false), "qwen2.5:1.5b");
        assert_eq!(recommended(16 * GB, false), "qwen2.5:3b");
        assert_eq!(recommended(16 * GB, true), "qwen2.5:7b", "a GPU the user gave the local model");
        assert_eq!(recommended(32 * GB, false), "qwen2.5:7b");
    }

    #[test]
    fn the_installed_models_come_from_api_tags_and_qwen_is_picked() {
        let body = br#"{"models":[{"name":"llama3.2:latest"},{"name":"nomic-embed-text:latest"},{"name":"qwen2.5:7b"},{"name":"qwen2.5:3b"}]}"#.to_vec();
        let url = serve_once("200 OK", "Content-Type: application/json\r\n", body);
        let models = block_on(tags_at(&url)).expect("Ollama answers");
        assert_eq!(models, ["llama3.2", "qwen2.5:7b", "qwen2.5:3b"], "embedding models left out, :latest dropped");
        assert_eq!(pick(&models, "qwen2.5:3b").as_deref(), Some("qwen2.5:3b"), "the one sized for this PC");
        assert_eq!(pick(&models, "qwen2.5:1.5b").as_deref(), Some("qwen2.5:7b"), "else any Qwen");
        assert_eq!(pick(&["llama3.2:latest".into()], "qwen2.5:3b").as_deref(), Some("llama3.2"), "else the first");
        assert_eq!(pick(&[], "qwen2.5:3b"), None);
    }

    #[test]
    fn ollama_down_is_none() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert_eq!(block_on(tags_at(&format!("http://127.0.0.1:{port}"))), None);
    }
}
