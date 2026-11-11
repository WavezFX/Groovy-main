// The wake phrases, "Groovy", "Hey Groovy" and "Yo Groovy" and no others
// (round 4B item 1), in what the speech engine wrote; and telling speech from
// silence on the microphone. Nothing here touches the OS.

/// Lower case, accents gone, everything but letters and digits a space.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let base = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ÿ' | 'ý' => 'y',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(base);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How the speech engine may write "Hey" and "Yo".
fn is_greeting(word: &str) -> bool {
    matches!(word, "hey" | "hay" | "yo")
}

/// What may come before a bare "Groovy" without it being a sentence about him.
fn is_filler(word: &str) -> bool {
    matches!(word, "um" | "uh" | "erm" | "er" | "hmm" | "so" | "oh" | "ah" | "and")
}

/// How Parakeet may write "Groovy", in one word.
fn is_groovy(word: &str) -> bool {
    matches!(
        word,
        "groovy" | "groovie" | "groovey" | "groovee" | "groovi" | "gruvy" | "gruvi" | "grooby" | "grovy" | "groove" | "groovys"
    )
}

/// …or in two.
fn is_groovy_pair(a: &str, b: &str) -> bool {
    matches!((a, b), ("groo", "vy") | ("groove", "y") | ("groove", "e") | ("groove", "ee") | ("grew", "vee") | ("grew", "v"))
}

/// When `text` holds a wake phrase, what was said after it (maybe nothing).
/// "Hey Groovy" and "Yo Groovy" count anywhere; a bare "Groovy" only as the
/// first word (a filler before it is fine), so a sentence that merely
/// mentions the name doesn't wake him. What follows the last wake phrase is
/// the command, with its case and punctuation as the engine wrote them.
pub fn after_wake(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let norm: Vec<String> = words.iter().map(|w| normalize(w)).collect();
    let mut end = None;
    let mut i = 0;
    while i < norm.len() {
        let leading = norm[..i].iter().all(|w| w.is_empty() || is_filler(w));
        let greeted = i > 0 && is_greeting(&norm[i - 1]);
        // The name in one word, or in two ("groo vy").
        let name = if is_groovy(&norm[i]) {
            1
        } else if norm.get(i + 1).is_some_and(|next| is_groovy_pair(&norm[i], next)) {
            2
        } else {
            0
        };
        if name > 0 {
            if greeted || leading {
                end = Some(i + name);
            }
            i += name;
            continue;
        }
        // One token for the whole phrase: "hey,groovy" (two words once the
        // comma is gone) or "heygroovy".
        let parts: Vec<&str> = norm[i].split(' ').collect();
        let glued = (parts.len() == 2 && is_greeting(parts[0]) && is_groovy(parts[1]))
            || norm[i].strip_prefix("hey").or_else(|| norm[i].strip_prefix("yo")).is_some_and(is_groovy);
        if glued {
            end = Some(i + 1);
        }
        i += 1;
    }
    let end = end?;
    Some(words[end..].join(" ").trim_start_matches([',', '.', '!', ' ']).trim().to_string())
}

/// What was said begins with "OK" or "Okay": « OK Groovy », which no longer
/// wakes him, however the spotter took it.
pub fn starts_with_ok(text: &str) -> bool {
    let norm = normalize(text);
    let mut words = norm.split(' ');
    match words.next() {
        Some("ok" | "okay" | "oke" | "okey") => true,
        Some("o") => words.next() == Some("k"),
        _ => false,
    }
}

/// The spotter heard "Hey Groovy" or "Yo Groovy" but the engine wrote it some
/// other way ("Hay Gruby, what time…"): the `words` it took are dropped.
pub fn drop_wake_words(text: &str, words: usize) -> String {
    text.split_whitespace().skip(words).collect::<Vec<_>>().join(" ").trim_start_matches([',', '.', '!', ' ']).to_string()
}

/// Speech or silence, 30 ms at a time, against a noise floor that follows the
/// room. A stretch of speech is handed out once followed by `hangover` frames
/// of silence, or when it gets too long.
pub struct Vad {
    floor: f32,
    in_speech: bool,
    speech_frames: usize,
    silent_frames: usize,
    frame: Vec<f32>,
    /// The current stretch, with a little audio from before it began.
    current: Vec<f32>,
    preroll: std::collections::VecDeque<f32>,
    /// Silence that ends a stretch, in frames.
    pub hangover: usize,
}

pub const FRAME: usize = 480; // 30 ms at 16 kHz
const PREROLL: usize = 16_000 * 3 / 10;
const MAX_SEGMENT: usize = 16_000 * 12;

impl Default for Vad {
    fn default() -> Self {
        Self {
            floor: 0.004,
            in_speech: false,
            speech_frames: 0,
            silent_frames: 0,
            frame: Vec::with_capacity(FRAME),
            current: Vec::new(),
            preroll: std::collections::VecDeque::with_capacity(PREROLL),
            hangover: 25,
        }
    }
}

impl Vad {
    #[cfg(test)]
    pub fn in_speech(&self) -> bool {
        self.in_speech
    }

    /// Feeds 16 kHz samples; the stretches of speech that ended.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        let mut done = Vec::new();
        for &s in samples {
            self.frame.push(s);
            if self.frame.len() < FRAME {
                continue;
            }
            let rms = (self.frame.iter().map(|x| x * x).sum::<f32>() / FRAME as f32).sqrt();
            let speech = rms > (self.floor * 3.0).max(0.012);
            if !speech && !self.in_speech {
                self.floor = self.floor * 0.97 + rms * 0.03;
            }
            if self.in_speech {
                self.current.extend_from_slice(&self.frame);
                if speech {
                    self.silent_frames = 0;
                } else {
                    self.silent_frames += 1;
                }
                if self.silent_frames >= self.hangover || self.current.len() >= MAX_SEGMENT {
                    done.push(std::mem::take(&mut self.current));
                    self.in_speech = false;
                    self.speech_frames = 0;
                    self.silent_frames = 0;
                }
            } else if speech {
                self.speech_frames += 1;
                if self.speech_frames >= 3 {
                    self.in_speech = true;
                    self.current = self.preroll.drain(..).collect();
                    self.current.extend_from_slice(&self.frame);
                }
            } else {
                self.speech_frames = 0;
            }
            if !self.in_speech {
                for &x in &self.frame {
                    if self.preroll.len() == PREROLL {
                        self.preroll.pop_front();
                    }
                    self.preroll.push_back(x);
                }
            }
            self.frame.clear();
        }
        done
    }

    /// Forgets what is being heard (Groovy speaks, or dictation starts).
    pub fn reset(&mut self) {
        self.in_speech = false;
        self.speech_frames = 0;
        self.silent_frames = 0;
        self.current.clear();
        self.preroll.clear();
        self.frame.clear();
    }
}

/// "fr-FR" for a French answer, else None (the system's language).
pub fn speech_lang(text: &str) -> Option<&'static str> {
    let n = normalize(text);
    let words: Vec<&str> = n.split(' ').collect();
    let french = ["le", "la", "les", "est", "tu", "vous", "je", "un", "une", "des", "pour", "pas", "c", "et", "sur", "avec", "ton", "ta"];
    let count = words.iter().filter(|w| french.contains(w)).count();
    (count * 6 >= words.len().max(1) && count >= 1).then_some("fr-FR")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_misspelt_wake_phrase_heard_by_the_spotter_is_dropped() {
        assert_eq!(drop_wake_words("Hay Gruby, what time is it?", 2), "what time is it?");
        assert_eq!(drop_wake_words("Hay Gruby.", 2), "");
        assert_eq!(drop_wake_words("Pippa, what time is it?", 1), "what time is it?");
        assert_eq!(drop_wake_words("", 2), "");
    }

    #[test]
    fn the_three_wake_phrases_however_the_engine_writes_them() {
        assert_eq!(after_wake("Hey Groovy, play some music.").as_deref(), Some("play some music."));
        assert_eq!(after_wake("Hey groovy, ça va ?").as_deref(), Some("ça va ?"));
        assert_eq!(after_wake("Yo Groovy next song").as_deref(), Some("next song"));
        assert_eq!(after_wake("yo, groovie! Quelle heure est-il ?").as_deref(), Some("Quelle heure est-il ?"));
        assert_eq!(after_wake("Groovy, what's the weather").as_deref(), Some("what's the weather"));
        assert_eq!(after_wake("Um, groovy what time is it").as_deref(), Some("what time is it"));
        assert_eq!(after_wake("hey groo vy add Stripe").as_deref(), Some("add Stripe"));
        assert_eq!(after_wake("Hey,Groovy open notepad").as_deref(), Some("open notepad"));
        assert_eq!(after_wake("heygroovy open notepad").as_deref(), Some("open notepad"));
        assert_eq!(after_wake("Groovy.").as_deref(), Some(""));
        assert_eq!(after_wake("Hey Groovy.").as_deref(), Some(""));
        // He hears his name twice: what follows the last one is the command.
        assert_eq!(after_wake("Groovy… hey Groovy, what time is it?").as_deref(), Some("what time is it?"));
        assert_eq!(normalize("Ça VA, Élodie ?"), "ca va elodie");
    }

    #[test]
    fn ok_at_the_start_is_told_apart() {
        assert!(starts_with_ok("OK Groovy, what time is it?"));
        assert!(starts_with_ok("Okay, gruby."));
        assert!(starts_with_ok("O.K. Groovie"));
        assert!(!starts_with_ok("Hey Groovy, is it OK?"));
        assert!(!starts_with_ok("Oklahoma is far"));
        assert!(!starts_with_ok(""));
    }

    #[test]
    fn nothing_else_wakes_him() {
        // « OK Groovy » and « Okay Groovy » are gone (round 4B item 1).
        assert_eq!(after_wake("OK Groovy, play some music."), None);
        assert_eq!(after_wake("Okay, groovy! What time is it?"), None);
        assert_eq!(after_wake("O.K. Groovie what's the weather"), None);
        assert_eq!(after_wake("okgroovy open notepad"), None);
        assert_eq!(after_wake("hi groovy"), None);
        // A sentence that only mentions the name.
        assert_eq!(after_wake("that bass line is groovy"), None);
        assert_eq!(after_wake("I think the groovy one is in the drawer"), None);
        assert_eq!(after_wake("hey, are you coming"), None);
        assert_eq!(after_wake("yo what's up"), None);
        assert_eq!(after_wake(""), None);
    }

    #[test]
    fn speech_is_cut_after_a_pause() {
        let mut vad = Vad::default();
        let silence = vec![0.0f32; 16_000];
        let tone: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.1).sin() * 0.3).collect();
        assert!(vad.feed(&silence).is_empty());
        assert!(vad.feed(&tone).is_empty());
        assert!(vad.in_speech());
        let out = vad.feed(&silence);
        assert_eq!(out.len(), 1);
        // The tone, a pre-roll before it and the hangover after it.
        assert!(out[0].len() > 16_000 && out[0].len() <= 16_000 + 25 * FRAME + PREROLL + 2 * FRAME);
        assert!(!vad.in_speech());
    }

    #[test]
    fn french_answers_are_spoken_in_french() {
        assert_eq!(speech_lang("C'est fait, j'ai ajouté Stripe pour toi."), Some("fr-FR"));
        assert_eq!(speech_lang("Done, Stripe is in the island."), None);
    }
}
