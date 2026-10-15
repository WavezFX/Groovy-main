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

/// The model downloaded by the one-click setup: small enough for an ordinary
/// laptop (about 2 GB), good enough for a chatty assistant.
pub const DEFAULT_MODEL: &str = "llama3.2";

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
    let v: Value = client(Duration::from_secs(3)).get(format!("{BASE}/api/tags")).send().await.ok()?.json().await.ok()?;
    Some(
        v["models"]
            .as_array()?
            .iter()
            .filter_map(|m| m["name"].as_str())
            .filter(|n| !n.contains("embed"))
            .map(|n| n.trim_end_matches(":latest").to_string())
            .collect(),
    )
}

pub async fn status() -> OllamaStatus {
    let models = tags().await;
    OllamaStatus {
        running: models.is_some(),
        installed: models.is_some() || exe().is_some(),
        models: models.unwrap_or_default(),
        can_install: cfg!(windows),
    }
}

/// Installs (if needed), starts and fills Ollama. Returns the model to use.
pub async fn setup(app: &AppHandle) -> Result<String, String> {
    if tags().await.is_none() {
        if exe().is_none() {
            install(app).await?;
        }
        start(app).await?;
    }
    let models = tags().await.unwrap_or_default();
    if let Some(first) = models.first() {
        emit(app, "done", 1, 1);
        return Ok(first.clone());
    }
    pull(app, DEFAULT_MODEL).await?;
    emit(app, "done", 1, 1);
    Ok(DEFAULT_MODEL.to_string())
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
    let _ = platform::no_console(&mut cmd).spawn();
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
