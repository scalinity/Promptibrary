//! AppSettings + SecretStatus per spec §13.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{PromptId, RunId};

use super::prompt::{ClaudeModelId, ClaudePermissionMode, LaunchDestination, VerifierMode};
use super::tag::TagColorSlug;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExtractionModelId {
    #[serde(rename = "claude-sonnet-4-6")]
    ClaudeSonnet46,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeepExtractionModelId {
    #[serde(rename = "claude-opus-4-7")]
    ClaudeOpus47,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionHistorySettings {
    /// default 200, range 50..1000
    pub rename_detection_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSettings {
    pub vault_path: Option<PathBuf>,
    pub default_destination: LaunchDestination,
    pub default_model: ClaudeModelId,
    pub default_verifier_mode: VerifierMode,
    pub default_permission_mode: ClaudePermissionMode,
    pub extraction_model: ExtractionModelId,
    pub deep_extraction_model: DeepExtractionModelId,
    pub telemetry_enabled: bool,
    pub update_manifest_url: Option<String>,
    pub version_history: VersionHistorySettings,
    pub recent_prompt_ids: Vec<PromptId>,
    pub recent_run_ids: Vec<RunId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSettings {
    pub tag_colors: HashMap<String, TagColorSlug>,
    pub default_prompt_directory: String,
    pub default_run_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveSettings {
    #[serde(flatten)]
    pub local: LocalSettings,
    pub vault_settings: VaultSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub local: LocalSettings,
    pub vault: VaultSettings,
    pub effective: EffectiveSettings,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SecretKey {
    AnthropicApiKey,
    XBearerToken,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretValidationStatus {
    Unknown,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatus {
    pub key: SecretKey,
    pub exists: bool,
    pub last_validated_at: Option<DateTime<Utc>>,
    pub validation_status: SecretValidationStatus,
}
