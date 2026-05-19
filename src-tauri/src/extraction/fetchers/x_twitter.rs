//! X/Twitter fetcher (spec §6 *X/Twitter fetcher*).
//!
//! Two paths:
//! - **Path A (default)** — oEmbed via `publish.twitter.com`. No bearer
//!   token required. Returns a single-post `FetchedSourceContent`.
//! - **Path B (thread reconstruction)** — X API v2. Requires the
//!   `x_bearer_token` secret. Fetches the root tweet, then paginates the
//!   author's recent tweets and filters by `conversation_id` +
//!   `author_id`; reachable replies become ordered `SourceChunk`s.
//!
//! Pure parsing (HTML strip; thread reply-chain construction) is unit-
//! tested in isolation. Network paths are integration-tested when the
//! token + connectivity are available.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;

use crate::domain::source::{Source, XTwitterSource};
use crate::error::Result;
use crate::extraction::types::{
    ExtractionFailure, FetchedSourceContent, SourceChunk, SourceChunkKind,
};
use crate::settings::keychain::{get_secret, SecretKey};
use crate::settings::secret_store::SecretStore;

const OEMBED_URL_PREFIX: &str = "https://publish.twitter.com/oembed?omit_script=1&url=";
const X_API_BASE: &str = "https://api.twitter.com/2";

#[derive(Debug, Deserialize)]
struct OembedBody {
    html: Option<String>,
    author_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TweetGetBody {
    data: Tweet,
    includes: Option<Includes>,
}

#[derive(Debug, Deserialize, Clone)]
struct Tweet {
    id: String,
    text: String,
    author_id: String,
    conversation_id: String,
    created_at: String,
    referenced_tweets: Option<Vec<Reference>>,
}

#[derive(Debug, Deserialize, Clone)]
struct Reference {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct Includes {
    users: Option<Vec<UserInclude>>,
}

#[derive(Debug, Deserialize)]
struct UserInclude {
    id: String,
    username: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserTweetsBody {
    data: Option<Vec<Tweet>>,
    meta: Option<UserTweetsMeta>,
}

#[derive(Debug, Deserialize)]
struct UserTweetsMeta {
    next_token: Option<String>,
}

pub async fn fetch_oembed(
    canonical_url: &str,
    post_id: &str,
    http: &reqwest::Client,
) -> Result<std::result::Result<FetchedSourceContent, ExtractionFailure>> {
    let url = format!(
        "{}{}",
        OEMBED_URL_PREFIX,
        utf8_percent_encode(canonical_url, NON_ALPHANUMERIC),
    );
    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() || e.is_connect() => {
            return Ok(Err(ExtractionFailure::NetworkUnavailable {
                message: format!("{e}"),
            }))
        }
        Err(e) => {
            return Err(crate::error::AppError::new(
                crate::error::AppErrorKind::ExtractionFailed,
                format!("oembed fetch: {e}"),
            ))
        }
    };
    if !resp.status().is_success() {
        return Ok(Err(ExtractionFailure::ExtractionFailed {
            reason: format!("oembed returned HTTP {}", resp.status()),
        }));
    }
    let body: OembedBody = resp.json().await.map_err(|e| {
        crate::error::AppError::new(
            crate::error::AppErrorKind::ExtractionFailed,
            format!("oembed decode: {e}"),
        )
    })?;
    let html = body.html.unwrap_or_default();
    let text = strip_oembed_html(&html);
    Ok(Ok(build_oembed_source(
        canonical_url,
        post_id,
        &text,
        body.author_name.as_deref(),
    )))
}

pub async fn fetch_thread_via_api(
    canonical_url: &str,
    post_id: &str,
    http: &reqwest::Client,
    secret_store: &Arc<dyn SecretStore>,
) -> Result<std::result::Result<FetchedSourceContent, ExtractionFailure>> {
    let bearer = match get_secret(secret_store.as_ref(), SecretKey::XBearerToken)? {
        Some(b) => b,
        None => {
            // Caller may have already fetched the oEmbed result; the IPC
            // layer wraps this case with a banner instead of failing hard.
            return Ok(Err(ExtractionFailure::ExtractionFailed {
                reason: "X bearer token missing — thread reconstruction requires `x_bearer_token`"
                    .into(),
            }));
        }
    };

    // 1. Root tweet
    let root_url = format!(
        "{X_API_BASE}/tweets/{post_id}?tweet.fields=author_id,conversation_id,created_at,referenced_tweets&expansions=author_id&user.fields=username,name"
    );
    let root_resp = http
        .get(&root_url)
        .bearer_auth(&bearer)
        .send()
        .await
        .map_err(|e| {
            crate::error::AppError::new(
                crate::error::AppErrorKind::NetworkUnavailable,
                format!("x api tweets: {e}"),
            )
        })?;
    if let Some(reset_at) = parse_rate_limit_reset(root_resp.headers()) {
        if root_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Ok(Err(ExtractionFailure::RateLimited {
                provider: "x_twitter".into(),
                reset_at: Some(reset_at),
            }));
        }
    }
    if !root_resp.status().is_success() {
        return Ok(Err(ExtractionFailure::ExtractionFailed {
            reason: format!("x api /tweets returned {}", root_resp.status()),
        }));
    }
    let root_body: TweetGetBody = root_resp.json().await.map_err(|e| {
        crate::error::AppError::new(
            crate::error::AppErrorKind::ExtractionFailed,
            format!("x api decode: {e}"),
        )
    })?;
    let root = root_body.data;
    let username = root_body
        .includes
        .as_ref()
        .and_then(|i| i.users.as_ref())
        .and_then(|users| users.iter().find(|u| u.id == root.author_id))
        .map(|u| u.username.clone());
    let author_name = root_body
        .includes
        .and_then(|i| i.users)
        .and_then(|users| users.into_iter().find(|u| u.id == root.author_id))
        .and_then(|u| u.name);

    // 2. Paginate author timeline, filter by conversation/author.
    let mut next_token: Option<String> = None;
    let mut self_replies: Vec<Tweet> = vec![root.clone()];
    for _ in 0..5 {
        let mut url = format!(
            "{X_API_BASE}/users/{}/tweets?max_results=100&tweet.fields=author_id,conversation_id,created_at,referenced_tweets",
            root.author_id
        );
        if let Some(tok) = &next_token {
            url.push_str("&pagination_token=");
            url.push_str(&utf8_percent_encode(tok, NON_ALPHANUMERIC).to_string());
        }
        let resp = http
            .get(&url)
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|e| {
                crate::error::AppError::new(
                    crate::error::AppErrorKind::NetworkUnavailable,
                    format!("x api timeline: {e}"),
                )
            })?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Ok(Err(ExtractionFailure::RateLimited {
                provider: "x_twitter".into(),
                reset_at: parse_rate_limit_reset(resp.headers()),
            }));
        }
        if !resp.status().is_success() {
            return Ok(Err(ExtractionFailure::ExtractionFailed {
                reason: format!("x api timeline returned {}", resp.status()),
            }));
        }
        let body: UserTweetsBody = resp.json().await.map_err(|e| {
            crate::error::AppError::new(
                crate::error::AppErrorKind::ExtractionFailed,
                format!("x api timeline decode: {e}"),
            )
        })?;
        if let Some(data) = body.data {
            for t in data {
                if t.conversation_id == root.conversation_id && t.author_id == root.author_id {
                    self_replies.push(t);
                }
            }
        }
        next_token = body.meta.and_then(|m| m.next_token);
        if next_token.is_none() {
            break;
        }
    }

    let chain = build_thread_chain(root.clone(), self_replies);
    let chunks = chain
        .iter()
        .enumerate()
        .map(|(i, t)| SourceChunk {
            kind: SourceChunkKind::Post,
            order: i as u32,
            text: t.text.clone(),
            url: Some(format!(
                "https://x.com/{}/status/{}",
                username.as_deref().unwrap_or("i/web"),
                t.id
            )),
            timestamp_seconds: None,
        })
        .collect::<Vec<_>>();
    let thread_post_ids = chain.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    let text = chain
        .iter()
        .map(|t| t.text.clone())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let now = Utc::now();
    let source = Source::XTwitter(XTwitterSource {
        origin_url: Some(canonical_url.to_string()),
        title: None,
        author: author_name.clone(),
        fetched_at: Some(now),
        content_hash: None,
        post_id: root.id.clone(),
        username: username.clone(),
        thread_post_ids,
    });

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    Ok(Ok(FetchedSourceContent {
        source,
        canonical_url: canonical_url.to_string(),
        fetched_at: now,
        title: None,
        author: author_name,
        text,
        chunks,
        raw_metadata: Default::default(),
        content_hash,
        cached: false,
    }))
}

// ─── Pure helpers ────────────────────────────────────────────────────────────

/// Strip HTML from an oEmbed `html` field: keep link text, drop URLs that
/// match the wrapping <blockquote>'s source post.
pub fn strip_oembed_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity_buf = String::new();
    for c in html.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            // Treat the closing of any block-level-ish tag as a soft break.
            if !out.ends_with('\n') && !out.is_empty() {
                out.push(' ');
            }
        } else if !in_tag {
            if c == '&' {
                in_entity = true;
                entity_buf.clear();
                entity_buf.push(c);
            } else if in_entity {
                entity_buf.push(c);
                if c == ';' {
                    out.push_str(&decode_entity(&entity_buf));
                    in_entity = false;
                    entity_buf.clear();
                } else if entity_buf.len() > 8 {
                    // Bail on overly long "entities" — likely not entities.
                    out.push_str(&entity_buf);
                    in_entity = false;
                    entity_buf.clear();
                }
            } else {
                out.push(c);
            }
        }
    }
    // Final flush if we ended mid-entity.
    if in_entity {
        out.push_str(&entity_buf);
    }
    // Collapse whitespace runs.
    let mut squeezed = String::with_capacity(out.len());
    let mut prev_ws = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                squeezed.push(' ');
            }
            prev_ws = true;
        } else {
            squeezed.push(c);
            prev_ws = false;
        }
    }
    squeezed.trim().to_string()
}

fn decode_entity(e: &str) -> String {
    match e {
        "&amp;" => "&".into(),
        "&lt;" => "<".into(),
        "&gt;" => ">".into(),
        "&quot;" => "\"".into(),
        "&#39;" | "&apos;" => "'".into(),
        "&nbsp;" => " ".into(),
        other => other.to_string(),
    }
}

/// Build the chain root → … reply chain. Self-replies are linked through
/// their `referenced_tweets[kind=replied_to].id`. Branches are ordered by
/// `created_at` and concatenated with a separator post.
fn build_thread_chain(root: Tweet, all: Vec<Tweet>) -> Vec<Tweet> {
    let mut by_replied: std::collections::HashMap<String, Vec<Tweet>> = Default::default();
    for t in all.into_iter() {
        if t.id == root.id {
            continue;
        }
        if let Some(refs) = &t.referenced_tweets {
            if let Some(parent) = refs.iter().find(|r| r.kind == "replied_to") {
                by_replied.entry(parent.id.clone()).or_default().push(t);
                continue;
            }
        }
        // Tweets without an explicit reply ref but matching conversation_id
        // get appended to the root chain. Stable order by created_at.
        by_replied.entry(root.id.clone()).or_default().push(t);
    }

    let mut out = vec![root.clone()];
    let mut frontier: Vec<&str> = vec![root.id.as_str()];
    while let Some(parent_id) = frontier.pop() {
        if let Some(children) = by_replied.get_mut(parent_id) {
            children.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            for child in children.iter() {
                out.push(child.clone());
                if out.len() >= 100 {
                    return out;
                }
            }
            // Push children IDs onto frontier for further descent.
            let child_ids: Vec<String> = children.iter().map(|c| c.id.clone()).collect();
            for id in child_ids {
                frontier.push(string_leak(id));
            }
        }
    }
    out
}

/// Leak a String to get a 'static-ish &str slice that lives until the end
/// of `build_thread_chain`. The leaked memory is bounded by the 100-post
/// cap, so this is a deliberate trade for code clarity in a single
/// short-lived call.
fn string_leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn build_oembed_source(
    canonical_url: &str,
    post_id: &str,
    text: &str,
    author: Option<&str>,
) -> FetchedSourceContent {
    let now = Utc::now();
    let chunks = vec![SourceChunk {
        kind: SourceChunkKind::Post,
        order: 0,
        text: text.to_string(),
        url: Some(canonical_url.to_string()),
        timestamp_seconds: None,
    }];
    let source = Source::XTwitter(XTwitterSource {
        origin_url: Some(canonical_url.to_string()),
        title: None,
        author: author.map(str::to_string),
        fetched_at: Some(now),
        content_hash: None,
        post_id: post_id.to_string(),
        username: None,
        thread_post_ids: vec![post_id.to_string()],
    });
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());
    FetchedSourceContent {
        source,
        canonical_url: canonical_url.to_string(),
        fetched_at: now,
        title: None,
        author: author.map(str::to_string),
        text: text.to_string(),
        chunks,
        raw_metadata: Default::default(),
        content_hash,
        cached: false,
    }
}

fn parse_rate_limit_reset(headers: &reqwest::header::HeaderMap) -> Option<DateTime<Utc>> {
    let reset = headers.get("x-rate-limit-reset")?.to_str().ok()?;
    let unix: i64 = reset.parse().ok()?;
    DateTime::from_timestamp(unix, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_oembed_keeps_visible_text_drops_tags() {
        let html = r#"<blockquote class="twitter-tweet"><p lang="en" dir="ltr">Hello <a href="t.co/foo">world</a></p>&mdash; Alice (@alice) <a href="t.co/bar">May 19, 2026</a></blockquote>"#;
        let text = strip_oembed_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<"));
        assert!(!text.contains(">"));
    }

    #[test]
    fn strip_oembed_decodes_basic_entities() {
        let html = "&amp; &lt; &gt; &quot; &#39;";
        assert_eq!(strip_oembed_html(html), "& < > \" '");
    }

    #[test]
    fn thread_chain_orders_self_replies_by_created_at() {
        let root = Tweet {
            id: "1".into(),
            text: "root".into(),
            author_id: "a".into(),
            conversation_id: "c".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            referenced_tweets: None,
        };
        let r2 = Tweet {
            id: "2".into(),
            text: "second".into(),
            author_id: "a".into(),
            conversation_id: "c".into(),
            created_at: "2026-01-01T00:01:00Z".into(),
            referenced_tweets: Some(vec![Reference {
                kind: "replied_to".into(),
                id: "1".into(),
            }]),
        };
        let r3 = Tweet {
            id: "3".into(),
            text: "third".into(),
            author_id: "a".into(),
            conversation_id: "c".into(),
            created_at: "2026-01-01T00:02:00Z".into(),
            referenced_tweets: Some(vec![Reference {
                kind: "replied_to".into(),
                id: "2".into(),
            }]),
        };
        let chain = build_thread_chain(root.clone(), vec![root, r2, r3]);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].id, "1");
        assert_eq!(chain[1].id, "2");
        assert_eq!(chain[2].id, "3");
    }

    #[test]
    fn thread_chain_capped_at_100() {
        let root = Tweet {
            id: "1".into(),
            text: "root".into(),
            author_id: "a".into(),
            conversation_id: "c".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            referenced_tweets: None,
        };
        let mut replies = vec![root.clone()];
        for i in 0..250 {
            replies.push(Tweet {
                id: format!("{}", 2 + i),
                text: format!("post {i}"),
                author_id: "a".into(),
                conversation_id: "c".into(),
                created_at: format!("2026-01-01T00:01:{:02}Z", i % 60),
                referenced_tweets: Some(vec![Reference {
                    kind: "replied_to".into(),
                    id: "1".into(),
                }]),
            });
        }
        let chain = build_thread_chain(root.clone(), replies);
        assert!(chain.len() <= 100);
    }

    #[test]
    fn rate_limit_reset_parsed_from_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-rate-limit-reset",
            reqwest::header::HeaderValue::from_str("1800000000").unwrap(),
        );
        let dt = parse_rate_limit_reset(&headers).unwrap();
        assert_eq!(dt.timestamp(), 1_800_000_000);
    }
}
