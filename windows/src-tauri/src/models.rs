// Local AI on this computer: GGUF models run by llama.cpp's llama-server,
// which Groovy installs, starts and stops itself — no Ollama, no Python, no
// account. After the one-time downloads, chat works with no network at all.
//
//   %LOCALAPPDATA%\Groovy\models\         GGUF files (downloaded or dropped in)
//   %LOCALAPPDATA%\Groovy\runtime\llama\  llama-server.exe and its DLLs
//
// Every download is pinned: a fixed URL and the SHA-256 it must have. A file
// whose hash does not match is deleted, never used. The server listens on
// 127.0.0.1 only, on a port picked at start, and speaks the OpenAI API, so the
// chat goes through local_chat.rs like any other local server.
//
// What a model needs is estimated from its own GGUF header (weights + KV cache
// for the profile's context size + overhead) and compared with the memory
// that is actually free before it is started: on an 8 GB laptop a model that
// would push the system into swap is refused with an explanation instead.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::assistant::AssistantPrefs;
use crate::{log, platform, settings};

// ── Pinned downloads ──────────────────────────────────────────────────────────

pub struct Runtime {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// llama.cpp b11000 (2026-09-16), the official Windows builds.
pub const RUNTIMES: &[Runtime] = &[
    Runtime {
        id: "cpu",
        label: "CPU (works everywhere)",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11000/llama-b11000-bin-win-cpu-x64.zip",
        sha256: "b6f6896d2e98c8af9ec89b2e05db05f469f295019a62446aa7472ce2b52abb29",
        size: 18_434_702,
    },
    Runtime {
        id: "vulkan",
        label: "Vulkan (uses the GPU, integrated Radeon/Intel included)",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11000/llama-b11000-bin-win-vulkan-x64.zip",
        sha256: "2b57010ad7014c75baf239502b2660df5109dd96fefa54e29b11bdb7de094e51",
        size: 31_706_644,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub license: &'static str,
    /// lightweight | balanced | performance — the smallest profile it suits.
    pub tier: &'static str,
    pub good_for: &'static str,
}

pub const CATALOG: &[CatalogModel] = &[
    CatalogModel {
        id: "qwen2.5-0.5b",
        label: "Qwen2.5 0.5B Instruct (Q4_K_M)",
        file: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
        sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
        size: 491_400_032,
        license: "Apache-2.0",
        tier: "lightweight",
        good_for: "Very fast, basic chat and commands. Weak at reasoning.",
    },
    CatalogModel {
        id: "llama3.2-1b",
        label: "Llama 3.2 1B Instruct (Q4_K_M)",
        file: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        sha256: "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83",
        size: 807_694_464,
        license: "Llama 3.2 Community License",
        tier: "lightweight",
        good_for: "Fast everyday chat, multilingual (EN, DE, ES, FR, IT, PT, HI, TH).",
    },
    CatalogModel {
        id: "qwen2.5-1.5b",
        label: "Qwen2.5 1.5B Instruct (Q4_K_M)",
        file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        size: 1_117_320_736,
        license: "Apache-2.0",
        tier: "lightweight",
        good_for: "Recommended for 8 GB laptops: good chat quality at a usable speed.",
    },
    CatalogModel {
        id: "qwen2.5-coder-1.5b",
        label: "Qwen2.5 Coder 1.5B Instruct (Q4_K_M)",
        file: "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf",
        sha256: "cc324af070c2ecbfd324a30884d2f951a7ff756aba85cb811a6ec436933bb046",
        size: 1_117_320_768,
        license: "Apache-2.0",
        tier: "lightweight",
        good_for: "Coding help offline: explaining, small functions, fixing errors.",
    },
    CatalogModel {
        id: "llama3.2-3b",
        label: "Llama 3.2 3B Instruct (Q4_K_M)",
        file: "Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        sha256: "6c1a2b41161032677be168d354123594c0e6e67d2b9227c84f296ad037c728ff",
        size: 2_019_377_696,
        license: "Llama 3.2 Community License",
        tier: "balanced",
        good_for: "Better answers, slower. Close other heavy apps on 8 GB.",
    },
    CatalogModel {
        id: "qwen2.5-3b",
        label: "Qwen2.5 3B Instruct (Q4_K_M)",
        file: "qwen2.5-3b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
        sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d",
        size: 2_104_932_768,
        license: "Qwen Research License (non-commercial)",
        tier: "balanced",
        good_for: "Better reasoning and coding than 1.5B. Non-commercial licence.",
    },
    CatalogModel {
        id: "qwen3-4b",
        label: "Qwen3 4B (Q4_K_M)",
        file: "Qwen3-4B-Q4_K_M.gguf",
        url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf",
        sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        size: 2_497_280_256,
        license: "Apache-2.0",
        tier: "performance",
        good_for: "Best quality here; thinks before answering. Slow on a laptop CPU.",
    },
];

// ── Profiles ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub context: u32,
    pub threads: u32,
    /// Share of the memory free right now a model may take.
    pub ram_share: f64,
    /// Stop the server after this many idle minutes to give the memory back; 0 = never.
    pub idle_minutes: u64,
    pub animation: &'static str,
}

pub fn profile(name: &str, cpus: u32) -> Profile {
    let cpus = cpus.max(1);
    match name {
        "lightweight" => Profile { context: 2048, threads: (cpus / 2).clamp(1, 4), ram_share: 0.6, idle_minutes: 5, animation: "reduced" },
        "performance" => Profile { context: 8192, threads: cpus, ram_share: 0.85, idle_minutes: 0, animation: "full" },
        _ => Profile { context: 4096, threads: (cpus.saturating_sub(2)).max(1).min(8), ram_share: 0.75, idle_minutes: 15, animation: "full" },
    }
}

/// The profile to suggest for a machine with `total` bytes of memory.
pub fn recommended_profile(total: u64) -> &'static str {
    const GB: u64 = 1024 * 1024 * 1024;
    if total <= 9 * GB {
        "lightweight"
    } else if total <= 17 * GB {
        "balanced"
    } else {
        "performance"
    }
}

// ── GGUF ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufInfo {
    pub version: u32,
    pub architecture: String,
    pub name: String,
    pub quantization: String,
    pub context_length: u64,
    pub block_count: u64,
    pub embedding_length: u64,
    pub head_count: u64,
    pub head_count_kv: u64,
    pub has_chat_template: bool,
}

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
/// Limits for an untrusted header: nothing legitimate comes near them.
const MAX_KV: u64 = 100_000;
const MAX_STRING: u64 = 64 * 1024 * 1024;
const MAX_ARRAY: u64 = 50_000_000;

fn rd<const N: usize>(r: &mut impl Read) -> std::io::Result<[u8; N]> {
    let mut b = [0u8; N];
    r.read_exact(&mut b)?;
    Ok(b)
}
fn u32le(r: &mut impl Read) -> std::io::Result<u32> {
    Ok(u32::from_le_bytes(rd::<4>(r)?))
}
fn u64le(r: &mut impl Read) -> std::io::Result<u64> {
    Ok(u64::from_le_bytes(rd::<8>(r)?))
}
fn bad(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_string())
}
fn gguf_string(r: &mut (impl Read + Seek), keep: bool) -> std::io::Result<Option<String>> {
    let len = u64le(r)?;
    if len > MAX_STRING {
        return Err(bad("string too long"));
    }
    if !keep {
        r.seek(SeekFrom::Current(len as i64))?;
        return Ok(None);
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn fixed_size(ty: u32) -> Option<i64> {
    match ty {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4 | 5 | 6 => Some(4),
        10 | 11 | 12 => Some(8),
        _ => None,
    }
}

enum Val {
    Int(u64),
    Str(String),
    Other,
}

fn read_value(r: &mut (impl Read + Seek), ty: u32, keep: bool) -> std::io::Result<Val> {
    Ok(match ty {
        0 | 7 => Val::Int(rd::<1>(r)?[0] as u64),
        1 => Val::Int(rd::<1>(r)?[0] as i8 as i64 as u64),
        2 => Val::Int(u16::from_le_bytes(rd::<2>(r)?) as u64),
        3 => Val::Int(i16::from_le_bytes(rd::<2>(r)?) as i64 as u64),
        4 => Val::Int(u32le(r)? as u64),
        5 => Val::Int(i32::from_le_bytes(rd::<4>(r)?) as i64 as u64),
        10 | 11 => Val::Int(u64le(r)?),
        6 | 12 => {
            r.seek(SeekFrom::Current(fixed_size(ty).unwrap()))?;
            Val::Other
        }
        8 => gguf_string(r, keep)?.map(Val::Str).unwrap_or(Val::Other),
        9 => {
            let inner = u32le(r)?;
            let count = u64le(r)?;
            if count > MAX_ARRAY {
                return Err(bad("array too long"));
            }
            if let Some(size) = fixed_size(inner) {
                r.seek(SeekFrom::Current(size * count as i64))?;
            } else if inner == 8 {
                for _ in 0..count {
                    gguf_string(r, false)?;
                }
            } else {
                for _ in 0..count {
                    read_value(r, inner, false)?;
                }
            }
            Val::Other
        }
        _ => return Err(bad("unknown value type")),
    })
}

pub fn quant_name(file_type: u64) -> &'static str {
    match file_type {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        7 => "Q8_0",
        8 => "Q5_0",
        9 => "Q5_1",
        10 => "Q2_K",
        11 => "Q3_K_S",
        12 => "Q3_K_M",
        13 => "Q3_K_L",
        14 => "Q4_K_S",
        15 => "Q4_K_M",
        16 => "Q5_K_S",
        17 => "Q5_K_M",
        18 => "Q6_K",
        19 => "IQ2_XXS",
        20 => "IQ2_XS",
        21 => "Q2_K_S",
        22 => "IQ3_XS",
        23 => "IQ3_XXS",
        24 => "IQ1_S",
        25 => "IQ4_NL",
        26 => "IQ3_S",
        27 => "IQ3_M",
        28 => "IQ2_S",
        29 => "IQ2_M",
        30 => "IQ4_XS",
        31 => "IQ1_M",
        32 => "BF16",
        _ => "unknown",
    }
}

/// Reads the metadata of a GGUF file without loading its weights.
pub fn read_gguf(path: &Path) -> std::io::Result<GgufInfo> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut r = std::io::BufReader::new(file);
    if &rd::<4>(&mut r)? != GGUF_MAGIC {
        return Err(bad("not a GGUF file"));
    }
    let version = u32le(&mut r)?;
    if !(2..=3).contains(&version) {
        return Err(bad("unsupported GGUF version"));
    }
    let _tensors = u64le(&mut r)?;
    let kv = u64le(&mut r)?;
    if kv > MAX_KV {
        return Err(bad("too many metadata entries"));
    }
    let mut info = GgufInfo { version, ..Default::default() };
    let mut ints: Vec<(String, u64)> = Vec::new();
    for _ in 0..kv {
        let key = gguf_string(&mut r, true)?.unwrap_or_default();
        let ty = u32le(&mut r)?;
        let wanted = key.starts_with("general.") || key.contains(".context_length") || key.contains(".block_count")
            || key.contains(".embedding_length") || key.contains(".attention.head_count")
            || key == "tokenizer.chat_template";
        if key == "tokenizer.chat_template" {
            info.has_chat_template = true;
            read_value(&mut r, ty, false)?;
            continue;
        }
        match read_value(&mut r, ty, wanted)? {
            Val::Str(s) if key == "general.architecture" => info.architecture = s,
            Val::Str(s) if key == "general.name" => info.name = s,
            Val::Int(v) => ints.push((key, v)),
            _ => {}
        }
    }
    // Skipped values are seeked over, which never fails by itself: a header
    // that runs past the end of the file is a truncated one.
    if r.stream_position()? > len {
        return Err(bad("truncated GGUF header"));
    }
    let arch = info.architecture.clone();
    let int = |k: &str| ints.iter().find(|(key, _)| key == k).map(|(_, v)| *v);
    info.quantization = int("general.file_type").map(quant_name).unwrap_or("unknown").to_string();
    info.context_length = int(&format!("{arch}.context_length")).unwrap_or(0);
    info.block_count = int(&format!("{arch}.block_count")).unwrap_or(0);
    info.embedding_length = int(&format!("{arch}.embedding_length")).unwrap_or(0);
    info.head_count = int(&format!("{arch}.attention.head_count")).unwrap_or(0);
    info.head_count_kv = int(&format!("{arch}.attention.head_count_kv")).unwrap_or(info.head_count);
    Ok(info)
}

/// Bytes a model needs running with `context` tokens: its weights, the f16 KV
/// cache, and a fixed allowance for the compute buffers and the server.
pub fn estimate_ram(file_size: u64, info: &GgufInfo, context: u32) -> u64 {
    const OVERHEAD: u64 = 350 * 1024 * 1024;
    let ctx = (context as u64).min(if info.context_length > 0 { info.context_length } else { u64::MAX });
    let kv = if info.head_count > 0 && info.block_count > 0 {
        let kv_dim = info.embedding_length / info.head_count * info.head_count_kv;
        2 * info.block_count * ctx * kv_dim * 2
    } else {
        // Unknown shape: assume a quarter of the weights again.
        file_size / 4
    };
    file_size + kv + OVERHEAD
}

// ── Where things are ──────────────────────────────────────────────────────────

pub fn models_dir() -> PathBuf {
    settings::local_dir().join("models")
}

fn runtime_dir() -> PathBuf {
    settings::local_dir().join("runtime").join("llama")
}

const SERVER_EXE: &str = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };

/// The llama-server Groovy installed, or one on the PATH.
pub fn server_exe() -> Option<PathBuf> {
    let own = runtime_dir().join(SERVER_EXE);
    if own.is_file() {
        return Some(own);
    }
    platform::find_on_path("llama-server")
}

fn installed_runtime() -> Option<String> {
    std::fs::read_to_string(runtime_dir().join("installed.txt")).ok().map(|s| s.trim().to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub file: String,
    pub path: String,
    pub size: u64,
    pub info: Option<GgufInfo>,
    pub error: Option<String>,
    /// Estimated memory with the current profile's context.
    pub ram_needed: u64,
    pub catalog_id: Option<String>,
}

/// Every usable GGUF under the models folder (two levels deep). Vision
/// projectors and the non-first parts of split models are left out.
pub fn discover(dir: &Path, context: u32) -> Vec<LocalModel> {
    let mut files = Vec::new();
    collect(dir, 0, &mut files);
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let file = path.file_name()?.to_string_lossy().to_string();
            let lower = file.to_lowercase();
            if lower.contains("mmproj") || (lower.contains("-of-") && !lower.contains("00001-of-")) {
                return None;
            }
            let size = std::fs::metadata(&path).ok()?.len();
            let (info, error) = match read_gguf(&path) {
                Ok(i) => (Some(i), None),
                Err(e) => (None, Some(e.to_string())),
            };
            let ram_needed = info.as_ref().map(|i| estimate_ram(size, i, context)).unwrap_or(0);
            let catalog_id = CATALOG.iter().find(|m| m.file == file).map(|m| m.id.to_string());
            Some(LocalModel { file, path: path.to_string_lossy().to_string(), size, info, error, ram_needed, catalog_id })
        })
        .collect()
}

fn collect(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
        if meta.is_dir() && depth < 2 {
            collect(&p, depth + 1, out);
        } else if meta.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("gguf")) {
            out.push(p);
        }
    }
}

/// A model file named by the page, only if it is directly a GGUF inside the
/// models folder (no `..`, no symlink out): the page never names arbitrary paths.
fn model_path(file: &str) -> Result<PathBuf, String> {
    let dir = models_dir().canonicalize().map_err(|_| "The models folder is empty.".to_string())?;
    let found = discover(&dir, 2048).into_iter().find(|m| m.file == file).ok_or("No such model.")?;
    let path = PathBuf::from(found.path).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&dir) {
        return Err("No such model.".into());
    }
    Ok(path)
}

// ── Memory ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total: u64,
    pub available: u64,
}

pub fn memory_info() -> MemoryInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    MemoryInfo { total: sys.total_memory(), available: sys.available_memory() }
}

pub fn cpu_count() -> u32 {
    std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4)
}

/// Why a model can't be started with the memory free now, if it can't.
pub fn memory_refusal(needed: u64, mem: MemoryInfo, share: f64) -> Option<String> {
    const MB: f64 = 1024.0 * 1024.0;
    let allowed = (mem.available as f64 * share) as u64;
    if needed <= allowed {
        return None;
    }
    Some(format!(
        "This model needs about {:.0} MB but only {:.0} MB of memory is free right now ({:.0} MB allowed by the performance profile). \
Close some apps, pick a smaller model, or switch to the Lightweight profile.",
        needed as f64 / MB,
        mem.available as f64 / MB,
        allowed as f64 / MB
    ))
}

// ── The server ────────────────────────────────────────────────────────────────

struct Running {
    child: Child,
    file: String,
    port: u16,
    last_used: Instant,
    idle_minutes: u64,
}

static SERVER: Mutex<Option<Running>> = Mutex::new(None);
static STARTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn free_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

pub fn server_args(model: &Path, port: u16, p: &Profile, gpu_layers: u32) -> Vec<String> {
    let mut a = vec![
        "-m".into(),
        model.to_string_lossy().to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "-c".into(),
        p.context.to_string(),
        "-t".into(),
        p.threads.to_string(),
        "--jinja".into(),
        "--no-webui".into(),
    ];
    if gpu_layers > 0 {
        a.push("-ngl".into());
        a.push(gpu_layers.to_string());
    }
    a
}

/// The address of the running server, if it runs.
pub fn running_url() -> Option<String> {
    let mut s = SERVER.lock().unwrap();
    let r = s.as_mut()?;
    if matches!(r.child.try_wait(), Ok(Some(_))) {
        *s = None;
        return None;
    }
    Some(format!("http://127.0.0.1:{}", r.port))
}

pub fn running_model() -> Option<String> {
    running_url()?;
    SERVER.lock().unwrap().as_ref().map(|r| r.file.clone())
}

pub fn touch() {
    if let Some(r) = SERVER.lock().unwrap().as_mut() {
        r.last_used = Instant::now();
    }
}

pub fn stop() {
    if let Some(mut r) = SERVER.lock().unwrap().take() {
        let _ = r.child.kill();
        let _ = r.child.wait();
        log::line(format!("local model stopped: {}", r.file));
    }
}

/// Starts llama-server with the chosen model (or keeps the one running when it
/// is already that model) and waits until it answers. Returns its address.
pub async fn ensure_running(prefs: &AssistantPrefs, app: Option<&AppHandle>) -> Result<String, String> {
    let _guard = STARTING.lock().await;
    let wanted = if prefs.local_model.is_empty() {
        discover(&models_dir(), 2048).into_iter().find(|m| m.info.is_some()).map(|m| m.file)
    } else {
        Some(prefs.local_model.clone())
    }
    .ok_or("No local model yet: download one in Settings → Models.")?;
    if running_model().as_deref() == Some(wanted.as_str()) {
        touch();
        return running_url().ok_or_else(|| "The local model stopped.".to_string());
    }
    stop();
    let exe = server_exe().ok_or("The local AI engine (llama.cpp) is not installed: install it in Settings → Models.")?;
    let path = model_path(&wanted)?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let info = read_gguf(&path).map_err(|e| format!("{wanted}: {e}"))?;
    let prof = profile(&prefs.performance_profile, cpu_count());
    if let Some(why) = memory_refusal(estimate_ram(size, &info, prof.context), memory_info(), prof.ram_share) {
        return Err(why);
    }
    let port = free_port()?;
    let gpu = if installed_runtime().as_deref() == Some("vulkan") { prefs.gpu_layers } else { 0 };
    let mut cmd = Command::new(&exe);
    cmd.args(server_args(&path, port, &prof, gpu)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    let child = platform::no_console(&mut cmd).spawn().map_err(|e| format!("Couldn't start llama-server: {e}"))?;
    log::line(format!("local model starting: {wanted} (ctx {}, {} threads, port {port})", prof.context, prof.threads));
    if let Some(app) = app {
        let _ = app.emit("local-model", serde_json::json!({ "state": "loading", "model": wanted }));
    }
    *SERVER.lock().unwrap() =
        Some(Running { child, file: wanted.clone(), port, last_used: Instant::now(), idle_minutes: prof.idle_minutes });
    let url = format!("http://127.0.0.1:{port}");
    let started = Instant::now();
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(3)).build().map_err(|e| e.to_string())?;
    loop {
        if running_url().is_none() {
            return Err("llama-server exited while loading the model (not enough memory, or a model it can't run).".into());
        }
        if let Ok(r) = client.get(format!("{url}/health")).send().await {
            if r.status().is_success() {
                break;
            }
        }
        if started.elapsed() > Duration::from_secs(180) {
            stop();
            return Err("The local model took too long to load.".into());
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    log::line(format!("local model ready in {:.1}s: {wanted}", started.elapsed().as_secs_f64()));
    if let Some(app) = app {
        let _ = app.emit("local-model", serde_json::json!({ "state": "ready", "model": wanted }));
    }
    Ok(url)
}

/// Stops an idle server when the profile says so, to give its memory back.
pub fn spawn_idle_reaper() {
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let idle = {
                let s = SERVER.lock().unwrap();
                s.as_ref().is_some_and(|r| r.idle_minutes > 0 && r.last_used.elapsed() > Duration::from_secs(r.idle_minutes * 60))
            };
            if idle {
                stop();
            }
        }
    });
}

// ── Downloads ─────────────────────────────────────────────────────────────────

static CANCEL: AtomicBool = AtomicBool::new(false);
static DOWNLOADING: AtomicBool = AtomicBool::new(false);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads `url` to `dest`, checking size and SHA-256 as it goes. The file
/// only takes its name once both match; anything else is deleted.
async fn fetch_verified(app: &AppHandle, what: &str, url: &str, sha256: &str, size: u64, dest: &Path) -> Result<(), String> {
    use std::io::Write;
    let part = dest.with_extension("part");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Couldn't download {what} ({e}). Check the internet connection."))?;
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let (mut done, mut last) = (0u64, 0u64);
    let result: Result<(), String> = async {
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("The download stopped ({e})."))? {
            if CANCEL.load(Ordering::Relaxed) {
                return Err("Download cancelled.".into());
            }
            done += chunk.len() as u64;
            if done > size {
                return Err("The download is larger than expected; it was discarded.".into());
            }
            ctx.update(&chunk);
            out.write_all(&chunk).map_err(|e| e.to_string())?;
            if done - last > 2 * 1024 * 1024 {
                last = done;
                let _ = app.emit("model-download", serde_json::json!({ "what": what, "done": done, "total": size }));
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        let got = hex(ctx.finish().as_ref());
        if done != size || got != sha256 {
            return Err(format!("{what} failed verification (SHA-256 mismatch); it was discarded."));
        }
        Ok(())
    }
    .await;
    drop(out);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
    let _ = app.emit("model-download", serde_json::json!({ "what": what, "done": size, "total": size }));
    log::line(format!("downloaded and verified: {what} ({size} bytes)"));
    Ok(())
}

struct DownloadGuard;
impl DownloadGuard {
    fn take() -> Result<Self, String> {
        if DOWNLOADING.swap(true, Ordering::SeqCst) {
            return Err("Another download is already running.".into());
        }
        CANCEL.store(false, Ordering::SeqCst);
        Ok(DownloadGuard)
    }
}
impl Drop for DownloadGuard {
    fn drop(&mut self) {
        DOWNLOADING.store(false, Ordering::SeqCst);
    }
}

/// Unpacks a zip into `dir`, refusing any entry that would land outside it.
pub fn unzip_into(zip_path: &Path, dir: &Path) -> Result<usize, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut n = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else { return Err(format!("unsafe path in archive: {}", entry.name())) };
        let out = dir.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsStatus {
    pub models_dir: String,
    pub runtime_installed: Option<String>,
    pub runtime_path: Option<String>,
    pub runtimes: Vec<serde_json::Value>,
    pub running: Option<String>,
    pub selected: String,
    pub local: Vec<LocalModel>,
    pub catalog: Vec<CatalogModel>,
    pub memory: MemoryInfo,
    pub cpus: u32,
    pub profile: Profile,
    pub recommended_profile: &'static str,
    pub downloading: bool,
}

#[tauri::command]
pub fn models_status(shared: tauri::State<crate::Shared>) -> ModelsStatus {
    let prefs = shared.settings.lock().unwrap().assistant.clone();
    let memory = memory_info();
    let cpus = cpu_count();
    let prof = profile(&prefs.performance_profile, cpus);
    ModelsStatus {
        models_dir: models_dir().to_string_lossy().to_string(),
        runtime_installed: installed_runtime().or_else(|| server_exe().map(|_| "path".to_string())),
        runtime_path: server_exe().map(|p| p.to_string_lossy().to_string()),
        runtimes: RUNTIMES
            .iter()
            .map(|r| serde_json::json!({ "id": r.id, "label": r.label, "size": r.size, "available": cfg!(windows) }))
            .collect(),
        running: running_model(),
        selected: prefs.local_model.clone(),
        local: discover(&models_dir(), prof.context),
        catalog: CATALOG.to_vec(),
        memory,
        cpus,
        profile: prof,
        recommended_profile: recommended_profile(memory.total),
        downloading: DOWNLOADING.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub async fn models_install_runtime(app: AppHandle, id: String) -> Result<String, String> {
    if !cfg!(windows) {
        return Err("On Linux, install llama.cpp from your distribution (llama-server on the PATH).".into());
    }
    let rt = RUNTIMES.iter().find(|r| r.id == id).ok_or("Unknown runtime.")?;
    let _g = DownloadGuard::take()?;
    let dl = settings::local_dir().join("downloads");
    platform::ensure_private_dir(&dl).map_err(|e| e.to_string())?;
    let zip_path = dl.join(format!("llama-{}.zip", rt.id));
    fetch_verified(&app, "llama.cpp", rt.url, rt.sha256, rt.size, &zip_path).await?;
    stop();
    let dir = runtime_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let n = unzip_into(&zip_path, &dir);
    let _ = std::fs::remove_file(&zip_path);
    let n = n?;
    if !dir.join(SERVER_EXE).is_file() {
        return Err("The archive has no llama-server; nothing was installed.".into());
    }
    std::fs::write(dir.join("installed.txt"), rt.id).map_err(|e| e.to_string())?;
    log::line(format!("llama.cpp runtime installed: {} ({n} files)", rt.id));
    Ok(rt.id.to_string())
}

#[tauri::command]
pub async fn models_download(app: AppHandle, id: String) -> Result<String, String> {
    let m = CATALOG.iter().find(|m| m.id == id).ok_or("Unknown model.")?;
    let _g = DownloadGuard::take()?;
    let dir = models_dir();
    platform::ensure_private_dir(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(m.file);
    if dest.is_file() {
        return Ok(m.file.to_string());
    }
    fetch_verified(&app, m.label, m.url, m.sha256, m.size, &dest).await?;
    Ok(m.file.to_string())
}

#[tauri::command]
pub fn models_cancel_download() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn models_delete(file: String) -> Result<(), String> {
    let path = model_path(&file)?;
    if running_model().as_deref() == Some(file.as_str()) {
        stop();
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    log::line(format!("model deleted: {file}"));
    Ok(())
}

#[tauri::command]
pub async fn models_start(app: AppHandle, shared: tauri::State<'_, crate::Shared>) -> Result<String, String> {
    let prefs = shared.settings.lock().unwrap().assistant.clone();
    ensure_running(&prefs, Some(&app)).await?;
    Ok(running_model().unwrap_or_default())
}

#[tauri::command]
pub fn models_stop() {
    stop();
}

#[tauri::command]
pub fn models_reveal_folder() -> Result<(), String> {
    let dir = models_dir();
    platform::ensure_private_dir(&dir).map_err(|e| e.to_string())?;
    platform::reveal_folder(&dir.to_string_lossy());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal GGUF v3 header with the keys Groovy reads plus some it skips.
    fn sample_gguf() -> Vec<u8> {
        fn s(out: &mut Vec<u8>, v: &str) {
            out.extend((v.len() as u64).to_le_bytes());
            out.extend(v.as_bytes());
        }
        fn kv_u32(out: &mut Vec<u8>, k: &str, v: u32) {
            s(out, k);
            out.extend(4u32.to_le_bytes());
            out.extend(v.to_le_bytes());
        }
        let mut b = Vec::new();
        b.extend(b"GGUF");
        b.extend(3u32.to_le_bytes());
        b.extend(0u64.to_le_bytes());
        b.extend(10u64.to_le_bytes());
        s(&mut b, "general.architecture");
        b.extend(8u32.to_le_bytes());
        s(&mut b, "llama");
        s(&mut b, "general.name");
        b.extend(8u32.to_le_bytes());
        s(&mut b, "Tiny Test");
        kv_u32(&mut b, "general.file_type", 15);
        kv_u32(&mut b, "llama.context_length", 131072);
        kv_u32(&mut b, "llama.block_count", 16);
        kv_u32(&mut b, "llama.embedding_length", 2048);
        kv_u32(&mut b, "llama.attention.head_count", 32);
        kv_u32(&mut b, "llama.attention.head_count_kv", 8);
        // A float and an array of strings, both skipped.
        s(&mut b, "llama.rope.freq_base");
        b.extend(6u32.to_le_bytes());
        b.extend(500000f32.to_le_bytes());
        s(&mut b, "tokenizer.ggml.tokens");
        b.extend(9u32.to_le_bytes());
        b.extend(8u32.to_le_bytes());
        b.extend(2u64.to_le_bytes());
        s(&mut b, "<s>");
        s(&mut b, "</s>");
        b
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("groovy-models-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_a_gguf_header() {
        let d = tmp("read");
        let f = d.join("m.gguf");
        std::fs::write(&f, sample_gguf()).unwrap();
        let i = read_gguf(&f).unwrap();
        assert_eq!(i.architecture, "llama");
        assert_eq!(i.name, "Tiny Test");
        assert_eq!(i.quantization, "Q4_K_M");
        assert_eq!((i.context_length, i.block_count, i.embedding_length), (131072, 16, 2048));
        assert_eq!((i.head_count, i.head_count_kv), (32, 8));
        assert!(!i.has_chat_template);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_files_that_are_not_gguf_or_are_hostile() {
        let d = tmp("bad");
        let f = d.join("x.gguf");
        std::fs::write(&f, b"PK\x03\x04 not a model").unwrap();
        assert!(read_gguf(&f).is_err());
        let mut b = sample_gguf();
        b[16..24].copy_from_slice(&u64::MAX.to_le_bytes()); // absurd kv count
        std::fs::write(&f, &b).unwrap();
        assert!(read_gguf(&f).is_err());
        let b = sample_gguf();
        std::fs::write(&f, &b[..b.len() - 3]).unwrap(); // truncated
        assert!(read_gguf(&f).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn estimates_memory_from_the_shape() {
        let info = GgufInfo { block_count: 16, embedding_length: 2048, head_count: 32, head_count_kv: 8, context_length: 131072, ..Default::default() };
        // KV: 2 (k,v) * 16 layers * 4096 ctx * (2048/32*8 = 512) * 2 bytes = 128 MiB
        let kv = 2 * 16 * 4096 * 512 * 2;
        assert_eq!(kv, 128 * 1024 * 1024);
        assert_eq!(estimate_ram(1_000, &info, 4096), 1_000 + kv + 350 * 1024 * 1024);
        // The context never goes past what the model supports.
        let short = GgufInfo { context_length: 1024, ..info.clone() };
        assert!(estimate_ram(0, &short, 8192) < estimate_ram(0, &info, 8192));
        // An unknown shape still costs something.
        assert!(estimate_ram(4000, &GgufInfo::default(), 4096) > 4000);
    }

    #[test]
    fn refuses_a_model_that_does_not_fit() {
        const GB: u64 = 1024 * 1024 * 1024;
        let mem = MemoryInfo { total: 8 * GB, available: 3 * GB };
        assert!(memory_refusal(GB, mem, 0.6).is_none());
        let why = memory_refusal(5 * GB, mem, 0.6).unwrap();
        assert!(why.contains("Lightweight"));
    }

    #[test]
    fn profiles_fit_the_target_laptop() {
        const GB: u64 = 1024 * 1024 * 1024;
        // Ryzen 7 4700U: 8 cores, 8 threads, 8 GB (about 7.4 GB visible).
        assert_eq!(recommended_profile(7 * GB + GB / 2), "lightweight");
        assert_eq!(recommended_profile(16 * GB), "balanced");
        assert_eq!(recommended_profile(32 * GB), "performance");
        let l = profile("lightweight", 8);
        assert_eq!((l.context, l.threads, l.idle_minutes), (2048, 4, 5));
        let b = profile("balanced", 8);
        assert_eq!((b.context, b.threads), (4096, 6));
        assert_eq!(profile("performance", 8).threads, 8);
        assert_eq!(profile("balanced", 1).threads, 1);
        assert_eq!(profile("nonsense", 4), profile("balanced", 4));
    }

    #[test]
    fn discovery_skips_projectors_and_later_split_parts() {
        let d = tmp("disc");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        for f in ["a.gguf", "mmproj-a.gguf", "big-00001-of-00002.gguf", "big-00002-of-00002.gguf", "notes.txt"] {
            std::fs::write(d.join(f), sample_gguf()).unwrap();
        }
        std::fs::write(d.join("sub").join("qwen2.5-0.5b-instruct-q4_k_m.gguf"), sample_gguf()).unwrap();
        std::fs::write(d.join("broken.gguf"), b"nope").unwrap();
        let found = discover(&d, 2048);
        let names: Vec<_> = found.iter().map(|m| m.file.as_str()).collect();
        assert_eq!(names, vec!["a.gguf", "big-00001-of-00002.gguf", "broken.gguf", "qwen2.5-0.5b-instruct-q4_k_m.gguf"]);
        assert!(found[2].error.is_some() && found[2].info.is_none());
        assert_eq!(found[3].catalog_id.as_deref(), Some("qwen2.5-0.5b"));
        assert!(found[0].ram_needed > 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn server_listens_on_loopback_only() {
        let a = server_args(Path::new("m.gguf"), 4321, &profile("lightweight", 8), 0);
        let host = a.iter().position(|x| x == "--host").unwrap();
        assert_eq!(a[host + 1], "127.0.0.1");
        assert!(a.windows(2).any(|w| w == ["-c", "2048"]));
        assert!(!a.contains(&"-ngl".to_string()));
        assert!(server_args(Path::new("m"), 1, &profile("balanced", 8), 99).windows(2).any(|w| w == ["-ngl", "99"]));
    }

    #[test]
    fn pinned_downloads_are_well_formed() {
        for r in RUNTIMES {
            assert!(r.url.starts_with("https://github.com/ggml-org/llama.cpp/releases/download/"));
            assert_eq!(r.sha256.len(), 64);
        }
        for m in CATALOG {
            assert!(m.url.starts_with("https://huggingface.co/") && m.url.ends_with(m.file), "{}", m.id);
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            assert!(["lightweight", "balanced", "performance"].contains(&m.tier));
        }
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
    }

    #[test]
    fn unzip_refuses_paths_outside_the_folder() {
        use std::io::Write;
        let d = tmp("zip");
        let good = d.join("good.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&good).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("llama-server.exe", o).unwrap();
            w.write_all(b"MZ").unwrap();
            w.start_file("lib/ggml.dll", o).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let out = d.join("out");
        assert_eq!(unzip_into(&good, &out).unwrap(), 2);
        assert!(out.join("lib/ggml.dll").is_file());
        let evil = d.join("evil.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&evil).unwrap());
            w.start_file("../escaped.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        assert!(unzip_into(&evil, &d.join("out2")).is_err());
        assert!(!d.join("escaped.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sha256_hex_matches_a_known_vector() {
        let d = ring::digest::digest(&ring::digest::SHA256, b"abc");
        assert_eq!(hex(d.as_ref()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
