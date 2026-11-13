// Sending a reply from an iCloud or IMAP account (round 3D item 8): a small
// SMTP client, TLS only (port 465 straight away, or 587 with STARTTLS), that
// logs in with the account's own app password and sends one message. Nothing
// is sent unless the user pressed Send.

use std::time::Duration;

use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;

use super::imap::{tls_config, Io};
use crate::i18n::{t, tf};

const TIMEOUT: Duration = Duration::from_secs(30);

/// The SMTP server for an IMAP host: iCloud's own, else `imap.` → `smtp.`.
pub fn server_for(kind: &str, imap_host: &str) -> (String, u16) {
    let host = imap_host.to_ascii_lowercase();
    if kind == "icloud" || host.ends_with("mail.me.com") {
        return ("smtp.mail.me.com".into(), 587);
    }
    let smtp = host.strip_prefix("imap.").map(|rest| format!("smtp.{rest}")).unwrap_or(host);
    (smtp, 465)
}

/// "." at the start of a line doubled, and the message ended with CRLF.CRLF.
pub fn dot_stuffed(message: &str) -> String {
    let mut out = String::new();
    for line in message.replace("\r\n", "\n").split('\n') {
        if line.starts_with('.') {
            out.push('.');
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(".\r\n");
    out
}

/// An address fit for MAIL FROM / RCPT TO: no spaces, brackets or line breaks.
fn plain_address(a: &str) -> Result<&str, String> {
    if a.is_empty() || a.chars().any(|c| c.is_whitespace() || c == '<' || c == '>' || c.is_control()) || !a.contains('@') {
        return Err(t("That address can't be written to."));
    }
    Ok(a)
}

struct Conn {
    io: BufReader<Box<dyn Io>>,
}

impl Conn {
    /// One reply, joining its continuation lines ("250-…" then "250 …").
    async fn read(&mut self) -> Result<(u16, String), String> {
        let mut text = String::new();
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout(TIMEOUT, self.io.read_line(&mut line))
                .await
                .map_err(|_| t("The mail server stopped answering."))?
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err(t("The mail server closed the connection."));
            }
            let code: u16 = line.get(..3).and_then(|c| c.parse().ok()).ok_or_else(|| t("The mail server answered something unexpected."))?;
            text.push_str(line.get(4..).unwrap_or("").trim_end());
            text.push('\n');
            if line.as_bytes().get(3) != Some(&b'-') {
                return Ok((code, text));
            }
        }
    }

    async fn send(&mut self, line: &str) -> Result<(), String> {
        self.io.get_mut().write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        self.io.get_mut().flush().await.map_err(|e| e.to_string())
    }

    /// Sends a command and expects a reply in the 2xx/3xx family given.
    async fn expect(&mut self, line: &str, want: u16) -> Result<String, String> {
        self.send(line).await?;
        let (code, text) = self.read().await?;
        if code / 100 != want / 100 {
            let reason = text.lines().next().unwrap_or("").to_string();
            return Err(if code == 535 || code == 534 {
                t("The mail server didn't accept that address and password.")
            } else {
                tf("The mail server said: {reason}", &[("reason", &format!("{code} {reason}"))])
            });
        }
        Ok(text)
    }
}

/// Logs in and sends `message` from `from` to `to`, over a connection already secured.
pub async fn send_over(io: Box<dyn Io>, user: &str, password: &str, from: &str, to: &str, message: &str, greeted: bool) -> Result<(), String> {
    let mut c = Conn { io: BufReader::new(io) };
    if !greeted {
        let (code, _) = c.read().await?;
        if code != 220 {
            return Err(t("The mail server answered something unexpected."));
        }
    }
    c.expect("EHLO groovy.local\r\n", 250).await?;
    let auth = base64::engine::general_purpose::STANDARD.encode(format!("\0{user}\0{password}"));
    c.expect(&format!("AUTH PLAIN {auth}\r\n"), 235).await?;
    c.expect(&format!("MAIL FROM:<{}>\r\n", plain_address(from)?), 250).await?;
    c.expect(&format!("RCPT TO:<{}>\r\n", plain_address(to)?), 250).await?;
    c.expect("DATA\r\n", 354).await?;
    c.expect(&dot_stuffed(message), 250).await?;
    let _ = c.send("QUIT\r\n").await;
    Ok(())
}

/// Connects to `host`:`port` (465: TLS at once; otherwise STARTTLS first) and sends.
pub async fn send(host: &str, port: u16, user: &str, password: &str, from: &str, to: &str, message: &str) -> Result<(), String> {
    let unreachable = || tf("Couldn't reach {host}.", &[("host", host)]);
    let tcp = tokio::time::timeout(TIMEOUT, TcpStream::connect((host, port))).await.map_err(|_| unreachable())?.map_err(|_| unreachable())?;
    let name = ServerName::try_from(host.to_string()).map_err(|_| unreachable())?;
    let secure = |e: std::io::Error| tf("A secure connection to {host} failed: {error}", &[("host", host), ("error", &e.to_string())]);
    if port == 465 {
        let tls = tokio::time::timeout(TIMEOUT, TlsConnector::from(tls_config()).connect(name, tcp)).await.map_err(|_| unreachable())?.map_err(secure)?;
        return send_over(Box::new(tls), user, password, from, to, message, false).await;
    }
    // STARTTLS: greeting and EHLO in the clear, then nothing but TLS.
    let mut plain = Conn { io: BufReader::new(Box::new(tcp) as Box<dyn Io>) };
    let (code, _) = plain.read().await?;
    if code != 220 {
        return Err(unreachable());
    }
    let ehlo = plain.expect("EHLO groovy.local\r\n", 250).await?;
    if !ehlo.to_ascii_uppercase().contains("STARTTLS") {
        return Err(tf("{host} doesn't offer a secure connection.", &[("host", host)]));
    }
    plain.expect("STARTTLS\r\n", 220).await?;
    if !plain.io.buffer().is_empty() {
        return Err(unreachable());
    }
    let tcp = plain.io.into_inner();
    let tls = tokio::time::timeout(TIMEOUT, TlsConnector::from(tls_config()).connect(name, tcp)).await.map_err(|_| unreachable())?.map_err(secure)?;
    send_over(Box::new(tls), user, password, from, to, message, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_and_dots() {
        assert_eq!(server_for("icloud", "imap.mail.me.com"), ("smtp.mail.me.com".into(), 587));
        assert_eq!(server_for("imap", "imap.fastmail.com"), ("smtp.fastmail.com".into(), 465));
        assert_eq!(dot_stuffed("a\r\n.b\r\n"), "a\r\n..b\r\n\r\n.\r\n");
        assert!(plain_address("a@x.com>\r\nRCPT TO:<b@y.com").is_err());
    }

    #[tokio::test]
    async fn a_reply_against_a_scripted_server() {
        let (client, mut server) = tokio::io::duplex(64 * 1024);
        let script = tokio::spawn(async move {
            let mut r = BufReader::new(&mut server);
            r.get_mut().write_all(b"220 smtp.example.com ready\r\n").await.unwrap();
            let mut seen = Vec::new();
            for answer in ["250-smtp.example.com\r\n250 AUTH PLAIN\r\n", "235 ok\r\n", "250 ok\r\n", "250 ok\r\n", "354 go on\r\n"] {
                let mut line = String::new();
                r.read_line(&mut line).await.unwrap();
                seen.push(line);
                r.get_mut().write_all(answer.as_bytes()).await.unwrap();
            }
            let mut data = String::new();
            loop {
                let mut line = String::new();
                r.read_line(&mut line).await.unwrap();
                data.push_str(&line);
                if line == ".\r\n" {
                    break;
                }
            }
            r.get_mut().write_all(b"250 queued\r\n").await.unwrap();
            (seen, data)
        });
        send_over(Box::new(client), "me@icloud.com", "app-pass", "me@icloud.com", "ava@example.com", "Subject: Re: x\r\n\r\n.hidden\r\n", false).await.unwrap();
        let (seen, data) = script.await.unwrap();
        assert_eq!(seen[2], "MAIL FROM:<me@icloud.com>\r\n");
        assert_eq!(seen[3], "RCPT TO:<ava@example.com>\r\n");
        assert!(data.contains("\r\n..hidden\r\n"), "a leading dot is doubled");
    }
}
