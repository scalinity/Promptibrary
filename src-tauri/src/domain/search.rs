//! Search input/output types per spec §9.

use serde::{Deserialize, Serialize};

use crate::ids::PromptId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Text,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPromptsInput {
    pub query: String,
    pub mode: SearchMode,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPromptsHit {
    pub prompt_id: PromptId,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub score_text: f64,
    pub score_semantic: f64,
    pub score_combined: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPromptsOutput {
    pub hits: Vec<SearchPromptsHit>,
    pub duration_ms: u32,
}
