//! Run + TokenCount per spec §4 "Run".

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppErrorDto;
use crate::ids::{PromptId, RunId};

use super::launch::LaunchProfile;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Created,
    Validating,
    Spawning,
    Running,
    Stopping,
    Succeeded,
    Failed,
    Canceled,
    VaultUnavailable,
    TranscriptSpooled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopSignal {
    #[serde(rename = "SIGINT")]
    SigInt,
    #[serde(rename = "SIGTERM")]
    SigTerm,
    #[serde(rename = "SIGKILL")]
    SigKill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCount {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: RunId,
    pub prompt_id: PromptId,
    pub prompt_title: String,
    pub status: RunStatus,
    pub profile: LaunchProfile,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub signal: Option<StopSignal>,
    pub transcript_vault_path: Option<String>,
    pub transcript_spool_path: Option<PathBuf>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub token_count: Option<TokenCount>,
    pub cost_usd: Option<f64>,
    pub error: Option<AppErrorDto>,
}
