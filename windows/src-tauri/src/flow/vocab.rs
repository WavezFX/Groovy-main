// Getting names, brands, products and model numbers right in dictation
// (FIXES_ROUND3B item 6), offline, for every engine:
//
// - the personal vocabulary (the dictionary in Settings → Voice) gains the
//   words the user corrects within a minute of a dictation (`learn`);
// - a name heard split in two ("vivo book") is joined back as written
//   ("VivoBook") when the vocabulary has it (`join_split`);
// - spoken numbers next to a model name become digits: "S fourteen" → "S14",
//   "RTX forty sixty" → "RTX 4060", "iPhone fourteen pro" → "iPhone 14 Pro"
//   (`model_numbers`);
// - the vocabulary reaches the recognisers as a prompt (`whisper_prompt`,
//   cloud keyterms) and the clean-up brain.

/// Words a dictation may teach at most, and the vocabulary's size.
pub const MAX_LEARNED_AT_ONCE: usize = 8;
pub const MAX_VOCABULARY: usize = 500;

fn tokens(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// A token's letters and digits only, lower case.
fn bare(token: &str) -> String {
    token.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Leading and trailing punctuation of a token, to put back around a replacement.
fn edges(token: &str) -> (&str, &str) {
    let start = token.find(|c: char| c.is_alphanumeric()).unwrap_or(token.len());
    let end = token.rfind(|c: char| c.is_alphanumeric()).map_or(start, |i| i + token[i..].chars().next().map_or(1, char::len_utf8));
    (&token[..start], &token[end.max(start)..])
}

/// A word worth remembering: a name or a model, not an everyday word.
pub fn is_name_like(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric());
    let chars: Vec<char> = w.chars().collect();
    chars.len() >= 2
        && chars.len() <= 40
        && chars.iter().all(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        && (chars.iter().any(|c| c.is_ascii_digit()) || chars.iter().skip(1).any(|c| c.is_uppercase()) || chars[0].is_uppercase())
}

/// The longest common subsequence of two token lists, as index pairs.
fn lcs(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// What the user corrected in what Flow inserted: the name-like words of the
/// field's text that stand where other words were inserted. Nothing when the
/// field no longer holds the dictation (mostly the same words).
pub fn learn(inserted: &str, field_now: &str) -> Vec<String> {
    let ins_tokens = tokens(inserted);
    if ins_tokens.is_empty() || field_now.len() > 64 * 1024 {
        return Vec::new();
    }
    let field_tokens = tokens(field_now);
    let a: Vec<String> = ins_tokens.iter().map(|t| bare(t)).collect();
    let b: Vec<String> = field_tokens.iter().map(|t| bare(t)).collect();
    let pairs = lcs(&a, &b);
    // Still the dictation: at least two in five of its words are there, in order.
    if pairs.len() < 2 || pairs.len() * 5 < a.len() * 2 {
        return Vec::new();
    }
    let mut learned: Vec<String> = Vec::new();
    let mut bounds = vec![(usize::MAX, usize::MAX)];
    bounds.extend(pairs.iter().copied());
    bounds.push((a.len(), b.len()));
    for w in bounds.windows(2) {
        let (pi, pj) = w[0];
        let (ni, nj) = w[1];
        let (si, sj) = (pi.wrapping_add(1), pj.wrapping_add(1));
        // A replaced stretch: inserted words si..ni became field words sj..nj.
        // Only inside the dictation (an untouched start or end of the field is not a correction).
        if ni <= si || nj <= sj || (pi == usize::MAX && sj == 0 && nj > ni + 3) {
            continue;
        }
        let old: Vec<&String> = a[si..ni].iter().collect();
        for t in &field_tokens[sj..nj] {
            let word = t.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '.').trim_end_matches('.');
            if is_name_like(word) && !old.contains(&&bare(word)) && !learned.iter().any(|l| l == word) {
                learned.push(word.to_string());
            }
        }
    }
    learned.truncate(MAX_LEARNED_AT_ONCE);
    learned
}

/// Adds learned words to the vocabulary: no duplicates (any case), the newest
/// kept when it is full.
pub fn remember(vocabulary: &mut Vec<String>, learned: &[String]) -> bool {
    let mut changed = false;
    for word in learned {
        if vocabulary.iter().any(|v| v.eq_ignore_ascii_case(word)) {
            continue;
        }
        vocabulary.push(word.clone());
        changed = true;
    }
    if vocabulary.len() > MAX_VOCABULARY {
        let extra = vocabulary.len() - MAX_VOCABULARY;
        vocabulary.drain(..extra);
    }
    changed
}

/// A vocabulary word heard as two or three pieces ("vivo book", "zen book
/// fourteen") is written back as one ("VivoBook").
pub fn join_split(text: &str, vocabulary: &[String]) -> String {
    let words: Vec<(String, &String)> = vocabulary
        .iter()
        .filter(|w| !w.contains(char::is_whitespace))
        .map(|w| (bare(w), w))
        .filter(|(b, _)| b.len() >= 4)
        .collect();
    if words.is_empty() {
        return text.to_string();
    }
    let toks = tokens(text);
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    'outer: while i < toks.len() {
        for span in [3usize, 2] {
            if i + span > toks.len() {
                continue;
            }
            let joined: String = toks[i..i + span].iter().map(|t| bare(t)).collect();
            if let Some((_, word)) = words.iter().find(|(b, _)| *b == joined) {
                let (lead, _) = edges(toks[i]);
                let (_, trail) = edges(toks[i + span - 1]);
                out.push(format!("{lead}{word}{trail}"));
                i += span;
                continue 'outer;
            }
        }
        out.push(toks[i].to_string());
        i += 1;
    }
    if out.len() == toks.len() {
        return text.to_string();
    }
    out.join(" ")
}

fn unit(w: &str) -> Option<u32> {
    Some(match w {
        "zero" | "oh" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        _ => return None,
    })
}

fn tens(w: &str) -> Option<u32> {
    Some(match w {
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

/// Number words from `toks[i]` on, read the way model numbers are said:
/// groups written one after the other ("forty sixty" → "4060", "fourteen" →
/// "14", "five hundred" → "500", "twenty one" → "21"). The digits and how
/// many words they took.
fn spoken_number(toks: &[&str], i: usize) -> Option<(String, usize)> {
    let word = |k: usize| toks.get(k).map(|t| bare(t));
    let mut digits = String::new();
    let mut k = i;
    while let Some(w) = word(k) {
        if let Some(t) = tens(&w) {
            let next = word(k + 1).and_then(|n| unit(&n)).filter(|u| (1..=9).contains(u));
            if let Some(u) = next {
                digits.push_str(&(t + u).to_string());
                k += 2;
            } else {
                digits.push_str(&t.to_string());
                k += 1;
            }
        } else if let Some(u) = unit(&w) {
            if w == "oh" && digits.is_empty() {
                break;
            }
            if word(k + 1).as_deref() == Some("thousand") {
                digits.push_str(&(u * 1000).to_string());
                k += 2;
            } else if word(k + 1).as_deref() == Some("hundred") {
                let rest = word(k + 2).and_then(|n| unit(&n).or_else(|| tens(&n)));
                match rest {
                    Some(r) if r > 0 => {
                        digits.push_str(&(u * 100 + r).to_string());
                        k += 3;
                    }
                    _ => {
                        digits.push_str(&(u * 100).to_string());
                        k += 2;
                    }
                }
            } else {
                digits.push_str(&u.to_string());
                k += 1;
            }
        } else {
            break;
        }
        // Keep only what the token said (no trailing punctuation inside a number).
        if toks[k - 1].ends_with([',', '.', '!', '?', ';', ':']) {
            break;
        }
    }
    (k > i).then_some((digits, k - i))
}

/// Brand and product words a model number follows.
const PRODUCT_WORDS: &[&str] = &[
    "iphone", "ipad", "galaxy", "pixel", "vivobook", "zenbook", "rog", "thinkpad", "ideapad", "xps", "inspiron", "ryzen",
    "core", "geforce", "radeon", "windows", "playstation", "ps", "xbox", "macbook", "surface", "note", "series", "model",
    "version", "gen", "generation", "rtx", "gtx", "rx", "arc", "snapdragon", "dimensity", "exynos", "kindle", "airpods",
];

/// What comes right after a model number and is part of its name.
const SUFFIXES: &[(&str, &str)] = &[
    ("pro", "Pro"), ("max", "Max"), ("plus", "Plus"), ("ultra", "Ultra"), ("mini", "Mini"), ("ti", "Ti"), ("air", "Air"),
    ("lite", "Lite"), ("fe", "FE"),
];

/// A token a model number belongs to: a single capital letter (not "I" or
/// "A"), letters in capitals (RTX), a name with a capital inside (iPhone,
/// VivoBook), a token with a digit, or a known product word.
fn takes_model_number(token: &str) -> bool {
    let w = token.trim_matches(|c: char| !c.is_alphanumeric());
    let chars: Vec<char> = w.chars().collect();
    if chars.is_empty() || token.ends_with([',', '.', '!', '?', ';', ':']) {
        return false;
    }
    if chars.len() == 1 {
        return chars[0].is_ascii_uppercase() && chars[0] != 'I' && chars[0] != 'A';
    }
    let all_caps = chars.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) && chars.iter().any(|c| c.is_ascii_uppercase());
    let inner_capital = chars.iter().skip(1).any(|c| c.is_uppercase()) && chars.iter().any(|c| c.is_lowercase());
    all_caps || inner_capital || chars.iter().any(|c| c.is_ascii_digit()) || PRODUCT_WORDS.contains(&bare(w).as_str())
}

/// Spoken numbers after a model name written as digits, and the name's suffix
/// capitalised: "S fourteen" → "S14", "RTX forty sixty" → "RTX 4060",
/// "iPhone fourteen pro" → "iPhone 14 Pro". Everyday numbers are left alone.
pub fn model_numbers(text: &str) -> String {
    let toks = tokens(text);
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut changed = false;
    while i < toks.len() {
        let after_model = i > 0 && takes_model_number(toks[i - 1]);
        if after_model {
            if let Some((digits, used)) = spoken_number(&toks, i) {
                let (_, trail) = edges(toks[i + used - 1]);
                let prev = out.pop().unwrap_or_default();
                let single_letter = prev.trim_matches(|c: char| !c.is_alphanumeric()).chars().count() == 1;
                let mut number = format!("{digits}{trail}");
                let mut next = i + used;
                // "fourteen pro" → "14 Pro"; the suffix carries the punctuation.
                if trail.is_empty() {
                    if let Some(tok) = toks.get(next) {
                        if let Some((_, proper)) = SUFFIXES.iter().find(|(s, _)| *s == bare(tok)) {
                            let (_, t2) = edges(tok);
                            number = format!("{digits} {proper}{t2}");
                            next += 1;
                        }
                    }
                }
                if single_letter {
                    let (lead, _) = edges(&prev);
                    let letter = prev.trim_matches(|c: char| !c.is_alphanumeric());
                    out.push(format!("{lead}{letter}{number}"));
                } else {
                    out.push(prev);
                    out.push(number);
                }
                i = next;
                changed = true;
                continue;
            }
        }
        out.push(toks[i].to_string());
        i += 1;
    }
    if changed {
        out.join(" ")
    } else {
        text.to_string()
    }
}

/// Whisper's prompt: the name it must spell, then the user's words.
#[cfg_attr(not(feature = "voice"), allow(dead_code))]
pub fn whisper_prompt(vocabulary: &[String]) -> String {
    let mut prompt = String::from("Hey Groovy.");
    let mut used = 0;
    for w in vocabulary.iter().rev() {
        // Whisper reads at most 224 tokens of prompt: a few hundred characters.
        if prompt.len() + w.len() + 2 > 600 || used >= 60 {
            break;
        }
        prompt.push(' ');
        prompt.push_str(w);
        prompt.push(',');
        used += 1;
    }
    prompt.trim_end_matches(',').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn model_numbers_become_digits() {
        assert_eq!(model_numbers("the Asus VivoBook S fourteen is light"), "the Asus VivoBook S14 is light");
        assert_eq!(model_numbers("an RTX forty sixty card"), "an RTX 4060 card");
        assert_eq!(model_numbers("my iPhone fourteen pro."), "my iPhone 14 Pro.");
        assert_eq!(model_numbers("Windows eleven, finally"), "Windows 11, finally");
        assert_eq!(model_numbers("a Galaxy S twenty three ultra"), "a Galaxy S23 Ultra");
        assert_eq!(model_numbers("the GTX ten eighty ti"), "the GTX 1080 Ti");
        assert_eq!(model_numbers("Ryzen seven five hundred"), "Ryzen 7500");
        assert_eq!(model_numbers("a Dimensity nine thousand"), "a Dimensity 9000");
    }

    #[test]
    fn everyday_numbers_are_left_alone() {
        for s in [
            "I have two cats",
            "see you at five",
            "A fourteen hour day",
            "Monday twelve people came",
            "it costs fifty pounds",
            "page one, then two",
        ] {
            assert_eq!(model_numbers(s), s, "{s}");
        }
    }

    #[test]
    fn a_name_heard_in_pieces_is_joined_back() {
        let vocab = v(&["VivoBook", "Supabase", "ZenBook"]);
        assert_eq!(join_split("the vivo book is great", &vocab), "the VivoBook is great");
        assert_eq!(join_split("we use super base.", &vocab), "we use super base.");
        assert_eq!(join_split("we use supa base.", &vocab), "we use Supabase.");
        assert_eq!(join_split("nothing here", &vocab), "nothing here");
    }

    #[test]
    fn corrections_teach_names_and_models() {
        let learned = learn("the A book S fourteen is light", "the Asus VivoBook S14 is light");
        assert_eq!(learned, v(&["Asus", "VivoBook", "S14"]));
        // An everyday rewording teaches nothing.
        assert!(learn("see you at five", "see you at six").is_empty());
        // The field holds something else now: nothing is learned.
        assert!(learn("deploy it to vercel today", "Dear Sam, thanks for the call").is_empty());
        // Text typed before the dictation is not a correction.
        let learned = learn("ship it on Friday", "Notes from Kubernetes Meetup: ship it on Friday");
        assert!(learned.is_empty(), "{learned:?}");
    }

    #[test]
    fn the_vocabulary_has_no_duplicates_and_a_limit() {
        let mut vocab = v(&["Supabase"]);
        assert!(!remember(&mut vocab, &v(&["supabase"])));
        assert!(remember(&mut vocab, &v(&["VivoBook"])));
        assert_eq!(vocab, v(&["Supabase", "VivoBook"]));
        let many: Vec<String> = (0..600).map(|i| format!("Name{i}")).collect();
        remember(&mut vocab, &many);
        assert_eq!(vocab.len(), MAX_VOCABULARY);
        assert_eq!(vocab.last().unwrap(), "Name599");
    }

    #[test]
    fn whisper_hears_the_vocabulary_newest_first_within_its_prompt() {
        assert_eq!(whisper_prompt(&[]), "Hey Groovy.");
        let p = whisper_prompt(&v(&["Supabase", "VivoBook"]));
        assert_eq!(p, "Hey Groovy. VivoBook, Supabase");
        let many: Vec<String> = (0..500).map(|i| format!("Word{i}")).collect();
        assert!(whisper_prompt(&many).len() <= 600);
    }

    #[test]
    fn names_are_told_from_everyday_words() {
        for w in ["Asus", "VivoBook", "S14", "RTX", "gpt-4o"] {
            assert!(is_name_like(w), "{w}");
        }
        for w in ["the", "light", "a", "I"] {
            assert!(!is_name_like(w), "{w}");
        }
    }
}
