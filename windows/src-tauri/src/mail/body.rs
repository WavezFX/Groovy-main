// Reading one email and answering it (round 3D item 8: the Mail tile grown
// in the island). A message's text is fetched only when the user opens it,
// is shown once and kept nowhere: not in the inbox cache, not in a log.
//
// Pure functions, tested below: the readable text of a MIME message (the
// plain part, or the HTML one turned to text), and the RFC 5322 reply.

use base64::Engine;

use super::text;

/// What a reply needs from the message it answers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Original {
    /// Where the answer goes: Reply-To, else From (the address only).
    pub to: String,
    pub subject: String,
    pub message_id: String,
    pub references: String,
}

/// The longest text the island shows of one message.
pub const MAX_TEXT: usize = 20_000;

/// From a whole message's headers: who to answer and how to thread it.
pub fn original(headers: &str) -> Original {
    let to = text::header(headers, "Reply-To").or_else(|| text::header(headers, "From")).map(|a| address(&a)).unwrap_or_default();
    Original {
        to,
        subject: text::header(headers, "Subject").unwrap_or_default(),
        message_id: text::header(headers, "Message-ID").unwrap_or_default().trim().to_string(),
        references: text::header(headers, "References").unwrap_or_default().trim().to_string(),
    }
}

/// "Anna <anna@example.com>" → "anna@example.com"; a bare address stays.
pub fn address(from: &str) -> String {
    let f = from.trim();
    match (f.rfind('<'), f.rfind('>')) {
        (Some(a), Some(b)) if b > a => f[a + 1..b].trim().to_string(),
        _ => f.trim_matches('"').to_string(),
    }
}

/// "Re: " once, however many times it was answered already.
pub fn re(subject: &str) -> String {
    let s = subject.trim();
    if s.len() >= 3 && s[..3].eq_ignore_ascii_case("re:") {
        s.to_string()
    } else {
        format!("Re: {s}")
    }
}

/// A header value that can't carry a line break into the next header.
fn one_line(s: &str) -> String {
    s.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect()
}

/// A header value as an RFC 2047 word when it isn't plain ASCII.
fn encode_word(s: &str) -> String {
    if s.is_ascii() {
        one_line(s)
    } else {
        format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(one_line(s)))
    }
}

/// The reply as an RFC 5322 message, in UTF-8 (base64 body), threaded under the original.
pub fn reply_message(from: &str, o: &Original, body: &str, date: &str, id: &str) -> String {
    let mut m = String::new();
    m.push_str(&format!("From: {}\r\n", one_line(from)));
    m.push_str(&format!("To: {}\r\n", one_line(&o.to)));
    m.push_str(&format!("Subject: {}\r\n", encode_word(&re(&o.subject))));
    m.push_str(&format!("Date: {}\r\n", one_line(date)));
    m.push_str(&format!("Message-ID: {}\r\n", one_line(id)));
    if !o.message_id.is_empty() {
        m.push_str(&format!("In-Reply-To: {}\r\n", one_line(&o.message_id)));
        let refs = format!("{} {}", o.references, o.message_id);
        m.push_str(&format!("References: {}\r\n", one_line(refs.trim())));
    }
    m.push_str("MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n");
    let encoded = base64::engine::general_purpose::STANDARD.encode(body.replace("\r\n", "\n").replace('\n', "\r\n"));
    for chunk in encoded.as_bytes().chunks(76) {
        m.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        m.push_str("\r\n");
    }
    m
}

/// Now as an RFC 5322 date, in UTC.
pub fn date_now() -> String {
    date(crate::db::now())
}

pub fn date(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{}, {d:02} {} {y} {:02}:{:02}:{:02} +0000", DAYS[days.rem_euclid(7) as usize], MONTHS[(m - 1) as usize], secs / 3600, secs / 60 % 60, secs % 60)
}

/// A Message-ID on the sender's own domain.
pub fn new_id(from: &str) -> String {
    let domain = address(from).rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_else(|| "groovy.local".into());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("<groovy.{nanos:x}@{domain}>")
}

/// The readable text of a whole MIME message (headers and body).
pub fn readable(raw: &[u8]) -> String {
    let (headers, body) = split(raw);
    let text = part_text(&headers, body).unwrap_or_default();
    tidy(&text)
}

fn split(raw: &[u8]) -> (String, &[u8]) {
    let find = |pat: &[u8]| raw.windows(pat.len()).position(|w| w == pat);
    match find(b"\r\n\r\n").map(|i| (i, 4)).or_else(|| find(b"\n\n").map(|i| (i, 2))) {
        Some((i, n)) => (String::from_utf8_lossy(&raw[..i]).into_owned(), &raw[i + n..]),
        None => (String::from_utf8_lossy(raw).into_owned(), &[]),
    }
}

/// `text/plain; charset="utf-8"` → its parameter `name`.
fn param(value: &str, name: &str) -> Option<String> {
    value.split(';').skip(1).find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim().eq_ignore_ascii_case(name)).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// A part's text: the plain alternative first, else the HTML one as text.
fn part_text(headers: &str, body: &[u8]) -> Option<String> {
    let ctype = text::header(headers, "Content-Type").unwrap_or_else(|| "text/plain".into());
    let kind = ctype.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if kind.starts_with("multipart/") {
        let boundary = param(&ctype, "boundary")?;
        let parts = parts(body, &boundary);
        let texts: Vec<(String, String)> = parts
            .iter()
            .filter_map(|p| {
                let (h, b) = split(p);
                let k = text::header(&h, "Content-Type").unwrap_or_else(|| "text/plain".into()).to_ascii_lowercase();
                let attachment = text::header(&h, "Content-Disposition").is_some_and(|d| d.to_ascii_lowercase().starts_with("attachment"));
                if attachment {
                    return None;
                }
                part_text(&h, b).map(|t| (k, t))
            })
            .collect();
        return texts
            .iter()
            .find(|(k, _)| k.starts_with("text/plain"))
            .or_else(|| texts.first())
            .map(|(_, t)| t.clone());
    }
    if !kind.starts_with("text/") {
        return None;
    }
    let bytes = decode_transfer(&text::header(headers, "Content-Transfer-Encoding").unwrap_or_default(), body);
    let s = text::charset_decode(&param(&ctype, "charset").unwrap_or_else(|| "utf-8".into()), &bytes);
    Some(if kind == "text/html" { html_text(&s) } else { s })
}

fn parts<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let marker = format!("--{boundary}");
    let m = marker.as_bytes();
    let mut starts = Vec::new();
    let mut i = 0;
    while i + m.len() <= body.len() {
        if &body[i..i + m.len()] == m && (i == 0 || body[i - 1] == b'\n') {
            starts.push(i);
            i += m.len();
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for w in starts.windows(2) {
        let from = w[0] + m.len();
        let part = &body[from..w[1]];
        let part = part.strip_prefix(b"\r\n").or_else(|| part.strip_prefix(b"\n")).unwrap_or(part);
        out.push(part);
    }
    out
}

fn decode_transfer(encoding: &str, body: &[u8]) -> Vec<u8> {
    match encoding.trim().to_ascii_lowercase().as_str() {
        "base64" => {
            let clean: Vec<u8> = body.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
            base64::engine::general_purpose::STANDARD.decode(&clean).unwrap_or_default()
        }
        "quoted-printable" => quoted_printable(body),
        _ => body.to_vec(),
    }
}

fn quoted_printable(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            // A soft line break.
            if b.get(i + 1) == Some(&b'\r') && b.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if b.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            let hex = |c: Option<&u8>| c.and_then(|c| (*c as char).to_digit(16));
            if let (Some(h), Some(l)) = (hex(b.get(i + 1)), hex(b.get(i + 2))) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// HTML as text: no tags, scripts or styles; paragraphs and breaks as new lines.
pub fn html_text(html: &str) -> String {
    let mut out = String::new();
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    let b = html.as_bytes();
    while i < b.len() {
        if b[i] == b'<' {
            let close = lower[i..].find('>').map(|n| i + n).unwrap_or(b.len() - 1);
            let tag = &lower[i + 1..close];
            let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if !tag.starts_with('/') && (name == "script" || name == "style" || name == "head") {
                let end = format!("</{name}");
                i = lower[close..].find(&end).map(|n| close + n).and_then(|n| lower[n..].find('>').map(|m| n + m + 1)).unwrap_or(b.len());
                continue;
            }
            if matches!(name.as_str(), "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "table") {
                out.push('\n');
            }
            i = close + 1;
            continue;
        }
        let next = lower[i..].find('<').map(|n| i + n).unwrap_or(b.len());
        out.push_str(&entities(&html[i..next]));
        i = next;
    }
    out
}

fn entities(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// Line ends as \n, no more than one empty line in a row, trimmed, capped.
pub fn tidy(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank > 1 || out.is_empty() {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(l);
        out.push('\n');
    }
    let t = out.trim().to_string();
    if t.chars().count() > MAX_TEXT {
        t.chars().take(MAX_TEXT).collect::<String>() + "…"
    } else {
        t
    }
}

/// Gmail's `format=full` payload: the plain text part, else the HTML one as text.
pub fn gmail_text(payload: &serde_json::Value) -> String {
    fn walk(p: &serde_json::Value, want: &str) -> Option<String> {
        let mime = p["mimeType"].as_str().unwrap_or("");
        if mime == want {
            let data = p["body"]["data"].as_str()?;
            let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).ok()?;
            return Some(String::from_utf8_lossy(&bytes).into_owned());
        }
        p["parts"].as_array()?.iter().find_map(|c| walk(c, want))
    }
    let text = walk(payload, "text/plain").or_else(|| walk(payload, "text/html").map(|h| html_text(&h))).unwrap_or_default();
    tidy(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plain_part_of_an_alternative_message() {
        let raw = b"From: Ava <ava@example.com>\r\nContent-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nAre we still on for Thursday at 6? Caf=C3=A9 =\r\nnearby.\r\n--b1\r\nContent-Type: text/html\r\n\r\n<p>HTML</p>\r\n--b1--\r\n";
        assert_eq!(readable(raw), "Are we still on for Thursday at 6? Café nearby.");
    }

    #[test]
    fn html_only_mail_reads_as_text_and_attachments_are_skipped() {
        let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/html; charset=iso-8859-1\r\nContent-Transfer-Encoding: base64\r\n\r\nPGh0bWw+PGhlYWQ+PHN0eWxlPnB7fTwvc3R5bGU+PC9oZWFkPjxib2R5PjxwPkhpICZhbXA7IGJ5ZTwvcD48YnI+PHA+Q2Fm6TwvcD48L2JvZHk+PC9odG1sPg==\r\n--x\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=a.txt\r\n\r\nsecret file\r\n--x--\r\n";
        assert_eq!(readable(raw), "Hi & bye\n\nCafé");
    }

    #[test]
    fn a_reply_threads_under_the_original_and_cant_inject_headers() {
        let o = original("From: Ava Chen <ava@example.com>\r\nSubject: Thursday plan?\r\nMessage-ID: <m1@example.com>\r\nReferences: <m0@example.com>\r\n");
        assert_eq!(o.to, "ava@example.com");
        let m = reply_message("me@icloud.com", &o, "Sounds good,\nsee you Thursday.", "Sat, 24 Oct 2026 09:00:00 +0000", "<r1@groovy>");
        assert!(m.contains("To: ava@example.com\r\nSubject: Re: Thursday plan?\r\n"));
        assert!(m.contains("In-Reply-To: <m1@example.com>\r\nReferences: <m0@example.com> <m1@example.com>\r\n"));
        let body = m.split("\r\n\r\n").nth(1).unwrap().replace("\r\n", "");
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(body).unwrap(), b"Sounds good,\r\nsee you Thursday.");
        let evil = Original { to: "a@x.com\r\nBcc: all@x.com".into(), subject: "Re: hi".into(), ..Default::default() };
        let m = reply_message("me@x.com", &evil, "x", "d", "<i>");
        assert!(!m.contains("\r\nBcc:"), "a line break can't start a new header");
        assert!(m.contains("Subject: Re: hi\r\n"), "Re: isn't doubled");
        assert!(reply_message("me@x.com", &Original { subject: "Café".into(), ..Default::default() }, "x", "d", "<i>").contains("Subject: =?UTF-8?B?"));
    }

    #[test]
    fn dates_and_ids() {
        assert_eq!(date(1_792_220_700), "Sat, 17 Oct 2026 07:05:00 +0000");
        assert_eq!(date(0), "Thu, 01 Jan 1970 00:00:00 +0000");
        assert!(new_id("Me <me@icloud.com>").ends_with("@icloud.com>"));
    }

    #[test]
    fn gmail_payloads_give_their_text() {
        let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("Hello there\r\n\r\n\r\n\r\nBye");
        let p = serde_json::json!({ "mimeType": "multipart/alternative", "parts": [
            { "mimeType": "text/html", "body": { "data": "PGI-SGk8L2I-" } },
            { "mimeType": "text/plain", "body": { "data": data } },
        ]});
        assert_eq!(gmail_text(&p), "Hello there\n\nBye");
        assert_eq!(gmail_text(&serde_json::json!({ "mimeType": "text/html", "body": { "data": "PGI-SGk8L2I-" } })), "Hi");
    }
}
