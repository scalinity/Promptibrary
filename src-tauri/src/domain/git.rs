//! Git history + diff types per spec §10.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::PromptId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortStat {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptHistoryEntry {
    pub commit_sha: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: DateTime<Utc>,
    pub message: String,
    pub short_stat: ShortStat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDiff {
    pub prompt_id: PromptId,
    pub from_sha: Option<String>,
    pub to_sha: String,
    pub unified: String,
}
