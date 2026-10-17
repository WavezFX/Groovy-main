// A small IMAP4rev1 client over TLS (port 993), for iCloud Mail and any other
// IMAP server: log in, read the newest headers in INBOX without marking them
// read (BODY.PEEK), count the unread, mark one read and move one to Archive.
// Nothing else: Groovy never downloads a message body.
//
// What the user typed never reaches the server unquoted: every string goes
// through `quote`, which refuses line breaks and escapes \ and ".

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::version::{TLS12, TLS13};
use tokio_rustls::rustls::{crypto::ring as ring_provider, ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use super::text;
use crate::i18n::{t, tf};

/// Anything a server sends back in one answer is far below this.
const MAX_LITERAL: usize = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub struct Session {
    io: BufReader<Box<dyn Io>>,
    tag: u32,
}

/// One message's headers, as read with BODY.PEEK.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub uid: u32,
    pub seen: bool,
    pub at: i64,
    pub from: String,
    pub subject: String,
}

/// An IMAP quoted string, or an error for text that can't be one.
pub fn quote(s: &str) -> Result<String, String> {
    if s.contains(['\r', '\n', '\0']) || !s.is_ascii() {
        return Err(t("Use only plain letters, digits and symbols here (no accents or line breaks)."));
    }
    Ok(format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
}

fn tls() -> Arc<ClientConfig> {
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(
        ClientConfig::builder_with_provider(Arc::new(ring_provider::default_provider()))
            .with_protocol_versions(&[&TLS13, &TLS12])
            .expect("the ring provider supports TLS 1.2 and 1.3")
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

impl Session {
    pub async fn connect(host: &str, port: u16) -> Result<Session, String> {
        let unreachable = || tf("Couldn't reach {host}.", &[("host", host)]);
        let tcp = tokio::time::timeout(TIMEOUT, TcpStream::connect((host, port))).await.map_err(|_| unreachable())?.map_err(|_| unreachable())?;
        let name = ServerName::try_from(host.to_string()).map_err(|_| unreachable())?;
        let stream = tokio::time::timeout(TIMEOUT, TlsConnector::from(tls()).connect(name, tcp))
            .await
            .map_err(|_| unreachable())?
            .map_err(|e| tf("A secure connection to {host} failed: {error}", &[("host", host), ("error", &e.to_string())]))?;
        let mut s = Session::over(Box::new(stream));
        let greeting = s.line().await?;
        if !greeting.starts_with("* OK") && !greeting.starts_with("* PREAUTH") {
            return Err(unreachable());
        }
        Ok(s)
    }

    pub fn over(io: Box<dyn Io>) -> Session {
        Session { io: BufReader::new(io), tag: 0 }
    }

    async fn line(&mut self) -> Result<String, String> {
        let mut buf = Vec::new();
        let n = tokio::time::timeout(TIMEOUT, (&mut self.io).take(MAX_LITERAL as u64).read_until(b'\n', &mut buf))
            .await
            .map_err(|_| t("The mail server stopped answering."))?
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err(t("The mail server closed the connection."));
        }
        Ok(String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string())
    }

    /// Sends one command and reads to its tagged answer. The untagged lines
    /// come back with their literals ({n} + n bytes) put in their place.
    pub async fn run(&mut self, command: &str) -> Result<Vec<String>, String> {
        self.tag += 1;
        let tag = format!("g{}", self.tag);
        self.io.get_mut().write_all(format!("{tag} {command}\r\n").as_bytes()).await.map_err(|e| e.to_string())?;
        self.io.get_mut().flush().await.map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        loop {
            let mut line = self.line().await?;
            // A literal: "{123}" at the end, then that many bytes, then the rest of the line.
            while let Some(n) = literal_len(&line) {
                if n > MAX_LITERAL {
                    return Err(t("The server's answer is too large."));
                }
                let mut lit = vec![0u8; n];
                tokio::time::timeout(TIMEOUT, self.io.read_exact(&mut lit)).await.map_err(|_| t("The mail server stopped answering."))?.map_err(|e| e.to_string())?;
                line.truncate(line.rfind('{').unwrap_or(line.len()));
                line.push_str(&String::from_utf8_lossy(&lit));
                line.push_str(&self.line().await?);
            }
            if let Some(rest) = line.strip_prefix(&format!("{tag} ")) {
                if rest.starts_with("OK") {
                    return Ok(out);
                }
                let why = rest.splitn(2, ' ').nth(1).unwrap_or(rest).trim();
                return Err(if rest.contains("AUTHENTICATIONFAILED") || command.starts_with("LOGIN") {
                    t("The mail server didn't accept that address and password.")
                } else {
                    tf("The mail server said: {reason}", &[("reason", why)])
                });
            }
            out.push(line);
        }
    }

    pub async fn login(&mut self, user: &str, password: &str) -> Result<(), String> {
        self.run(&format!("LOGIN {} {}", quote(user)?, quote(password)?)).await.map(|_| ())
    }

    /// Selects INBOX; the number of messages in it.
    pub async fn inbox(&mut self) -> Result<u32, String> {
        let lines = self.run("SELECT INBOX").await?;
        Ok(lines.iter().find_map(|l| l.strip_prefix("* ")?.strip_suffix(" EXISTS")?.parse().ok()).unwrap_or(0))
    }

    pub async fn unread(&mut self) -> Result<u32, String> {
        let lines = self.run("UID SEARCH UNSEEN").await?;
        Ok(lines.iter().filter_map(|l| l.strip_prefix("* SEARCH")).map(|r| r.split_whitespace().count() as u32).sum())
    }

    /// The newest `n` messages' headers, newest first, without marking them read.
    pub async fn newest(&mut self, exists: u32, n: u32) -> Result<Vec<Header>, String> {
        if exists == 0 {
            return Ok(Vec::new());
        }
        let from = exists.saturating_sub(n - 1).max(1);
        let lines = self.run(&format!("FETCH {from}:{exists} (UID FLAGS INTERNALDATE BODY.PEEK[HEADER.FIELDS (FROM SUBJECT)])")).await?;
        let mut out: Vec<Header> = lines.iter().filter_map(|l| parse_fetch(l)).collect();
        out.sort_by(|a, b| b.uid.cmp(&a.uid));
        Ok(out)
    }

    pub async fn mark_read(&mut self, uid: u32) -> Result<(), String> {
        self.run(&format!("UID STORE {uid} +FLAGS.SILENT (\\Seen)")).await.map(|_| ())
    }

    /// Moves it to the mailbox marked \Archive, or the one called Archive.
    pub async fn archive(&mut self, uid: u32) -> Result<(), String> {
        let list = self.run("LIST \"\" \"*\"").await?;
        let target = list
            .iter()
            .find(|l| l.contains("\\Archive"))
            .or_else(|| list.iter().find(|l| l.trim_end().trim_end_matches('"').ends_with("Archive")))
            .and_then(|l| mailbox_name(l))
            .ok_or_else(|| t("This account has no Archive folder."))?;
        self.run(&format!("UID MOVE {uid} {}", quote(&target)?)).await.map(|_| ())
    }

    pub async fn logout(mut self) {
        let _ = self.run("LOGOUT").await;
    }
}

fn literal_len(line: &str) -> Option<usize> {
    let open = line.rfind('{')?;
    line.strip_suffix('}')?[open + 1..].trim_end_matches('+').parse().ok()
}

/// `* LIST (\HasNoChildren \Archive) "/" "Archive"` → "Archive".
fn mailbox_name(line: &str) -> Option<String> {
    let after = line.rsplit_once(") ")?.1;
    let name = after.split_once(' ').map(|(_, n)| n).unwrap_or(after).trim();
    Some(name.trim_matches('"').replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// `* 12 FETCH (UID 345 FLAGS (\Seen) INTERNALDATE "…" BODY[HEADER.FIELDS (FROM SUBJECT)] From: …)`.
pub fn parse_fetch(line: &str) -> Option<Header> {
    if !line.starts_with("* ") || !line.contains(" FETCH (") {
        return None;
    }
    let uid = after_word(line, "UID ")?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    let flags = after_word(line, "FLAGS (").and_then(|r| r.split(')').next()).unwrap_or("");
    let at = after_word(line, "INTERNALDATE \"").and_then(|r| r.split('"').next()).and_then(text::imap_date).unwrap_or(0);
    let headers = line.find("]").map(|i| &line[i + 1..]).unwrap_or("");
    Some(Header {
        uid,
        seen: flags.contains("\\Seen"),
        at,
        from: text::header(headers, "From").map(|f| text::sender_name(&f)).unwrap_or_default(),
        subject: text::header(headers, "Subject").unwrap_or_default(),
    })
}

fn after_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    line.find(word).map(|i| &line[i + word.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_text_is_quoted_never_spliced() {
        assert_eq!(quote("a\"b\\c").unwrap(), "\"a\\\"b\\\\c\"");
        assert!(quote("x\r\nA2 DELETE INBOX").is_err());
        assert!(quote("mot de passé").is_err());
    }

    #[test]
    fn fetch_lines_become_headers() {
        let line = "* 12 FETCH (UID 345 FLAGS (\\Seen \\Flagged) INTERNALDATE \"17-Oct-2026 09:05:00 +0200\" BODY[HEADER.FIELDS (FROM SUBJECT)] From: \"Anna\" <anna@example.com>\r\nSubject: =?UTF-8?Q?Caf=C3=A9?=\r\n\r\n)";
        let h = parse_fetch(line).unwrap();
        assert_eq!(h, Header { uid: 345, seen: true, at: 1_792_220_700, from: "Anna".into(), subject: "Café".into() });
        assert!(parse_fetch("* 3 EXISTS").is_none());
        assert_eq!(mailbox_name("* LIST (\\HasNoChildren \\Archive) \"/\" \"Archive\"").as_deref(), Some("Archive"));
        assert_eq!(literal_len("* 1 FETCH (BODY[] {42}"), Some(42));
    }

    #[tokio::test]
    async fn a_session_against_a_scripted_server() {
        let (client, mut server) = tokio::io::duplex(64 * 1024);
        let script = tokio::spawn(async move {
            let mut r = BufReader::new(&mut server);
            let mut seen = Vec::new();
            let answers = [
                "g1 OK LOGIN done\r\n".to_string(),
                "* 2 EXISTS\r\ng2 OK [READ-WRITE] SELECT\r\n".to_string(),
                "* SEARCH 8\r\ng3 OK\r\n".to_string(),
                format!("* 1 FETCH (UID 7 FLAGS (\\Seen) INTERNALDATE \"17-Oct-2026 09:05:00 +0200\" BODY[HEADER.FIELDS (FROM SUBJECT)] {{{}}}\r\nFrom: a@x.com\r\nSubject: One\r\n\r\n)\r\n* 2 FETCH (UID 8 FLAGS () INTERNALDATE \"17-Oct-2026 10:05:00 +0200\" BODY[HEADER.FIELDS (FROM SUBJECT)] {{{}}}\r\nFrom: Bob <b@x.com>\r\nSubject: Two\r\n\r\n)\r\ng4 OK\r\n", "From: a@x.com\r\nSubject: One\r\n\r\n".len(), "From: Bob <b@x.com>\r\nSubject: Two\r\n\r\n".len()),
            ];
            for answer in answers {
                let mut line = String::new();
                r.read_line(&mut line).await.unwrap();
                seen.push(line);
                r.get_mut().write_all(answer.as_bytes()).await.unwrap();
            }
            seen
        });
        let mut s = Session::over(Box::new(client));
        s.login("me@icloud.com", "abcd-efgh").await.unwrap();
        let exists = s.inbox().await.unwrap();
        assert_eq!(exists, 2);
        assert_eq!(s.unread().await.unwrap(), 1);
        let list = s.newest(exists, 10).await.unwrap();
        assert_eq!(list.iter().map(|h| (h.uid, h.seen, h.from.as_str(), h.subject.as_str())).collect::<Vec<_>>(), [(8, false, "Bob", "Two"), (7, true, "a@x.com", "One")]);
        let seen = script.await.unwrap();
        assert_eq!(seen[0], "g1 LOGIN \"me@icloud.com\" \"abcd-efgh\"\r\n");
        assert!(seen[3].contains("BODY.PEEK"), "headers are read without marking them read");
    }
}
