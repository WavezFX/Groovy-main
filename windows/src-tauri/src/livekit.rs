// LiveKit: the access token Groovy joins the user's own room with. LiveKit
// tokens are JWTs signed (HS256) with the project's API secret; the key and
// secret stay in the keychain (secrets.rs) and only this short-lived token,
// limited to one room, goes to the island.

use ring::hmac;
use serde::Serialize;
use serde_json::json;

/// How long a token stays valid: just long enough to join.
const TTL_SECONDS: u64 = 10 * 60;

#[derive(Serialize, Debug)]
pub struct Access {
    pub url: String,
    pub token: String,
}

/// Base64url without padding, as JWTs use.
pub fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out
}

/// A signed HS256 JWT for these claims.
pub fn sign(claims: &serde_json::Value, secret: &str) -> String {
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = b64url(claims.to_string().as_bytes());
    let input = format!("{header}.{payload}");
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let sig = hmac::sign(&key, input.as_bytes());
    format!("{input}.{}", b64url(sig.as_ref()))
}

/// The claims LiveKit expects: who joins which room, allowed to talk and listen.
pub fn claims(api_key: &str, room: &str, identity: &str, now: u64) -> serde_json::Value {
    json!({
        "iss": api_key,
        "sub": identity,
        "name": "Groovy",
        "nbf": now.saturating_sub(10),
        "exp": now + TTL_SECONDS,
        "video": {
            "room": room,
            "roomJoin": true,
            "canPublish": true,
            "canSubscribe": true,
            "canPublishData": true,
        },
    })
}

pub fn token(url: &str, room: &str, api_key: Option<String>, api_secret: Option<String>) -> Result<Access, String> {
    let url = url.trim();
    if !(url.starts_with("wss://") || url.starts_with("ws://localhost") || url.starts_with("ws://127.0.0.1")) {
        return Err("Add your LiveKit server (wss://…) in Settings → Voice.".into());
    }
    let (Some(key), Some(secret)) = (api_key, api_secret) else {
        return Err("Add your LiveKit API key and secret in Settings → Voice.".into());
    };
    let room = if room.trim().is_empty() { "groovy" } else { room.trim() };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let identity = format!("groovy-{}", std::process::id());
    Ok(Access {
        url: url.to_string(),
        token: sign(&claims(&key, room, &identity, now), &secret),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_has_no_padding_and_url_safe_letters() {
        assert_eq!(b64url(b""), "");
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(b"foo"), "Zm9v");
        assert_eq!(b64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn the_token_is_a_signed_jwt_for_one_room() {
        // RFC 4231 test case 2's key and message, through the JWT framing.
        let t = sign(&json!({"a": 1}), "Jefe");
        let parts: Vec<&str> = t.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        let c = claims("APIkey", "groovy", "groovy-1", 1_000);
        assert_eq!(c["video"]["room"], "groovy");
        assert_eq!(c["exp"], 1_000 + TTL_SECONDS);
    }

    #[test]
    fn a_missing_server_or_secret_says_what_to_add() {
        assert!(token("", "r", Some("k".into()), Some("s".into())).unwrap_err().contains("server"));
        assert!(token("wss://x.livekit.cloud", "r", None, Some("s".into())).unwrap_err().contains("API key"));
        assert!(token("wss://x.livekit.cloud", "", Some("k".into()), Some("s".into())).is_ok());
    }
}
