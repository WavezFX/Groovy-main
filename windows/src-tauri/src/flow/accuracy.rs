// The dictation accuracy set (FIXES_ROUND3B item 6): 40 transcripts as a
// recogniser hears product names, brands, model numbers and everyday
// sentences, with the text that was meant. It measures what Groovy itself
// does after any engine: the vocabulary (names heard in pieces) and spoken
// model numbers. The test prints the score and fails if the proper-noun
// accuracy of the default pipeline drops below what it is now.
//
// Not measured here: the engines' own hearing (Parakeet, Whisper, the cloud
// services). That needs recorded clips run on each engine; the cases below
// are their text, ready to be recorded.

use super::vocab::{is_name_like, join_split, model_numbers};

/// (what the engine wrote, the user's vocabulary, what was meant)
pub const CASES: &[(&str, &[&str], &str)] = &[
    ("the Asus VivoBook S fourteen is light", &[], "the Asus VivoBook S14 is light"),
    ("I want the Asus vivo book S fourteen", &["VivoBook"], "I want the Asus VivoBook S14"),
    ("an RTX forty sixty is enough", &[], "an RTX 4060 is enough"),
    ("the GTX ten eighty ti still works", &[], "the GTX 1080 Ti still works"),
    ("my iPhone fourteen pro died", &[], "my iPhone 14 Pro died"),
    ("a Galaxy S twenty three ultra", &[], "a Galaxy S23 Ultra"),
    ("upgrade to Windows eleven today", &[], "upgrade to Windows 11 today"),
    ("Ryzen seven five hundred is fast", &[], "Ryzen 7500 is fast"),
    ("the Pixel eight pro camera", &[], "the Pixel 8 Pro camera"),
    ("a ThinkPad X one carbon", &[], "a ThinkPad X1 carbon"),
    ("the zen book fourteen is thin", &["ZenBook"], "the ZenBook 14 is thin"),
    ("we deploy on super base", &["Supabase"], "we deploy on super base"),
    ("we deploy on supa base", &["Supabase"], "we deploy on Supabase"),
    ("push it to git hub", &["GitHub"], "push it to GitHub"),
    ("check the next js docs", &["Next.js"], "check the Next.js docs"),
    ("ask open AI about it", &["OpenAI"], "ask OpenAI about it"),
    ("the play station five is sold out", &["PlayStation"], "the PlayStation 5 is sold out"),
    ("order a Mac book air", &["MacBook"], "order a MacBook Air"),
    ("the Snapdragon eight gen three", &[], "the Snapdragon 8 gen 3"),
    ("Core i seven", &[], "Core i 7"),
    ("send it to Vercel", &["Vercel"], "send it to Vercel"),
    ("my Kindle eleven arrived", &[], "my Kindle 11 arrived"),
    ("the XPS thirteen plus", &[], "the XPS 13 Plus"),
    ("a Surface pro nine", &[], "a Surface pro 9"),
    ("the Xbox series X is loud", &[], "the Xbox series X is loud"),
    ("meet Priya at the Shoreditch office", &[], "meet Priya at the Shoreditch office"),
    ("tell Jean-Luc the plan", &[], "tell Jean-Luc the plan"),
    ("I have two cats", &[], "I have two cats"),
    ("see you at five", &[], "see you at five"),
    ("it costs fifty pounds", &[], "it costs fifty pounds"),
    ("A fourteen hour day", &[], "A fourteen hour day"),
    ("Monday twelve people came", &[], "Monday twelve people came"),
    ("page one, then two", &[], "page one, then two"),
    ("we hired three engineers", &[], "we hired three engineers"),
    ("the meeting is at ten thirty", &[], "the meeting is at ten thirty"),
    ("call me in twenty minutes", &[], "call me in twenty minutes"),
    ("the Notion page for Q four", &[], "the Notion page for Q4"),
    ("use claude code in VS Code", &["Claude"], "use Claude code in VS Code"),
    ("the AirPods pro two", &[], "the AirPods Pro 2"),
    ("a Dimensity nine thousand", &[], "a Dimensity 9000"),
];

/// Groovy's offline pass, as Flow runs it on every engine's text.
pub fn pipeline(heard: &str, vocabulary: &[String]) -> String {
    let joined = join_split(heard, vocabulary);
    let cased = super::text::prepare(&joined, vocabulary, &[], &[]).text;
    model_numbers(&cased)
}

/// The share of the meant names and model numbers written exactly, and the
/// share of whole sentences right.
pub fn score(f: impl Fn(&str, &[String]) -> String) -> (f64, f64) {
    let (mut names, mut names_right, mut exact) = (0usize, 0usize, 0usize);
    for (heard, vocab, meant) in CASES {
        let vocabulary: Vec<String> = vocab.iter().map(|s| s.to_string()).collect();
        let got = f(heard, &vocabulary);
        if got == *meant {
            exact += 1;
        }
        let got_words: Vec<&str> = got.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')).collect();
        for word in meant.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')) {
            if is_name_like(word) {
                names += 1;
                if got_words.contains(&word) {
                    names_right += 1;
                }
            }
        }
    }
    (names_right as f64 / names.max(1) as f64, exact as f64 / CASES.len() as f64)
}

/// Where the default pipeline stands now: CI fails if it ever gets worse.
pub const BASELINE_NAMES: f64 = 0.95;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_has_forty_cases() {
        assert!(CASES.len() >= 40);
    }

    #[test]
    fn names_and_models_do_not_get_worse() {
        let (raw_names, raw_exact) = score(|heard, _| heard.to_string());
        let (names, exact) = score(pipeline);
        println!(
            "dictation accuracy (offline pass after the engine): names {:.0} % (as heard {:.0} %), whole sentences {:.0} % (as heard {:.0} %)",
            names * 100.0,
            raw_names * 100.0,
            exact * 100.0,
            raw_exact * 100.0
        );
        assert!(names >= BASELINE_NAMES, "names {names:.3} < baseline {BASELINE_NAMES}");
        assert!(names > raw_names, "the pass must help");
    }

    #[test]
    fn everyday_sentences_come_through_untouched() {
        for (heard, vocab, meant) in CASES {
            if heard == meant {
                let vocabulary: Vec<String> = vocab.iter().map(|s| s.to_string()).collect();
                assert_eq!(pipeline(heard, &vocabulary), *meant);
            }
        }
    }
}
