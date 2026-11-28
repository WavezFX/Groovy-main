// The island's microphone, captured here instead of in the page (round 4A item 2).
// A page microphone (getUserMedia, an AudioContext and a ScriptProcessor) brings up
// WebView2's audio capture and media pipeline in the island's renderer for the whole
// call. Here it is the same cpal capture dictation and « Hey Groovy » use: 16 kHz
// mono, sent to the island as frames of 16-bit samples; the page's VoiceController
// is unchanged. Closed at hang-up (or when the page lets the microphone go).

use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::speech::audio;

static MIC: Mutex<Option<audio::Capture>> = Mutex::new(None);

/// 16-bit samples of a frame, as they cross to the page.
pub fn to_i16(frame: &[f32]) -> Vec<i16> {
    frame.iter().map(|v| (v.clamp(-1.0, 1.0) * 32767.0) as i16).collect()
}

/// Opens the microphone picked for dictation (or the default one); frames of `frame`
/// samples go to the island as "call-mic-frame". Err: the page opens its own instead.
#[tauri::command]
pub fn call_mic_start(app: AppHandle, frame: Option<usize>) -> Result<(), String> {
    call_mic_stop();
    let frame = frame.unwrap_or(1024).clamp(160, 4096);
    let mic = crate::flow::get(&app).map(|f| f.store.lock().unwrap().prefs.mic.clone()).unwrap_or_default();
    let mut buf: Vec<f32> = Vec::with_capacity(frame * 2);
    let capture = audio::Capture::start(audio::Source::Mic(if mic.is_empty() { None } else { Some(mic) }), move |s| {
        buf.extend_from_slice(s);
        while buf.len() >= frame {
            let out = to_i16(&buf[..frame]);
            buf.drain(..frame);
            let _ = app.emit_to(crate::island::WINDOW_LABEL, "call-mic-frame", out);
        }
    })?;
    *MIC.lock().unwrap() = Some(capture);
    crate::log::voice("microphone opened in Rust for the island");
    Ok(())
}

/// Closes the island's microphone, if open.
#[tauri::command]
pub fn call_mic_stop() {
    let taken = MIC.lock().unwrap().take();
    if let Some(mut c) = taken {
        c.stop();
        crate::log::voice("microphone closed");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn frames_cross_as_clamped_16_bit_samples() {
        assert_eq!(super::to_i16(&[0.0, 0.5, -1.0, 2.0]), vec![0, 16383, -32767, 32767]);
    }
}
