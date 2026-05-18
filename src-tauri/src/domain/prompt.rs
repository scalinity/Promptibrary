//! Prompt + LaunchDefaults per spec §4 "Prompt".

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::PromptId;

use super::source::Source;
use super::variable::Variable;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchDestination {
    ClaudeCodeCli,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaudeModelId {
    #[serde(rename = "claude-sonnet-4-6")]
    ClaudeSonnet46,
    #[serde(rename = "claude-opus-4-7")]
    ClaudeOpus47,
    #[serde(rename = "sonnet")]
    Sonnet,
    #[serde(rename = "opus")]
    Opus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClaudePermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Auto,
    DontAsk,
    BypassPermissions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerifierMode {
    Off,
    ManualUltrareviewAfterRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudePermissionRule {
    Tool { tool: ClaudeToolName },
    ToolSpecifier { rule: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaudeToolName {
    Read,
    Edit,
    Write,
    WebFetch,
    Bash,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchDefaults {
    pub destination: LaunchDestination,
    pub model: ClaudeModelId,
    pub verifier_mode: VerifierMode,
    pub working_directory: Option<PathBuf>,
    pub additional_directories: Vec<PathBuf>,
    pub permission_mode: ClaudePermissionMode,
    pub allowed_tools: Vec<ClaudePermissionRule>,
    pub disallowed_tools: Vec<ClaudePermissionRule>,
    pub mcp_config_paths: Vec<PathBuf>,
    pub strict_mcp_config: bool,
    pub append_system_prompt: Option<String>,
    /// Spec §7 risk note: do **not** pass this to the current `claude` CLI invocation.
    /// Stored for forward compatibility.
    pub max_turns: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTelemetrySummary {
    pub launch_count: u32,
    pub last_used_at: Option<DateTime<Utc>>,
    pub success_rate: Option<f64>,
    pub avg_run_seconds: Option<f64>,
    pub avg_token_count: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub id: PromptId,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body: String,
    pub vault_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub source: Source,
    pub variables: Vec<Variable>,
    pub launch_defaults: LaunchDefaults,
    pub telemetry: PromptTelemetrySummary,
    pub checksum_sha256: String,
}
