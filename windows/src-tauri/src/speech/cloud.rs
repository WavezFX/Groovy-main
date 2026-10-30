// Cloud speech-to-text for dictation (FIXES_ROUND3B item 6), first when the
// user has a key, like the brains (brain_ai.rs); Parakeet or Whisper on the PC
// stay the fallback, and the only engine when there is no key or no network.
//
// Auto order: Deepgram Nova-3 (keyterm prompting for names), OpenAI
// gpt-4o-transcribe (a prompt with the vocabulary), ElevenLabs Scribe, then
// Gemini (the free tier of a Google AI key, prompted with the vocabulary).
// A service that says it is out of credit or rate-limited is skipped for a
// while, so the next one is tried. The audio (16 kHz mono WAV) goes only to
// the service that transcribes it, with its own key, and nowhere else.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// A cloud service that turns speech into text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Deepgram,
    OpenAi,
    ElevenLabs,
    Gemini,
}

impl Service {
    pub const ALL: [Service; 4] = [Service::Deepgram, Service::OpenAi, Service::ElevenLabs, Service::Gemini];

    pub fn key(self) -> &'static str {
        match self {
            Service::Deepgram => "deepgram-api-key",
            Service::OpenAi => "openai-api-key",
            Service::ElevenLabs => "elevenlabs-api-key",
            Service::Gemini => "google-api-key",
        }
    }

    /// Where it answers: the connection the wake word warms (round 3D item 10).
    pub fn host(self) -> &'static str {
        match self {
            Service::Deepgram => "https://api.deepgram.com/",
            Service::OpenAi => "https://api.openai.com/",
            Service::ElevenLabs => "https://api.elevenlabs.io/",
            Service::Gemini => "https://generativelanguage.googleapis.com/",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Service::Deepgram => "Deepgram Nova-3",
            Service::OpenAi => "OpenAI gpt-4o-transcribe",
            Service::ElevenLabs => "ElevenLabs Scribe",
            Service::Gemini => "Gemini",
        }
    }
}

/// How long a service that ran out of credit or hit its limit is skipped.
const COOLDOWN: Duration = Duration::from_secs(10 * 60);
/// Past this, the PC's engine transcribes instead.
pub const TIMEOUT: Duration = Duration::from_secs(8);

static RESTING: Mutex<Vec<(Service, Instant)>> = Mutex::new(Vec::new());

/// The services to try, in order: those with a key, not resting.
pub fn order(has_key: impl Fn(&str) -> bool, resting: &[(Service, Instant)], now: Instant) -> Vec<Service> {
    Service::ALL
        .into_iter()
        .filter(|s| has_key(s.key()))
        .filter(|s| !resting.iter().any(|(r, until)| r == s && *until > now))
        .collect()
}

/// An answer that means "try another service for a while".
pub fn is_limit(status: u16, body: &str) -> bool {
    status == 429 || status == 402 || (status == 403 && body.to_ascii_lowercase().contains("quota")) || body.to_ascii_lowercase().contains("insufficient_quota")
}

fn rest(service: Service) {
    let mut r = RESTING.lock().unwrap();
    r.retain(|(s, _)| *s != service);
    r.push((service, Instant::now() + COOLDOWN));
}

/// The cloud services that would be tried now, for Settings → Voice.
pub fn available() -> Vec<Service> {
    let resting = RESTING.lock().unwrap().clone();
    order(|k| crate::secrets::get(k).is_some(), &resting, Instant::now())
}

/// 16 kHz mono samples as a 16-bit PCM WAV file.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    wav_at(samples, 16_000)
}

/// Mono samples at `rate` as a 16-bit PCM WAV file.
pub fn wav_at(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A multipart/form-data body: text fields, then the audio file.
fn multipart(boundary: &str, fields: &[(&str, &str)], audio: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"dictation.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
    body.extend_from_slice(audio);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// The words to listen for: names and models first, a bounded number.
pub fn keyterms(vocabulary: &[String]) -> Vec<&str> {
    vocabulary.iter().rev().map(String::as_str).filter(|w| !w.trim().is_empty() && w.len() <= 50).take(50).collect()
}

fn prompt(vocabulary: &[String]) -> String {
    let words = keyterms(vocabulary);
    if words.is_empty() {
        "Dictation. Write names, brands, products and model numbers as they are written (S14, RTX 4060, iPhone 14 Pro).".into()
    } else {
        format!("Dictation. Names and words that may come up: {}. Write model numbers as digits (S14, RTX 4060).", words.join(", "))
    }
}

/// The text of a service's answer.
pub fn read_answer(service: Service, v: &Value) -> Option<String> {
    let text = match service {
        Service::Deepgram => v["results"]["channels"][0]["alternatives"][0]["transcript"].as_str(),
        Service::OpenAi | Service::ElevenLabs => v["text"].as_str(),
        Service::Gemini => v["candidates"][0]["content"]["parts"][0]["text"].as_str(),
    }?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

async fn ask(service: Service, key: &str, audio: &[u8], vocabulary: &[String], language: Option<&str>) -> Result<String, (u16, String)> {
    // Shared, so the connection the wake word warmed is the one used (round 3D item 10).
    let client = crate::net::client(&reqwest::Url::parse("https://api.deepgram.com/").unwrap(), TIMEOUT).map_err(|e| (0, e))?;
    let boundary = "groovy-dictation-7d4e1f";
    let request = match service {
        Service::Deepgram => {
            let mut url = reqwest::Url::parse("https://api.deepgram.com/v1/listen").unwrap();
            {
                let mut q = url.query_pairs_mut();
                q.append_pair("model", "nova-3").append_pair("smart_format", "true").append_pair("punctuate", "true");
                match language {
                    Some(l) => q.append_pair("language", l),
                    None => q.append_pair("detect_language", "true"),
                };
                for term in keyterms(vocabulary).into_iter().take(30) {
                    q.append_pair("keyterm", term);
                }
            }
            client.post(url).header("Authorization", format!("Token {key}")).header("Content-Type", "audio/wav").body(audio.to_vec())
        }
        Service::OpenAi => {
            let p = prompt(vocabulary);
            let mut fields = vec![("model", "gpt-4o-transcribe"), ("prompt", p.as_str()), ("response_format", "json")];
            if let Some(l) = language {
                fields.push(("language", l));
            }
            client
                .post("https://api.openai.com/v1/audio/transcriptions")
                .bearer_auth(key)
                .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
                .body(multipart(boundary, &fields, audio))
        }
        Service::ElevenLabs => {
            let mut fields = vec![("model_id", "scribe_v1"), ("tag_audio_events", "false")];
            if let Some(l) = language {
                fields.push(("language_code", l));
            }
            client
                .post("https://api.elevenlabs.io/v1/speech-to-text")
                .header("xi-api-key", key)
                .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
                .body(multipart(boundary, &fields, audio))
        }
        Service::Gemini => {
            let body = serde_json::json!({
                "contents": [{ "parts": [
                    { "text": format!("{} Reply with the transcript only, nothing else.", prompt(vocabulary)) },
                    { "inline_data": { "mime_type": "audio/wav", "data": base64(audio) } }
                ]}],
                "generationConfig": { "temperature": 0 }
            });
            client
                .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent")
                .header("x-goog-api-key", key)
                .json(&body)
        }
    };
    let response = request.send().await.map_err(|e| (0, e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err((status, text.chars().take(300).collect()));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| (status, e.to_string()))?;
    read_answer(service, &v).ok_or((status, "no text in the answer".into()))
}

/// The dictation transcribed by the first cloud service that answers, and
/// which one; None when there is no key, no network or no answer in time.
pub async fn transcribe(samples: &[f32], vocabulary: &[String], language: Option<&str>) -> Option<(String, Service)> {
    let services = available();
    if services.is_empty() {
        return None;
    }
    let audio = wav(samples);
    let started = Instant::now();
    for service in services {
        if started.elapsed() >= TIMEOUT {
            break;
        }
        let Some(key) = crate::secrets::get(service.key()) else { continue };
        match ask(service, &key, &audio, vocabulary, language).await {
            Ok(text) => {
                crate::log::line(format!("speech: {} in {:?}", service.name(), started.elapsed()));
                return Some((text, service));
            }
            Err((status, body)) => {
                if is_limit(status, &body) {
                    rest(service);
                }
                // The status only: an answer may quote what was said.
                crate::log::line(format!("speech: {} failed ({status})", service.name()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_order_follows_the_keys_and_skips_a_resting_service() {
        let now = Instant::now();
        let keys = |k: &str| k == "openai-api-key" || k == "google-api-key" || k == "deepgram-api-key";
        assert_eq!(order(keys, &[], now), vec![Service::Deepgram, Service::OpenAi, Service::Gemini]);
        let resting = [(Service::Deepgram, now + Duration::from_secs(60))];
        assert_eq!(order(keys, &resting, now), vec![Service::OpenAi, Service::Gemini]);
        let over = [(Service::Deepgram, now - Duration::from_secs(1))];
        assert_eq!(order(keys, &over, now)[0], Service::Deepgram);
        assert!(order(|_| false, &[], now).is_empty());
    }

    #[test]
    fn out_of_credit_or_rate_limited_means_try_the_next() {
        assert!(is_limit(429, ""));
        assert!(is_limit(402, ""));
        assert!(is_limit(400, r#"{"error":{"code":"insufficient_quota"}}"#));
        assert!(!is_limit(401, "bad key"));
        assert!(!is_limit(500, ""));
    }

    #[test]
    fn every_service_has_a_key_groovy_may_store() {
        for s in Service::ALL {
            assert!(crate::secrets::KNOWN_KEYS.contains(&s.key()), "{}", s.key());
        }
    }

    #[test]
    fn the_answers_are_read_and_empty_ones_refused() {
        let dg = serde_json::json!({"results":{"channels":[{"alternatives":[{"transcript":" the Asus VivoBook S14 "}]}]}});
        assert_eq!(read_answer(Service::Deepgram, &dg).unwrap(), "the Asus VivoBook S14");
        assert_eq!(read_answer(Service::OpenAi, &serde_json::json!({"text":"hi"})).unwrap(), "hi");
        let gm = serde_json::json!({"candidates":[{"content":{"parts":[{"text":"RTX 4060"}]}}]});
        assert_eq!(read_answer(Service::Gemini, &gm).unwrap(), "RTX 4060");
        assert!(read_answer(Service::ElevenLabs, &serde_json::json!({"text":"  "})).is_none());
    }

    #[test]
    fn the_wav_and_the_form_are_well_made() {
        let w = wav(&[0.0, 1.0, -1.0]);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(i16::from_le_bytes([w[46], w[47]]), 32767);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        let body = String::from_utf8_lossy(&multipart("B", &[("model", "m")], b"xyz")).to_string();
        assert!(body.starts_with("--B\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nm\r\n"));
        assert!(body.ends_with("xyz\r\n--B--\r\n"));
    }

    #[test]
    fn the_vocabulary_is_asked_for_newest_first_and_bounded() {
        let vocab: Vec<String> = (0..80).map(|i| format!("Name{i}")).collect();
        let k = keyterms(&vocab);
        assert_eq!(k.len(), 50);
        assert_eq!(k[0], "Name79");
        assert!(prompt(&vocab[..2]).contains("Name1, Name0"));
    }
}
