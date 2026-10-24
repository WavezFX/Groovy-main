// The Gmail API and Microsoft Graph mail calls: the unread count, the newest
// messages' sender, subject and snippet, Mark read / Archive, and (round 3D
// item 8) one message's text when it is opened and the reply the user sends.
// Each takes an access token from oauth.rs.

use std::time::Duration;

use serde_json::{json, Value};

use base64::Engine;

use super::body;
use super::oauth::segment;
use super::text;
use super::Message;
use crate::i18n::{t, tf};

const GMAIL: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const GRAPH: &str = "https://graph.microsoft.com/v1.0/me";

async fn call(method: reqwest::Method, url: &str, token: &str, body: Option<Value>) -> Result<Value, String> {
    call_with(method, url, token, body, None).await
}

async fn call_with(method: reqwest::Method, url: &str, token: &str, body: Option<Value>, prefer: Option<&str>) -> Result<Value, String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    let mut req = crate::net::client(&parsed, Duration::from_secs(30))?.request(method, parsed).bearer_auth(token);
    if let Some(p) = prefer {
        req = req.header("Prefer", p);
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    let response = req.send().await.map_err(|e| tf("Network error: {error}", &[("error", &e.to_string())]))?;
    let status = response.status();
    let bytes = crate::net::read_capped(response, 2 * 1024 * 1024).await?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(t("The account needs to be connected again."));
    }
    // An Outlook account connected before replies: it may read, not send.
    if status == reqwest::StatusCode::FORBIDDEN && url.ends_with("/reply") {
        return Err(t("Connect this account again to send replies from Groovy."));
    }
    if !status.is_success() {
        crate::log::line(format!("mail: {} answered {status} ({})", crate::net::host_for_log(&reqwest::Url::parse(url).map_err(|e| e.to_string())?), crate::net::error_detail(&bytes)));
        return Err(tf("The mail service answered {status}.", &[("status", status.as_str())]));
    }
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

// ── Gmail ─────────────────────────────────────────────────────────────────────

pub async fn gmail_email(token: &str) -> Result<String, String> {
    let v = call(reqwest::Method::GET, &format!("{GMAIL}/profile"), token, None).await?;
    v["emailAddress"].as_str().map(str::to_string).ok_or_else(|| t("The account didn't say its address."))
}

pub async fn gmail_inbox(token: &str, account: i64, email: &str, n: u32) -> Result<(u32, Vec<Message>), String> {
    let label = call(reqwest::Method::GET, &format!("{GMAIL}/labels/INBOX"), token, None).await?;
    let unread = label["messagesUnread"].as_u64().unwrap_or(0) as u32;
    let list = call(reqwest::Method::GET, &format!("{GMAIL}/messages?labelIds=INBOX&maxResults={n}"), token, None).await?;
    let mut out = Vec::new();
    for id in list["messages"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()) {
        let m = call(
            reqwest::Method::GET,
            &format!("{GMAIL}/messages/{}?format=metadata&metadataHeaders=From&metadataHeaders=Subject", segment(id)),
            token,
            None,
        )
        .await?;
        out.push(gmail_message(&m, account, email));
    }
    Ok((unread, out))
}

pub fn gmail_message(m: &Value, account: i64, email: &str) -> Message {
    let header = |name: &str| {
        m["payload"]["headers"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|h| h["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .and_then(|h| h["value"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let id = m["id"].as_str().unwrap_or("").to_string();
    Message {
        account,
        from: text::sender_name(&header("From")),
        subject: header("Subject"),
        snippet: decode_entities(m["snippet"].as_str().unwrap_or("")),
        at: m["internalDate"].as_str().and_then(|s| s.parse::<i64>().ok()).map(|ms| ms / 1000).unwrap_or(0),
        unread: m["labelIds"].as_array().is_some_and(|l| l.iter().any(|x| x == "UNREAD")),
        link: Some(format!("https://mail.google.com/mail/u/?authuser={}#inbox/{}", text::url_encode(email), text::url_encode(&id))),
        id,
    }
}

/// Gmail's snippets come HTML-escaped.
fn decode_entities(s: &str) -> String {
    s.replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

pub async fn gmail_modify(token: &str, id: &str, remove: &str) -> Result<(), String> {
    call(reqwest::Method::POST, &format!("{GMAIL}/messages/{}/modify", segment(id)), token, Some(json!({ "removeLabelIds": [remove] }))).await.map(|_| ())
}

/// One message opened: who, what about, its text (never kept).
pub async fn gmail_read(token: &str, id: &str) -> Result<(String, String, String), String> {
    let m = call(reqwest::Method::GET, &format!("{GMAIL}/messages/{}?format=full", segment(id)), token, None).await?;
    let header = |name: &str| gmail_header(&m, name);
    Ok((text::sender_name(&header("From")), header("Subject"), body::gmail_text(&m["payload"])))
}

fn gmail_header(m: &Value, name: &str) -> String {
    m["payload"]["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| h["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        .and_then(|h| h["value"].as_str())
        .map(text::decode_words)
        .unwrap_or_default()
}

/// The reply, threaded under the original, from the account's own address.
pub async fn gmail_reply(token: &str, from: &str, id: &str, reply: &str) -> Result<(), String> {
    let m = call(
        reqwest::Method::GET,
        &format!("{GMAIL}/messages/{}?format=metadata&metadataHeaders=From&metadataHeaders=Reply-To&metadataHeaders=Subject&metadataHeaders=Message-ID&metadataHeaders=References", segment(id)),
        token,
        None,
    )
    .await?;
    let headers: String = ["From", "Reply-To", "Subject", "Message-ID", "References"]
        .iter()
        .map(|n| (n, gmail_header(&m, n)))
        .filter(|(_, v)| !v.is_empty())
        .map(|(n, v)| format!("{n}: {v}\r\n"))
        .collect();
    let original = body::original(&headers);
    let raw = body::reply_message(from, &original, reply, &body::date_now(), &body::new_id(from));
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    call(reqwest::Method::POST, &format!("{GMAIL}/messages/send"), token, Some(json!({ "raw": encoded, "threadId": m["threadId"] }))).await.map(|_| ())
}

// ── Outlook (Microsoft Graph) ─────────────────────────────────────────────────

pub async fn graph_email(token: &str) -> Result<String, String> {
    let v = call(reqwest::Method::GET, &format!("{GRAPH}?$select=mail,userPrincipalName"), token, None).await?;
    v["mail"].as_str().or(v["userPrincipalName"].as_str()).map(str::to_string).ok_or_else(|| t("The account didn't say its address."))
}

pub async fn graph_inbox(token: &str, account: i64, n: u32) -> Result<(u32, Vec<Message>), String> {
    let folder = call(reqwest::Method::GET, &format!("{GRAPH}/mailFolders/inbox?$select=unreadItemCount"), token, None).await?;
    let unread = folder["unreadItemCount"].as_u64().unwrap_or(0) as u32;
    let list = call(
        reqwest::Method::GET,
        &format!("{GRAPH}/mailFolders/inbox/messages?$top={n}&$select=id,subject,from,receivedDateTime,isRead,bodyPreview,webLink&$orderby=receivedDateTime%20desc"),
        token,
        None,
    )
    .await?;
    Ok((unread, list["value"].as_array().into_iter().flatten().map(|m| graph_message(m, account)).collect()))
}

pub fn graph_message(m: &Value, account: i64) -> Message {
    let from = &m["from"]["emailAddress"];
    Message {
        account,
        id: m["id"].as_str().unwrap_or("").to_string(),
        from: from["name"].as_str().filter(|s| !s.is_empty()).or(from["address"].as_str()).unwrap_or("").to_string(),
        subject: m["subject"].as_str().unwrap_or("").to_string(),
        snippet: m["bodyPreview"].as_str().unwrap_or("").chars().take(200).collect(),
        at: m["receivedDateTime"].as_str().and_then(text::iso_date).unwrap_or(0),
        unread: !m["isRead"].as_bool().unwrap_or(true),
        link: m["webLink"].as_str().filter(|l| l.starts_with("https://")).map(str::to_string),
    }
}

pub async fn graph_read(token: &str, id: &str) -> Result<(String, String, String), String> {
    let m = call_with(
        reqwest::Method::GET,
        &format!("{GRAPH}/messages/{}?$select=subject,from,body", segment(id)),
        token,
        None,
        Some("outlook.body-content-type=\"text\""),
    )
    .await?;
    let from = &m["from"]["emailAddress"];
    let who = from["name"].as_str().filter(|s| !s.is_empty()).or(from["address"].as_str()).unwrap_or("").to_string();
    let content = m["body"]["content"].as_str().unwrap_or("");
    let text = if m["body"]["contentType"].as_str() == Some("html") { body::html_text(content) } else { content.to_string() };
    Ok((who, m["subject"].as_str().unwrap_or("").to_string(), body::tidy(&text)))
}

/// Graph threads and addresses the reply itself.
pub async fn graph_reply(token: &str, id: &str, reply: &str) -> Result<(), String> {
    call(reqwest::Method::POST, &format!("{GRAPH}/messages/{}/reply", segment(id)), token, Some(json!({ "comment": reply }))).await.map(|_| ())
}

pub async fn graph_mark_read(token: &str, id: &str) -> Result<(), String> {
    call(reqwest::Method::PATCH, &format!("{GRAPH}/messages/{}", segment(id)), token, Some(json!({ "isRead": true }))).await.map(|_| ())
}

pub async fn graph_archive(token: &str, id: &str) -> Result<(), String> {
    call(reqwest::Method::POST, &format!("{GRAPH}/messages/{}/move", segment(id)), token, Some(json!({ "destinationId": "archive" }))).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_and_graph_messages_read_alike() {
        let g = json!({
            "id": "18c", "snippet": "See you at 9 &amp; don&#39;t be late", "internalDate": "1792220700000", "labelIds": ["INBOX", "UNREAD"],
            "payload": { "headers": [{ "name": "From", "value": "Anna <anna@example.com>" }, { "name": "Subject", "value": "Tomorrow" }] }
        });
        let m = gmail_message(&g, 3, "me@gmail.com");
        assert_eq!((m.from.as_str(), m.subject.as_str(), m.at, m.unread), ("Anna", "Tomorrow", 1_792_220_700, true));
        assert_eq!(m.snippet, "See you at 9 & don't be late");
        assert_eq!(m.link.as_deref(), Some("https://mail.google.com/mail/u/?authuser=me%40gmail.com#inbox/18c"));
        let o = json!({
            "id": "AAMk=", "subject": "Invoice", "isRead": false, "bodyPreview": "Hello", "receivedDateTime": "2026-10-17T07:05:00Z",
            "from": { "emailAddress": { "name": "", "address": "billing@example.com" } }, "webLink": "https://outlook.live.com/owa/?ItemID=x"
        });
        let m = graph_message(&o, 4);
        assert_eq!((m.from.as_str(), m.subject.as_str(), m.at, m.unread), ("billing@example.com", "Invoice", 1_792_220_700, true));
        assert!(graph_message(&json!({ "webLink": "javascript:alert(1)" }), 4).link.is_none(), "only https links are opened");
    }
}
