//! Round 3N item 2: the native speech engines run in on-demand helper processes.
//!
//! Speech recognition (Parakeet, Whisper) and Kokoro's voice each run in their own
//! copy of groovy.exe started with `--speech-helper stt|tts`. A helper starts when it
//! is first needed, loads only its own model, answers over its stdin/stdout pipe, and
//! exits once idle for `IDLE` (or as soon as Groovy closes the pipe). So the main
//! process never holds a speech model, and everything a model grew to while working
//! (onnxruntime's arenas never shrink) goes back to Windows when its helper exits.
//!
//! The wire format is a line of JSON, then `len` bytes. Requests: `{"op":"parakeet",
//! "len"}` + f32 samples, `{"op":"whisper","model","language","wake","len"}` + f32
//! samples, `{"op":"voice","which","text","gender","speed"}` (`which`: "light" or
//! "kokoro"), `{"op":"warm"}`. Answers:
//! `{"ok","error","len"}` + the transcript (JSON), the text, or the WAV.
//!
//! If a helper cannot start, the engine runs in this process as before, so no
//! feature depends on the helper. Unit tests always run in-process.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::Transcript;
use crate::notes::builder::TimedWord;

const ARG: &str = "--speech-helper";
/// A helper exits after Groovy's idle setting without a request (resources/idle.rs; this if none is given).
const IDLE: Duration = Duration::from_secs(30);
/// The longest a helper waits for its next request (seconds), whatever the idle setting.
const SHORT_IDLE: u64 = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Stt,
    Tts,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Stt => "stt",
            Kind::Tts => "tts",
        }
    }
}

static IN_HELPER: AtomicBool = AtomicBool::new(false);

/// True inside a helper process.
pub fn in_helper() -> bool {
    IN_HELPER.load(Ordering::Relaxed)
}

/// Whether this process hands speech to a helper (not inside one, not in unit tests,
/// and not turned off with GROOVY_SPEECH_IN_PROCESS=1).
pub fn enabled() -> bool {
    !cfg!(test) && !in_helper() && std::env::var_os("GROOVY_SPEECH_IN_PROCESS").is_none()
}

/// Called first thing in `run()`: in a helper process, serves until idle and returns true.
pub fn run_if_helper() -> bool {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(ARG) {
        return false;
    }
    let kind = match args.next().as_deref() {
        Some("tts") => Kind::Tts,
        _ => Kind::Stt,
    };
    // The idle setting in seconds; 0: "never", the helper stays until Groovy closes the pipe.
    let idle = args.next().and_then(|s| s.parse::<u64>().ok()).map_or(Some(IDLE), |s| (s > 0).then(|| Duration::from_secs(s)));
    IN_HELPER.store(true, Ordering::Relaxed);
    serve(kind, idle);
    true
}

// ── The helper's side ─────────────────────────────────────────────────────────

fn read_frame(r: &mut impl BufRead) -> Option<(Value, Vec<u8>)> {
    let mut line = String::new();
    if r.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let head: Value = serde_json::from_str(line.trim()).ok()?;
    let len = head["len"].as_u64().unwrap_or(0) as usize;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).ok()?;
    Some((head, body))
}

fn write_frame(w: &mut impl Write, mut head: Value, body: &[u8]) -> std::io::Result<()> {
    head["len"] = json!(body.len());
    let mut line = head.to_string();
    line.push('\n');
    w.write_all(line.as_bytes())?;
    w.write_all(body)?;
    w.flush()
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn float_bytes(samples: &[f32]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

fn transcript_json(t: &Transcript) -> Vec<u8> {
    let words: Vec<Value> = t.words.iter().map(|w| json!([w.text, w.start, w.end])).collect();
    json!({ "text": t.text, "words": words }).to_string().into_bytes()
}

fn transcript_from(bytes: &[u8]) -> Option<Transcript> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let words = v["words"]
        .as_array()?
        .iter()
        .filter_map(|w| Some(TimedWord { text: w[0].as_str()?.to_string(), start: w[1].as_f64()?, end: w[2].as_f64()? }))
        .collect();
    Some(Transcript { text: v["text"].as_str()?.to_string(), words })
}

fn answer(kind: Kind, head: &Value, body: &[u8]) -> (Value, Vec<u8>) {
    let fail = |e: &str| (json!({ "ok": false, "error": e }), Vec::new());
    match (kind, head["op"].as_str().unwrap_or("")) {
        (Kind::Stt, "warm") => {
            let ok = super::recognizer().is_some();
            (json!({ "ok": ok }), Vec::new())
        }
        (Kind::Stt, "parakeet") => match super::transcribe_here(&floats(body)) {
            Some(t) => (json!({ "ok": true }), transcript_json(&t)),
            None => fail("the voice model is not loaded"),
        },
        (Kind::Stt, "whisper") => {
            let model = head["model"].as_str().unwrap_or_default();
            let language = head["language"].as_str();
            let wake = head["wake"].as_bool().unwrap_or(false);
            match crate::voice::run_here(model, language, &floats(body), wake) {
                Ok(text) => (json!({ "ok": true }), text.into_bytes()),
                Err(e) => fail(&e),
            }
        }
        (Kind::Tts, "voice") => {
            let which = head["which"].as_str().unwrap_or_default();
            let text = head["text"].as_str().unwrap_or_default();
            let gender = head["gender"].as_str().unwrap_or_default();
            let speed = head["speed"].as_f64().unwrap_or(1.0) as f32;
            match super::tts::offline_here_by_id(which, text, gender, speed) {
                Some(wav) => (json!({ "ok": true }), wav),
                None => fail("the offline voice could not speak"),
            }
        }
        _ => fail("unknown request"),
    }
}

fn serve(kind: Kind, idle: Option<Duration>) {
    crate::log::line(format!("speech helper ({}) started", kind.name()));
    let (tx, rx) = mpsc::channel::<(Value, Vec<u8>)>();
    // Reads requests on their own thread, so being idle can be timed; ends when Groovy closes the pipe.
    std::thread::spawn(move || {
        let mut r = BufReader::new(std::io::stdin().lock());
        while let Some(frame) = read_frame(&mut r) {
            if tx.send(frame).is_err() {
                break;
            }
        }
    });
    let mut out = std::io::stdout().lock();
    loop {
        let next = match idle {
            Some(d) => rx.recv_timeout(d).map_err(|_| ()),
            None => rx.recv().map_err(|_| ()),
        };
        match next {
            Ok((head, body)) => {
                let (h, b) = answer(kind, &head, &body);
                if write_frame(&mut out, h, &b).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    crate::log::line(format!("speech helper ({}) exits (idle)", kind.name()));
}

// ── Groovy's side ─────────────────────────────────────────────────────────────

struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

fn slot(kind: Kind) -> &'static Mutex<Option<Running>> {
    static STT: Mutex<Option<Running>> = Mutex::new(None);
    static TTS: Mutex<Option<Running>> = Mutex::new(None);
    match kind {
        Kind::Stt => &STT,
        Kind::Tts => &TTS,
    }
}

fn spawn(kind: Kind) -> Option<Running> {
    // The ceiling (round 3D item 7) makes room before a model loads, as it did in-process.
    match kind {
        Kind::Stt => crate::resources::ceiling::make_room("speech", crate::resources::ceiling::WHISPER_MB),
        // The voice's own room is made in `speak`, where it is known which voice it is.
        Kind::Tts => {}
    }
    let exe = std::env::current_exe().ok()?;
    let mut cmd = Command::new(exe);
    // Round 3N item 2, step 4: never longer than SHORT_IDLE, so a model's memory goes back soon after use.
    let idle = crate::resources::idle::limit().map_or(SHORT_IDLE, |d| d.as_secs().clamp(1, SHORT_IDLE));
    cmd.arg(ARG).arg(kind.name()).arg(idle.to_string()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            let stdin = child.stdin.take()?;
            let stdout = BufReader::new(child.stdout.take()?);
            crate::log::line(format!("speech helper ({}) spawned, pid {}", kind.name(), child.id()));
            Some(Running { child, stdin, stdout })
        }
        Err(e) => {
            crate::log::line(format!("speech helper ({}) could not start: {e}; speech runs in Groovy", kind.name()));
            None
        }
    }
}

/// One request and its answer. A helper that has exited (idle) or died is started
/// again once. None: no helper could answer (the caller then works in-process).
fn call(kind: Kind, head: Value, body: &[u8]) -> Option<(Value, Vec<u8>)> {
    let mut g = slot(kind).lock().unwrap_or_else(|e| e.into_inner());
    for _ in 0..2 {
        if g.as_mut().is_none_or(|r| r.child.try_wait().ok().flatten().is_some()) {
            *g = spawn(kind);
        }
        let r = g.as_mut()?;
        let sent = write_frame(&mut r.stdin, head.clone(), body).is_ok();
        if sent {
            if let Some(answer) = read_frame(&mut r.stdout) {
                return Some(answer);
            }
        }
        if let Some(mut dead) = g.take() {
            let _ = dead.child.kill();
            let _ = dead.child.wait();
        }
    }
    None
}

/// A helper of this kind is running (its model may be loaded).
pub fn is_running(kind: Kind) -> bool {
    slot(kind)
        .try_lock()
        .map(|mut g| g.as_mut().is_some_and(|r| r.child.try_wait().ok().flatten().is_none()))
        .unwrap_or(true)
}

/// What a helper process runs, by its pid (for the memory meter): None if it isn't one.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn label(pid: u32) -> Option<&'static str> {
    for (kind, name) in [(Kind::Stt, "speech helper"), (Kind::Tts, "voice helper")] {
        if slot(kind).try_lock().ok().and_then(|g| g.as_ref().map(|r| r.child.id())) == Some(pid) {
            return Some(name);
        }
    }
    None
}

/// Stops a helper now (memory pressure, the models removed): everything it held is freed.
pub fn stop(kind: Kind) {
    if let Ok(mut g) = slot(kind).try_lock() {
        if let Some(mut r) = g.take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
            crate::log::line(format!("speech helper ({}) stopped", kind.name()));
        }
    }
}

/// Starts the recognizer's helper and loads Parakeet, before the words come.
pub fn warm() -> bool {
    call(Kind::Stt, json!({ "op": "warm" }), &[]).is_some_and(|(h, _)| h["ok"].as_bool().unwrap_or(false))
}

/// Parakeet, in its helper. Outer None: the helper is unavailable (use this process).
pub fn parakeet(samples: &[f32]) -> Option<Option<Transcript>> {
    let (h, b) = call(Kind::Stt, json!({ "op": "parakeet" }), &float_bytes(samples))?;
    Some(if h["ok"].as_bool().unwrap_or(false) { transcript_from(&b) } else { None })
}

/// Whisper, in its helper. None: the helper is unavailable (use this process).
pub fn whisper(model: &str, language: Option<&str>, pcm: &[f32], wake: bool) -> Option<Result<String, String>> {
    let (h, b) = call(Kind::Stt, json!({ "op": "whisper", "model": model, "language": language, "wake": wake }), &float_bytes(pcm))?;
    Some(if h["ok"].as_bool().unwrap_or(false) {
        Ok(String::from_utf8_lossy(&b).into_owned())
    } else {
        Err(h["error"].as_str().unwrap_or("speech failed").to_string())
    })
}

/// The offline voice `which` ("light" or "kokoro"), in its helper. Outer None:
/// the helper is unavailable (use this process).
pub fn speak(which: &str, text: &str, gender: &str, speed: f32) -> Option<Option<Vec<u8>>> {
    if !is_running(Kind::Tts) {
        let mb = if which == "kokoro" { crate::resources::ceiling::KOKORO_MB } else { crate::resources::ceiling::LIGHT_MB };
        crate::resources::ceiling::make_room("voice", mb);
    }
    let (h, b) = call(Kind::Tts, json!({ "op": "voice", "which": which, "text": text, "gender": gender, "speed": speed }), &[])?;
    Some(h["ok"].as_bool().unwrap_or(false).then_some(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_transcripts_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, json!({ "op": "parakeet" }), &float_bytes(&[0.5, -1.0])).unwrap();
        let (h, b) = read_frame(&mut BufReader::new(&buf[..])).unwrap();
        assert_eq!(h["op"], "parakeet");
        assert_eq!(floats(&b), vec![0.5, -1.0]);
        let t = Transcript { text: "hi there".into(), words: vec![TimedWord { text: "hi".into(), start: 0.0, end: 0.4 }] };
        let back = transcript_from(&transcript_json(&t)).unwrap();
        assert_eq!(back.text, "hi there");
        assert_eq!(back.words, t.words);
    }

    #[test]
    fn unit_tests_never_spawn_helpers() {
        assert!(!enabled());
    }
}
