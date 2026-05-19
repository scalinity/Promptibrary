//! `Source` discriminated union per spec §4 "Source".

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Manual,
    Youtube,
    XTwitter,
    Article,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    Manual(ManualSource),
    Youtube(YouTubeSource),
    XTwitter(XTwitterSource),
    Article(ArticleSource),
}

/// Manual source — user-entered prompt with no fetch origin. Per spec §4,
/// `originUrl` is always `null` for manual sources, which is now encoded in
/// the type itself: the field does not exist. The TS mirror uses
/// `Omit<SourceBase<"manual">, "originUrl">` to match.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManualSource {
    pub title: Option<String>,
    pub author: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeSource {
    pub origin_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub video_id: String,
    pub channel_name: Option<String>,
    pub transcript_language: Option<String>,
    pub duration_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XTwitterSource {
    pub origin_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub post_id: String,
    pub username: Option<String>,
    pub thread_post_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleSource {
    pub origin_url: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub site_name: Option<String>,
    pub byline: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}
