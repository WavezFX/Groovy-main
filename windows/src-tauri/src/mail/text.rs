// Mail headers as people read them: RFC 2047 encoded words decoded, folded
// lines unfolded, "Name <address>" cut to the name, and the dates IMAP and the
// web APIs give turned into Unix seconds. Pure functions, tested below.

use base64::Engine;

/// "Subject: =?UTF-8?B?…?=\r\n continued" → the header's value, unfolded and decoded.
pub fn header(raw: &str, name: &str) -> Option<String> {
    let unfolded = raw.replace("\r\n ", " ").replace("\r\n\t", " ").replace("\n ", " ").replace("\n\t", " ");
    let prefix = format!("{}:", name.to_ascii_lowercase());
    unfolded
        .lines()
        .map(str::trim_start)
        .find(|l| l.to_ascii_lowercase().starts_with(&prefix))
        .map(|l| decode_words(l[prefix.len()..].trim()))
}

/// Decodes every `=?charset?B|Q?text?=` in `s`; whitespace between two encoded words goes.
pub fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut last_was_word = false;
    while let Some(start) = rest.find("=?") {
        let (before, tail) = rest.split_at(start);
        let Some((word, after)) = encoded_word(tail) else {
            out.push_str(before);
            out.push_str("=?");
            rest = &tail[2..];
            last_was_word = false;
            continue;
        };
        if !(last_was_word && before.trim().is_empty()) {
            out.push_str(before);
        }
        out.push_str(&word);
        rest = after;
        last_was_word = true;
    }
    out.push_str(rest);
    out
}

fn encoded_word(s: &str) -> Option<(String, &str)> {
    let body = s.strip_prefix("=?")?;
    let q1 = body.find('?')?;
    let charset = &body[..q1];
    let enc = body.get(q1 + 1..q1 + 2)?;
    if body.get(q1 + 2..q1 + 3)? != "?" {
        return None;
    }
    let text_start = q1 + 3;
    let end = body[text_start..].find("?=")? + text_start;
    let text = &body[text_start..end];
    let bytes = match enc {
        "B" | "b" => base64::engine::general_purpose::STANDARD_NO_PAD.decode(text.trim_end_matches('=')).ok()?,
        "Q" | "q" => quoted(text),
        _ => return None,
    };
    Some((charset_decode(charset, &bytes), &body[end + 2..]))
}

fn quoted(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'_' => out.push(b' '),
            b'=' if i + 2 < b.len() => {
                match (b.get(i + 1).and_then(|c| (*c as char).to_digit(16)), b.get(i + 2).and_then(|c| (*c as char).to_digit(16))) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 2;
                    }
                    _ => out.push(b'='),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// UTF-8 and ASCII as they are; Latin-1 and Windows-1252 byte by byte (the
/// 1252 extras as their Latin-1 code points, close enough for a subject line).
pub fn charset_decode(charset: &str, bytes: &[u8]) -> String {
    let c = charset.to_ascii_lowercase();
    let c = c.split('*').next().unwrap_or("");
    if c == "iso-8859-1" || c == "latin1" || c == "windows-1252" || c == "cp1252" {
        return bytes.iter().map(|b| *b as char).collect();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// "Anna Smith <anna@example.com>" → "Anna Smith"; a bare address stays.
pub fn sender_name(from: &str) -> String {
    let from = from.trim();
    if let Some(lt) = from.find('<') {
        let name = from[..lt].trim().trim_matches('"').trim();
        if !name.is_empty() {
            return name.to_string();
        }
        return from[lt + 1..].trim_end_matches('>').trim().to_string();
    }
    from.trim_matches('"').to_string()
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// IMAP's INTERNALDATE: "17-Oct-2026 09:05:00 +0200" → Unix seconds.
pub fn imap_date(s: &str) -> Option<i64> {
    let mut parts = s.trim().split_whitespace();
    let date = parts.next()?;
    let time = parts.next()?;
    let zone = parts.next().unwrap_or("+0000");
    let mut d = date.split('-');
    let day: u32 = d.next()?.trim().parse().ok()?;
    let mon = d.next()?;
    let month = MONTHS.iter().position(|m| mon.eq_ignore_ascii_case(m))? as u32 + 1;
    let year: i64 = d.next()?.parse().ok()?;
    let mut t = time.split(':').map(|x| x.parse::<i64>().ok());
    let (h, mi, se) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let z = zone.trim_start_matches(['+', '-']);
    let off = sign * (z.get(0..2)?.parse::<i64>().ok()? * 3600 + z.get(2..4)?.parse::<i64>().ok()? * 60);
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + mi * 60 + se - off)
}

/// Graph's "2026-10-17T09:05:00Z" → Unix seconds.
pub fn iso_date(s: &str) -> Option<i64> {
    let (date, time) = s.trim().split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.trim_end_matches('Z');
    let time = time.split(['+', '.']).next()?;
    let mut t = time.split(':').map(|x| x.parse::<i64>().ok());
    let (h, mi, se) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    Some(days_from_civil(y, m as u32, day as u32) * 86_400 + h * 3600 + mi * 60 + se)
}

/// For form bodies and query strings: everything but unreserved characters escaped.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v))).collect::<Vec<_>>().join("&")
}

/// The value of `key` in a query string ("code=…&state=…"), percent-decoded.
pub fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| url_decode(v))
    })
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_headers_are_read_as_people_read_them() {
        assert_eq!(decode_words("=?UTF-8?B?Q2Fmw6kgw6AgOWg=?="), "Café à 9h");
        assert_eq!(decode_words("=?iso-8859-1?Q?R=E9union_demain?="), "Réunion demain");
        assert_eq!(decode_words("=?UTF-8?Q?a?= =?UTF-8?Q?b?= plain"), "ab plain");
        assert_eq!(decode_words("no =? word here"), "no =? word here");
        let raw = "From: =?UTF-8?Q?Ren=C3=A9e?= <renee@example.com>\r\nSubject: Hello\r\n there\r\n";
        assert_eq!(header(raw, "Subject").as_deref(), Some("Hello there"));
        assert_eq!(sender_name(&header(raw, "from").unwrap()), "Renée");
        assert_eq!(sender_name("<bob@example.com>"), "bob@example.com");
        assert_eq!(sender_name("\"GitHub\" <noreply@github.com>"), "GitHub");
    }

    #[test]
    fn dates_from_imap_and_graph() {
        assert_eq!(imap_date("17-Oct-2026 09:05:00 +0200"), Some(1_792_220_700));
        assert_eq!(imap_date(" 7-Oct-2026 07:05:00 +0000"), Some(1_792_220_700 - 10 * 86_400));
        assert_eq!(iso_date("2026-10-17T07:05:00Z"), Some(1_792_220_700));
        assert_eq!(iso_date("2026-10-17T07:05:00.123+00:00"), Some(1_792_220_700));
        assert_eq!(imap_date("nonsense"), None);
    }

    #[test]
    fn query_strings_and_forms() {
        assert_eq!(query_value("code=4%2F0Ab&state=xyz&scope=a+b", "code").as_deref(), Some("4/0Ab"));
        assert_eq!(query_value("code=1&state=x", "missing"), None);
        assert_eq!(form(&[("redirect_uri", "http://127.0.0.1:5/"), ("a", "b c")]), "redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2F&a=b%20c");
    }
}
