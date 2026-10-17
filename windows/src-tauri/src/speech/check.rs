// The guided speech setup (FIXES_ROUND1 item 3): a live level meter for the
// microphone being picked, and a short recording turned into text with the
// engine chosen (Parakeet or Whisper), so the user sees it work before
// relying on dictation, the Notetaker or « OK Groovy ». Nothing is kept.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use super::audio::{Capture, Source};

struct Check {
    capture: Capture,
    samples: Arc<Mutex<Vec<f32>>>,
}

static CHECK: Mutex<Option<Check>> = Mutex::new(None);

/// At most this much is kept for the test transcription.
const MAX_SECONDS: usize = 15;

/// 0…1 from a block of samples: RMS on a log-ish scale, so speech moves the bar.
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    // -60 dB → 0, 0 dB → 1.
    let db = 20.0 * rms.max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Opens `mic` (empty: the default one): "speech-level" events carry 0…1
/// about 15 times a second, and the audio is kept for speech_check_stop.
#[tauri::command]
pub fn speech_check_start(app: AppHandle, mic: String) -> Result<(), String> {
    speech_check_cancel();
    let samples = Arc::new(Mutex::new(Vec::new()));
    let keep = samples.clone();
    let mut last = Instant::now();
    let source = Source::Mic(if mic.is_empty() { None } else { Some(mic) });
    let capture = Capture::start(source, move |block| {
        {
            let mut s = keep.lock().unwrap();
            if s.len() < 16_000 * MAX_SECONDS {
                s.extend_from_slice(block);
            }
        }
        if last.elapsed() > Duration::from_millis(66) {
            last = Instant::now();
            let _ = app.emit_to(crate::HUB_LABEL, "speech-level", level(block));
        }
    })
    .map_err(|e| crate::i18n::tf("The microphone can't be opened: {0}", &[("0", &e)]))?;
    *CHECK.lock().unwrap() = Some(Check { capture, samples });
    Ok(())
}

/// Starts again from silence (the test's 5 seconds), keeping the microphone open.
#[tauri::command]
pub fn speech_check_reset() {
    if let Some(c) = CHECK.lock().unwrap().as_ref() {
        c.samples.lock().unwrap().clear();
    }
}

/// Closes the microphone without transcribing.
#[tauri::command]
pub fn speech_check_cancel() {
    if let Some(mut c) = CHECK.lock().unwrap().take() {
        c.capture.stop();
    }
}

/// Closes the microphone and turns what it heard into text with the chosen engine.
#[tauri::command]
pub async fn speech_check_stop() -> Result<String, String> {
    let Some(mut c) = CHECK.lock().unwrap().take() else {
        return Err(crate::i18n::t("The microphone is not open."));
    };
    c.capture.stop();
    let samples = std::mem::take(&mut *c.samples.lock().unwrap());
    if samples.len() < 16_000 / 2 {
        return Err(crate::i18n::t("Nothing was recorded: check the microphone."));
    }
    if !super::is_ready() {
        return Err(crate::i18n::t("The speech model isn't ready yet: download it first."));
    }
    let heard = tauri::async_runtime::spawn_blocking(move || super::transcribe(&samples))
        .await
        .map_err(|e| e.to_string())?
        .map(|t| t.text.trim().to_string())
        .unwrap_or_default();
    if heard.is_empty() {
        return Err(crate::i18n::t("Nothing was understood. Speak a little louder or closer, then try again."));
    }
    Ok(heard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_follows_the_loudness() {
        assert_eq!(level(&[]), 0.0);
        assert_eq!(level(&[0.0; 160]), 0.0);
        let quiet = level(&[0.001; 160]);
        let speech = level(&[0.1; 160]);
        let loud = level(&[1.0; 160]);
        assert!(quiet < speech && speech < loud);
        assert!((loud - 1.0).abs() < 1e-6);
    }
}
