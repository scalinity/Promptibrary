//! LaunchProfile + ResolvedVariableValue per spec §4 "LaunchProfile".

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::PromptId;

use super::prompt::{
    ClaudeModelId, ClaudePermissionMode, ClaudePermissionRule, LaunchDestination, VerifierMode,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedVariableValue {
    File { key: String, value: PathBuf },
    Folder { key: String, value: PathBuf },
    Text { key: String, value: String },
    Multiline { key: String, value: String },
    Select { key: String, value: String },
    Bool { key: String, value: bool },
    Number { key: String, value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfile {
    pub prompt_id: PromptId,
    pub prompt_title: String,
    pub prompt_version_checksum: String,
    pub template_body: String,
    pub inline_tweak_body: Option<String>,
    pub resolved_prompt: String,
    pub variable_values: Vec<ResolvedVariableValue>,
    pub working_directory: PathBuf,
    pub additional_directories: Vec<PathBuf>,
    pub destination: LaunchDestination,
    pub model: ClaudeModelId,
    pub verifier_mode: VerifierMode,
    pub permission_mode: ClaudePermissionMode,
    pub allowed_tools: Vec<ClaudePermissionRule>,
    pub disallowed_tools: Vec<ClaudePermissionRule>,
    pub mcp_config_paths: Vec<PathBuf>,
    pub strict_mcp_config: bool,
    pub append_system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub launched_at: DateTime<Utc>,
}
