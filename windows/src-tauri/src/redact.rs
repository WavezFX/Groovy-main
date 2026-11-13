// What may never reach a log (round 3G item 7). Keys and tokens live in the
// Windows Credential Manager (secrets.rs); the logs are for diagnosing, and a
// log that is pasted into an issue or a chat must not hold one. Every line
// written by log.rs goes through `redact` first: anything that looks like a
// provider's key, a bearer token, an Authorization header, a password, an OAuth
// code or a private key becomes `[hidden]`.
//
// No regex crate: a small scanner over the words of the line, so it costs
// nothing at start-up and reads in one place.

const HIDDEN: &str = "[hidden]";

/// Names whose value (after `=` or `:`) is a secret: query strings, forms,
/// headers, key=value pairs in a message and JSON-ish fields.
const SECRET_NAMES: &[&str] = &[
    "key", "api_key", "apikey", "api-key", "x-api-key", "x-goog-api-key", "token", "access_token", "refresh_token", "id_token",
    "auth", "authorization", "proxy-authorization", "bearer", "code", "client_secret", "secret", "password", "passwd", "pwd", "credential",
    "credentials", "private_key", "session", "cookie", "set-cookie", "signature",
];

/// `text` with every secret in it replaced by `[hidden]`.
pub fn redact(text: &str) -> String {
    let text = hide_private_keys(text);
    let mut out = String::with_capacity(text.len());
    // Whether the next word is a secret: Some(true) always ("Authorization:",
    // "password:", "Bearer"), Some(false) only when it looks like a value and
    // not like a status word ("key: set" is not a secret).
    let mut hide_next: Option<bool> = None;
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, hide_next: &mut Option<bool>| {
        if word.is_empty() {
            return;
        }
        match *hide_next {
            Some(always) if !is_plain_marker(word) => {
                *hide_next = None;
                if always || value_like(word) {
                    out.push_str(HIDDEN);
                    word.clear();
                    return;
                }
            }
            _ => {}
        }
        let (cleaned, wants_next) = redact_word(word);
        out.push_str(&cleaned);
        if wants_next.is_some() {
            *hide_next = wants_next;
        }
        word.clear();
    };
    for c in text.chars() {
        if is_delimiter(c) {
            flush(&mut word, &mut out, &mut hide_next);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out, &mut hide_next);
    out
}

fn is_delimiter(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | '|' | '`')
}

/// A word that only introduces the secret ("Bearer", "Basic", a lone colon).
fn is_plain_marker(word: &str) -> bool {
    matches!(word.to_ascii_lowercase().as_str(), "bearer" | "basic" | "token" | ":" | "=" | "is")
}

/// A word that could be a secret value: long enough, or mixing letters with digits or symbols.
fn value_like(word: &str) -> bool {
    word.len() >= 16 || (word.len() >= 8 && word.chars().any(|c| c.is_ascii_digit()) && word.chars().any(|c| c.is_ascii_alphabetic()))
}

/// Names whose next word is always the secret, whatever it looks like.
fn always_secret_name(name: &str) -> bool {
    let n = name.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-').to_ascii_lowercase();
    matches!(n.as_str(), "authorization" | "proxy-authorization" | "bearer" | "x-api-key" | "x-goog-api-key" | "cookie" | "set-cookie" | "password" | "passwd" | "pwd" | "client_secret" | "private_key")
}

/// One word, cleaned; and whether the *next* word is the secret ("Authorization:", "Bearer", "password:").
fn redact_word(word: &str) -> (String, Option<bool>) {
    let lower = word.to_ascii_lowercase();
    // "Bearer" / "Basic": the next word is the credential.
    if lower == "bearer" || lower == "basic" {
        return (word.to_string(), Some(true));
    }
    // name=value pieces joined by ? and & (a URL's query, a form body).
    if word.contains('=') || word.contains('?') || word.contains('&') {
        let mut out = String::with_capacity(word.len());
        let mut first = true;
        for piece in split_keep(word, &['?', '&']) {
            if first {
                first = false;
            }
            let (head, sep) = match piece.strip_suffix(['?', '&']) {
                Some(p) => (p, &piece[p.len()..]),
                None => (piece.as_str(), ""),
            };
            let cleaned = match head.split_once('=') {
                Some((name, value)) if is_secret_name(name) && !value.is_empty() => format!("{name}={HIDDEN}"),
                Some((name, value)) => format!("{name}={}", hide_secret_shaped(value)),
                None => hide_secret_shaped(head),
            };
            out.push_str(&cleaned);
            out.push_str(sep);
        }
        return (hide_userinfo(&out), None);
    }
    // "name:" then the value as the next word, or "name:value" in one word.
    if let Some(name) = word.strip_suffix(':') {
        if is_secret_name(name) {
            return (word.to_string(), Some(always_secret_name(name)));
        }
    }
    if let Some((name, value)) = word.split_once(':') {
        if is_secret_name(name) && !value.is_empty() && !value.starts_with("//") {
            return (format!("{name}:{HIDDEN}"), None);
        }
    }
    (hide_userinfo(&hide_secret_shaped(word)), None)
}

/// Splits after each separator, keeping it on the piece before: `a?b&c` → `a?`, `b&`, `c`.
fn split_keep(text: &str, seps: &[char]) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if seps.contains(&c) {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

fn is_secret_name(name: &str) -> bool {
    let n = name.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-').to_ascii_lowercase();
    if SECRET_NAMES.contains(&n.as_str()) {
        return true;
    }
    // client_secret, xi-api-key, OPENAI_API_KEY, github_token…
    n.ends_with("_key") || n.ends_with("-key") || n.ends_with("_token") || n.ends_with("-token") || n.ends_with("_secret") || n.ends_with("password")
}

/// `https://user:pass@host` → `https://user:[hidden]@host`.
fn hide_userinfo(word: &str) -> String {
    let Some(scheme_end) = word.find("://") else { return word.to_string() };
    let rest = &word[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let Some(at) = authority.rfind('@') else { return word.to_string() };
    let Some((user, _pass)) = authority[..at].split_once(':') else { return word.to_string() };
    format!("{}://{user}:{HIDDEN}@{}", &word[..scheme_end], &rest[at + 1..])
}

/// The word itself, or `[hidden]` when it has the shape of a provider's key or token.
fn hide_secret_shaped(word: &str) -> String {
    // Punctuation hugging the token ("key.", "(sk-…)" is split already) stays.
    let core_start = word.find(|c: char| c.is_ascii_alphanumeric()).unwrap_or(word.len());
    let core_end = word.rfind(|c: char| c.is_ascii_alphanumeric()).map(|i| i + 1).unwrap_or(core_start);
    if core_start >= core_end {
        return word.to_string();
    }
    let core = &word[core_start..core_end];
    if looks_like_secret(core) {
        format!("{}{HIDDEN}{}", &word[..core_start], &word[core_end..])
    } else {
        word.to_string()
    }
}

fn token_chars(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '/' | '='))
}

/// The key and token formats of the providers Groovy talks to, and the generic ones.
fn looks_like_secret(core: &str) -> bool {
    let has_digit = core.chars().any(|c| c.is_ascii_digit());
    let upper_alnum = |s: &str| s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let n = core.len();
    // (prefix, shortest whole length)
    const PREFIXES: &[(&str, usize)] = &[
        ("sk-ant-", 20),   // Anthropic
        ("sk-proj-", 20),  // OpenAI project keys
        ("sk-svcacct-", 20),
        ("sk-or-", 20),    // OpenRouter
        ("sk-", 24),       // OpenAI, DeepSeek and the compatible ones
        ("sk_live_", 20),  // Stripe
        ("sk_test_", 20),
        ("rk_live_", 20),
        ("rk_test_", 20),
        ("whsec_", 20),
        ("sk_", 28),       // ElevenLabs
        ("AIza", 35),      // Google
        ("ghp_", 30),      // GitHub
        ("gho_", 30),
        ("ghu_", 30),
        ("ghs_", 30),
        ("ghr_", 30),
        ("github_pat_", 30),
        ("glpat-", 20),    // GitLab
        ("xoxb-", 20),     // Slack
        ("xoxp-", 20),
        ("xoxa-", 20),
        ("xoxr-", 20),
        ("xoxs-", 20),
        ("xapp-", 20),
        ("hf_", 30),       // Hugging Face
        ("gsk_", 30),      // Groq
        ("napi_", 30),     // Neon
        ("re_", 24),       // Resend
        ("ntn_", 30),      // Notion
        ("secret_", 30),
        ("pplx-", 30),     // Perplexity
        ("nvapi-", 30),
        ("r8_", 30),       // Replicate
        ("fc-", 30),
        ("ya29.", 30),     // Google OAuth access tokens
        ("1//", 30),       // Google OAuth refresh tokens
        ("EAA", 40),       // Meta
        ("dop_v1_", 30),   // DigitalOcean
        ("npm_", 30),
        ("pypi-", 30),
    ];
    for (prefix, min) in PREFIXES {
        if core.starts_with(prefix) && n >= *min && token_chars(core) && (has_digit || n >= 40) {
            return true;
        }
    }
    // AWS access key ids.
    if (core.starts_with("AKIA") || core.starts_with("ASIA")) && n == 20 && upper_alnum(core) {
        return true;
    }
    // A JWT: three dot-separated base64url parts, the first a JSON object ("eyJ…").
    if core.starts_with("eyJ") && core.matches('.').count() == 2 && token_chars(core) && n >= 40 {
        return true;
    }
    false
}

/// A PEM private key block: from its BEGIN line to the end (or its END line).
fn hide_private_keys(text: &str) -> String {
    let Some(begin) = text.find("-----BEGIN ") else { return text.to_string() };
    let header_end = text[begin..].find("-----").and_then(|first| text[begin + first + 5..].find("-----").map(|second| begin + first + 5 + second + 5));
    let is_private = header_end.is_some_and(|end| text[begin..end].contains("PRIVATE KEY"));
    if !is_private {
        return text.to_string();
    }
    let after = match text[begin..].find("-----END ") {
        Some(end) => text[begin + end + 9..].find("-----").map(|close| begin + end + 9 + close + 5).unwrap_or(text.len()),
        None => text.len(),
    };
    format!("{}{HIDDEN}{}", &text[..begin], hide_private_keys(&text[after..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fake keys in each provider's own format, none of them real. Each is
    // written as its prefix and the rest, and put together when the test runs,
    // so no secret-shaped text sits in the source for scanners to find.
    const FAKES: &[(&str, &str, &str)] = &[
        ("Anthropic", "sk-", "ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-AbCdEfGhIjKlMnOp"),
        ("OpenAI", "sk-p", "roj-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEf"),
        ("OpenAI classic", "s", "k-AbCdEfGhIjKlMnOpQrStUvWx0123456789"),
        ("Google", "AI", "zaSyA1b2C3d4E5f6G7h8I9j0KlMnOpQrStUvW"),
        ("GitHub", "gh", "p_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"),
        ("GitHub fine-grained", "githu", "b_pat_11ABCDEFG0AbCdEfGhIjKl_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"),
        ("Slack", "xo", "xb-1234567890-0987654321-AbCdEfGhIjKlMnOpQrStUvWx"),
        ("AWS", "AK", "IAIOSFODNN7EXAMPLE"),
        ("Stripe", "sk_l", "ive_AbCdEfGhIjKlMnOpQrStUv0123456789"),
        ("Stripe restricted", "rk_l", "ive_AbCdEfGhIjKlMnOpQrStUv0123456789"),
        ("Resend", "r", "e_AbCdEfGh_IjKlMnOpQrStUvWxYz0123456789"),
        ("Hugging Face", "h", "f_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"),
        ("Groq", "gs", "k_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEfGhIj"),
        ("Neon", "na", "pi_abcdefghijklmnopqrstuvwxyz0123456789abcdefghijkl"),
        ("ElevenLabs", "s", "k_0123456789abcdef0123456789abcdef0123456789abcdef"),
        ("Notion", "nt", "n_0123456789AbCdEfGhIjKlMnOpQrStUvWxYz01234567"),
        ("JWT", "e", "yJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4ifQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"),
        ("Google OAuth access", "ya", "29.a0AfH6SMBAbCdEfGhIjKlMnOpQrStUvWxYz0123456789"),
    ];

    #[test]
    fn a_key_of_each_provider_never_reaches_the_log() {
        for (provider, head, tail) in FAKES {
            let key = format!("{head}{tail}");
            let key = key.as_str();
            for shape in [format!("{key}"), format!("request failed with {key} in it"), format!("(key {key})"), format!("\"token\":\"{key}\""), format!("url https://api.example.com/v1?x=1&foo={key}")] {
                let out = redact(&shape);
                assert!(!out.contains(key), "{provider}: {out}");
                assert!(out.contains(HIDDEN), "{provider}: {out}");
            }
        }
    }

    #[test]
    fn headers_passwords_codes_and_private_keys_are_hidden() {
        for (input, secret) in [
            ("Authorization: Bearer abc123def456ghi789", "abc123def456ghi789"),
            ("authorization: Basic dXNlcjpwYXNzd29yZA==", "dXNlcjpwYXNzd29yZA=="),
            ("x-api-key: 0123456789abcdef0123456789abcdef", "0123456789abcdef0123456789abcdef"),
            ("xi-api-key=0123456789abcdef", "0123456789abcdef"),
            ("GET /callback?code=4/0AbCdEfGh&state=xyz", "4/0AbCdEfGh"),
            ("refresh_token=1//0gAbCdEfGhIjKlMn", "1//0gAbCdEfGhIjKlMn"),
            ("imap login password=hunter2 failed", "hunter2"),
            ("client_secret: s3cr3t-value-here", "s3cr3t-value-here"),
            ("smtp://alice:correct-horse@smtp.example.com:587/", "correct-horse"),
            ("-----BEGIN RSA PRIVATE KEY-----\nMIIBOgIBAAJBAKj34GkxFhD90vcNLYLInFEX6Ppy1tPf9Cnzj4p4WGeKLs1Pt8Qu\n-----END RSA PRIVATE KEY-----", "MIIBOgIBAAJBAKj34GkxFhD90vcNLYLInFEX6Ppy1tPf9Cnzj4p4WGeKLs1Pt8Qu"),
        ] {
            let out = redact(input);
            assert!(!out.contains(secret), "{input} → {out}");
            assert!(out.contains(HIDDEN), "{input} → {out}");
        }
    }

    #[test]
    fn ordinary_lines_are_left_alone() {
        for line in [
            "boot: interface loaded (0.6.0)",
            "load: kokoro (the offline voice)",
            "Kokoro loaded in 1.395098s",
            "memory: 458 MB, making room for speech (+240 MB) by freeing the other model: 108 MB",
            "downloaded and verified: llama.cpp (18434702 bytes)",
            "sha256 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08 ok",
            "flow: 12 words in 1.2s",
            "Fish Audio key: set",
            "token: expired",
            "code: 404 not found",
            "the key is saved in the Windows Credential Manager",
            "spoke with re_open and sk-ip them",
            "https://api.github.com/repos/WavezFX/Groovy-main/actions/runs?per_page=4",
            "mail: 2 inboxes, last checked 10:41",
        ] {
            assert_eq!(redact(line), line);
        }
    }

    #[test]
    fn what_is_written_to_the_log_is_redacted() {
        let dir = std::env::temp_dir().join(format!("groovy-redact-{}", std::process::id()));
        crate::log::append_to(&dir, "test.log", &format!("calling with {}{} now", "sk-an", "t-api03-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"));
        let written = std::fs::read_to_string(dir.join("test.log")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written.contains("calling with [hidden] now"), "{written}");
        assert!(!written.contains("sk-ant"));
    }
}
