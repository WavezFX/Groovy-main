// Signing in to Gmail (Google) and Outlook (Microsoft) the way desktop apps
// should (RFC 8252): the browser opens the provider's own page, and the
// answer comes back to a one-time listener on 127.0.0.1 with a random port,
// protected by PKCE (S256) and a random state. Groovy never sees the
// password. The refresh token goes to Windows Credential Manager (mod.rs);
// access tokens stay in memory and are refreshed when they run out.
//
// The app registrations are the owner's: the client IDs come from the build
// (GROOVY_GOOGLE_CLIENT_ID, GROOVY_GOOGLE_CLIENT_SECRET — Google's "desktop"
// secret is not confidential, by Google's own documentation — and
// GROOVY_MICROSOFT_CLIENT_ID) or from Settings → Mail.

use std::time::Duration;

use base64::Engine;
use ring::rand::SecureRandom;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::text::{form, query_value, url_encode};
use crate::i18n::{t, tf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Microsoft,
}

pub struct Def {
    pub auth: &'static str,
    pub token: &'static str,
    pub scope: &'static str,
}

pub const GOOGLE: Def = Def {
    auth: "https://accounts.google.com/o/oauth2/v2/auth",
    token: "https://oauth2.googleapis.com/token",
    // Read, plus modify for "Mark read", "Archive" and (round 3D item 8) sending
    // the reply the user wrote and pressed Send on. Nothing can be deleted.
    scope: "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.modify",
};

pub const MICROSOFT: Def = Def {
    auth: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    // Mail.Send (round 3D item 8) only sends the reply the user pressed Send on.
    scope: "offline_access https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read",
};

/// What Outlook accounts connected before replies were asked for (no Mail.Send):
/// their refresh asks for no more than they agreed to.
const MICROSOFT_READ_SCOPE: &str = "offline_access https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/User.Read";

/// How long the browser may take before the sign-in is given up.
const WAIT: Duration = Duration::from_secs(300);

impl Provider {
    pub fn def(self) -> &'static Def {
        match self {
            Provider::Google => &GOOGLE,
            Provider::Microsoft => &MICROSOFT,
        }
    }

    fn kv_key(self) -> &'static str {
        match self {
            Provider::Google => "mail-client-google",
            Provider::Microsoft => "mail-client-microsoft",
        }
    }

    pub fn client_id(self) -> Option<String> {
        let saved = crate::db::conn().ok().and_then(|c| crate::music::kv_get(&c, self.kv_key())).filter(|v| !v.trim().is_empty());
        saved.or_else(|| {
            match self {
                Provider::Google => option_env!("GROOVY_GOOGLE_CLIENT_ID"),
                Provider::Microsoft => option_env!("GROOVY_MICROSOFT_CLIENT_ID"),
            }
            // An unset repository secret builds as an empty string.
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string)
        })
    }

    pub fn client_secret(self) -> Option<String> {
        match self {
            Provider::Google => crate::secrets::get_internal("mail-google-client-secret").or_else(|| option_env!("GROOVY_GOOGLE_CLIENT_SECRET").filter(|v| !v.is_empty()).map(str::to_string)),
            Provider::Microsoft => None,
        }
    }

    /// The owner's own app registration, from Settings → Mail.
    pub fn configure(self, client_id: &str, client_secret: &str) -> Result<(), String> {
        let id = client_id.trim();
        if id.len() > 200 || id.contains(char::is_whitespace) {
            return Err(t("That doesn't look like a client ID."));
        }
        crate::music::kv_set(&*crate::db::conn()?, self.kv_key(), id);
        if self == Provider::Google {
            crate::secrets::set_internal("mail-google-client-secret", client_secret.trim())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Tokens {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at: i64,
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn random(n: usize) -> Result<String, String> {
    let mut b = vec![0u8; n];
    ring::rand::SystemRandom::new().fill(&mut b).map_err(|_| "no randomness".to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

/// The PKCE challenge for a verifier (S256).
pub fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()))
}

pub fn auth_url(p: Provider, client_id: &str, redirect: &str, state: &str, verifier: &str) -> String {
    let d = p.def();
    let mut q = vec![
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", d.scope),
        ("state", state),
        ("code_challenge_method", "S256"),
    ];
    let ch = challenge(verifier);
    q.push(("code_challenge", &ch));
    if p == Provider::Google {
        // A refresh token, and the account picker every time.
        q.push(("access_type", "offline"));
        q.push(("prompt", "consent select_account"));
    } else {
        q.push(("prompt", "select_account"));
    }
    format!("{}?{}", d.auth, form(&q))
}

/// The browser's request to the listener: `code` when the state matches.
pub fn read_redirect(request: &str, state: &str) -> Option<Result<String, String>> {
    let path = request.lines().next()?.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    if let Some(err) = query_value(query, "error") {
        return Some(Err(if err == "access_denied" { t("Sign-in was cancelled.") } else { tf("Sign-in failed: {reason}", &[("reason", &err)]) }));
    }
    let code = query_value(query, "code")?;
    if query_value(query, "state").as_deref() != Some(state) {
        return Some(Err(t("Sign-in failed: the answer didn't match. Try again.")));
    }
    Some(Ok(code))
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Groovy</title><body style=\"font:16px system-ui;background:#111;color:#eee;display:grid;place-items:center;height:90vh\"><p>";

/// Opens the provider's page in the browser and waits for the answer.
pub async fn authorize(p: Provider) -> Result<Tokens, String> {
    let client_id = p.client_id().ok_or_else(|| t("Add the client ID in Settings → Mail first."))?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| e.to_string())?;
    let redirect = format!("http://127.0.0.1:{}/", listener.local_addr().map_err(|e| e.to_string())?.port());
    let state = random(16)?;
    let verifier = random(48)?;
    crate::browser::open(&auth_url(p, &client_id, &redirect, &state, &verifier))?;
    let code = tokio::time::timeout(WAIT, async {
        loop {
            let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 8192];
            let n = tokio::time::timeout(Duration::from_secs(10), sock.read(&mut buf)).await.unwrap_or(Ok(0)).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            // Favicon and other requests carry no code: answer and keep waiting.
            let Some(answer) = read_redirect(&request, &state) else {
                let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                continue;
            };
            let message = if answer.is_ok() { t("Signed in. You can close this tab and go back to Groovy.") } else { t("Sign-in didn't finish. You can close this tab.") };
            let body = format!("{DONE_PAGE}{}</p>", html_escape(&message));
            let _ = sock
                .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes())
                .await;
            return answer;
        }
    })
    .await
    .map_err(|_| t("Sign-in took too long. Try again."))??;
    exchange(p, &client_id, &[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &redirect), ("code_verifier", &verifier)], p.def().scope).await
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub async fn refresh(p: Provider, refresh_token: &str) -> Result<Tokens, String> {
    let client_id = p.client_id().ok_or_else(|| t("Add the client ID in Settings → Mail first."))?;
    let grant = [("grant_type", "refresh_token"), ("refresh_token", refresh_token)];
    let mut t = match exchange(p, &client_id, &grant, p.def().scope).await {
        Ok(t) => t,
        // Connected before Mail.Send was asked for: reading goes on; a reply asks to connect again.
        Err(_) if p == Provider::Microsoft => exchange(p, &client_id, &grant, MICROSOFT_READ_SCOPE).await?,
        Err(e) => return Err(e),
    };
    // Microsoft may hand out a new refresh token; Google keeps the old one.
    if t.refresh.is_none() {
        t.refresh = Some(refresh_token.to_string());
    }
    Ok(t)
}

async fn exchange(p: Provider, client_id: &str, extra: &[(&str, &str)], scope: &str) -> Result<Tokens, String> {
    let d = p.def();
    let secret = p.client_secret();
    let mut pairs: Vec<(&str, &str)> = vec![("client_id", client_id)];
    if let Some(s) = secret.as_deref() {
        pairs.push(("client_secret", s));
    }
    if p == Provider::Microsoft {
        pairs.push(("scope", scope));
    }
    pairs.extend_from_slice(extra);
    let url = reqwest::Url::parse(d.token).map_err(|e| e.to_string())?;
    let response = crate::net::client(&url, Duration::from_secs(30))?
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form(&pairs))
        .send()
        .await
        .map_err(|e| tf("Network error: {error}", &[("error", &e.to_string())]))?;
    let ok = response.status().is_success();
    let body = crate::net::read_capped(response, 64 * 1024).await?;
    if !ok {
        crate::log::line(format!("mail: token exchange refused ({})", crate::net::error_detail(&body)));
        return Err(t("The account refused the sign-in. Connect it again."));
    }
    let a: TokenAnswer = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok(Tokens { access: a.access_token, refresh: a.refresh_token, expires_at: crate::db::now() + a.expires_in.unwrap_or(3600) - 60 })
}

/// For a path segment of an API URL (Graph's message ids may hold `/` or `=`).
pub fn segment(id: &str) -> String {
    url_encode(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc_7636() {
        // Appendix B of RFC 7636.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn the_sign_in_page_asks_for_mail_only_with_pkce() {
        let u = auth_url(Provider::Google, "id.apps.googleusercontent.com", "http://127.0.0.1:5000/", "st", "verifier");
        assert!(u.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=id.apps.googleusercontent.com&redirect_uri=http%3A%2F%2F127.0.0.1%3A5000%2F"));
        assert!(u.contains("gmail.readonly") && u.contains("gmail.modify") && !u.contains("mail.google.com%2F&"));
        assert!(u.contains("code_challenge_method=S256") && u.contains(&format!("code_challenge={}", challenge("verifier"))));
        assert!(!u.contains("verifier&"), "the verifier itself never goes in the URL");
        let m = auth_url(Provider::Microsoft, "abc", "http://127.0.0.1:1/", "st", "v");
        assert!(m.contains("Mail.ReadWrite") && m.contains("offline_access"));
    }

    #[test]
    fn the_redirect_must_carry_our_state() {
        assert_eq!(read_redirect("GET /?code=4%2Fabc&state=s1 HTTP/1.1\r\nHost: x\r\n", "s1"), Some(Ok("4/abc".into())));
        assert!(matches!(read_redirect("GET /?code=4&state=other HTTP/1.1", "s1"), Some(Err(_))));
        assert!(matches!(read_redirect("GET /?error=access_denied&state=s1 HTTP/1.1", "s1"), Some(Err(_))));
        assert_eq!(read_redirect("GET /favicon.ico HTTP/1.1", "s1"), None);
    }
}
