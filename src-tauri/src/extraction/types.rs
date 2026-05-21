//! Extraction domain types — Rust mirror of `src/shared/types/extraction.ts`.
//!
//! Source of truth: spec §6. Discriminated unions use `kind`/`type` tags
//! matching the TS string literals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::prompt::{ClaudeModelId, ClaudePermissionMode, LaunchDefaults, VerifierMode};
use crate::domain::source::Source;
use crate::domain::variable::Variable;

// ─── Mode ────────────────────────────────────────────────────────────────────

/// Extraction mode (default `standard`). Deep mode caps source at 160k chars
/// and routes to Opus 4.7 instead of Sonnet 4.6. Per spec §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMode {
    Standard,
    Deep,
}

impl Default for ExtractionMode {
    fn default() -> Self {
        Self::Standard
    }
}

impl ExtractionMode {
    pub fn model(self) -> ClaudeModelId {
        match self {
            Self::Standard => ClaudeModelId::ClaudeSonnet46,
            Self::Deep => ClaudeModelId::ClaudeOpus47,
        }
    }

    pub fn max_tokens(self) -> u32 {
        match self {
            Self::Standard => 6000,
            Self::Deep => 12000,
        }
    }

    pub fn source_char_cap(self) -> usize {
        match self {
            Self::Standard => 60_000,
            Self::Deep => 160_000,
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
}

// ─── Source detection ────────────────────────────────────────────────────────

/// Result of `detect_source(url)`. Spec §6 *Source detection*.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceDetection {
    Youtube {
        #[serde(rename = "canonicalUrl")]
        canonical_url: String,
        #[serde(rename = "videoId")]
        video_id: String,
    },
    #[serde(rename = "x_twitter")]
    XTwitter {
        #[serde(rename = "canonicalUrl")]
        canonical_url: String,
        #[serde(rename = "postId")]
        post_id: String,
        username: Option<String>,
    },
    Article {
        #[serde(rename = "canonicalUrl")]
        canonical_url: String,
        hostname: String,
    },
    Unsupported {
        reason: UnsupportedSourceReason,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedSourceReason {
    InvalidUrl,
    UnsupportedScheme,
    UnsupportedHost,
}

// ─── Fetched source content ──────────────────────────────────────────────────

/// One chunk of source material handed to the LLM (transcript line, tweet,
/// article paragraph). Spec §6 *Fetched content model*.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceChunk {
    pub kind: SourceChunkKind,
    pub order: u32,
    pub text: String,
    pub url: Option<String>,
    pub timestamp_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceChunkKind {
    Title,
    Metadata,
    Transcript,
    Post,
    Article,
    Code,
    Quote,
}

/// Output of the per-source fetchers (article / youtube / x_twitter), input
/// to normalization + LLM extraction. Spec §6 *Fetched content model*.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedSourceContent {
    pub source: Source,
    pub canonical_url: String,
    pub fetched_at: DateTime<Utc>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub text: String,
    pub chunks: Vec<SourceChunk>,
    pub raw_metadata: serde_json::Map<String, serde_json::Value>,
    pub content_hash: String,
    /// `true` when this preview came from the SQLite cache rather than a
    /// fresh fetch. Lets the UI surface a "cached" badge.
    #[serde(default)]
    pub cached: bool,
}

// ─── LLM input + output ──────────────────────────────────────────────────────

/// Built from `FetchedSourceContent` after normalization; handed to the
/// Anthropic client. Spec §6 *Normalization before LLM*.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionInput {
    pub source: Source,
    pub title: Option<String>,
    pub author: Option<String>,
    pub url: String,
    pub text: String,
    pub chunks: Vec<SourceChunk>,
    pub max_candidate_count: u32,
    pub extraction_mode: ExtractionMode,
    /// SCA-906 — resolved model identifier for this extraction call.
    /// Populated from `LocalSettings.extraction_model` / `deep_extraction_model`
    /// at the IPC boundary. Old serializations without this field fall
    /// back to the default for the extraction mode via serde.
    #[serde(default = "default_model_id")]
    pub model_id: String,
}

fn default_model_id() -> String {
    // Falls back to the standard-mode default. Callers that route by
    // mode populate this explicitly via the IPC layer.
    crate::domain::prompt::ClaudeModelId::ClaudeSonnet46
        .as_wire()
        .to_string()
}

impl ExtractionInput {
    pub fn from_fetched(content: FetchedSourceContent, mode: ExtractionMode) -> Self {
        let max_candidate_count = match mode {
            ExtractionMode::Standard => 4,
            ExtractionMode::Deep => 8,
        };
        Self {
            source: content.source,
            title: content.title,
            author: content.author,
            url: content.canonical_url,
            text: content.text,
            chunks: content.chunks,
            max_candidate_count,
            model_id: mode.model().as_wire().to_string(),
            extraction_mode: mode,
        }
    }
}

/// One candidate prompt produced by the LLM extraction step. Spec §6
/// *LLM extraction step → Response schema*.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePrompt {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub tags: Vec<String>,
    pub variables: Vec<Variable>,
    pub launch_defaults_patch: LaunchDefaultsPatch,
    pub confidence: CandidateConfidence,
    pub rationale: String,
    pub source_anchors: Vec<SourceAnchor>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAnchor {
    pub chunk_order: u32,
    pub quote: String,
    pub reason: String,
}

/// Patch over the default `LaunchDefaults` for a candidate. All fields are
/// optional; the candidate editor merges these over the user's chosen
/// defaults. `destination`, when present, must be `claude_code_cli`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchDefaultsPatch {
    pub destination: Option<String>,
    pub model: Option<ClaudeModelId>,
    pub verifier_mode: Option<VerifierMode>,
    pub permission_mode: Option<ClaudePermissionMode>,
    pub allowed_tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub mcp_config_paths: Option<Vec<String>>,
    pub strict_mcp_config: Option<bool>,
    pub append_system_prompt: Option<String>,
    pub max_turns: Option<u32>,
}

impl LaunchDefaultsPatch {
    pub fn apply(&self, base: &mut LaunchDefaults) {
        if let Some(model) = self.model {
            base.model = model;
        }
        if let Some(mode) = self.verifier_mode {
            base.verifier_mode = mode;
        }
        if let Some(mode) = self.permission_mode {
            base.permission_mode = mode;
        }
        if let Some(strict) = self.strict_mcp_config {
            base.strict_mcp_config = strict;
        }
        // Forward-compat fields (allowed/disallowed tools, mcp paths,
        // append_system_prompt, max_turns) are intentionally not applied
        // here — they're surfaced in the candidate editor UI for the user
        // to review individually rather than auto-merged. Per spec §7
        // `max_turns` is also forward-compat only in V1.
    }
}

/// Top-level LLM response. Spec §6 *LLM extraction step → Response schema*.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionResponse {
    pub schema_version: u32,
    pub candidates: Vec<CandidatePrompt>,
}

// ─── Failures ────────────────────────────────────────────────────────────────

/// Discriminated failure surface used by the IPC commands; the frontend
/// maps each variant to a tailored panel per spec §6 *Failure modes*.
///
/// `rename_all` controls the variant *tag* (snake_case for the wire),
/// `rename_all_fields` controls the inner struct-variant *fields*
/// (camelCase to match the TS contract). Without the second attribute,
/// fields like `install_hint` would leak through as `install_hint` on the
/// wire while TS expects `installHint`, silently dropping the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ExtractionFailure {
    NetworkUnavailable {
        message: String,
    },
    UnsupportedSource {
        reason: UnsupportedSourceReason,
    },
    DependencyMissing {
        name: String,
        install_hint: String,
    },
    TranscriptUnavailable,
    PaywallLikely {
        preview: String,
    },
    RateLimited {
        provider: String,
        reset_at: Option<DateTime<Utc>>,
    },
    AnthropicKeyMissing,
    AnthropicAuthInvalid,
    LlmRefusal {
        excerpt: String,
    },
    MalformedModelOutput {
        raw: String,
        errors: Vec<String>,
    },
    ExtractionFailed {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_serializes_kind_tag_snake_case() {
        let d = SourceDetection::Youtube {
            canonical_url: "https://youtu.be/abc".into(),
            video_id: "abc".into(),
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["kind"], "youtube");
        assert_eq!(v["videoId"], "abc");

        let d = SourceDetection::XTwitter {
            canonical_url: "https://x.com/u/status/1".into(),
            post_id: "1".into(),
            username: Some("u".into()),
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["kind"], "x_twitter");
        assert_eq!(v["postId"], "1");

        let d = SourceDetection::Unsupported {
            reason: UnsupportedSourceReason::InvalidUrl,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["kind"], "unsupported");
        assert_eq!(v["reason"], "invalid_url");
    }

    #[test]
    fn mode_caps_match_spec() {
        assert_eq!(ExtractionMode::Standard.source_char_cap(), 60_000);
        assert_eq!(ExtractionMode::Deep.source_char_cap(), 160_000);
        assert_eq!(ExtractionMode::Standard.max_tokens(), 6000);
        assert_eq!(ExtractionMode::Deep.max_tokens(), 12000);
        assert_eq!(ExtractionMode::Standard.model(), ClaudeModelId::ClaudeSonnet46);
        assert_eq!(ExtractionMode::Deep.model(), ClaudeModelId::ClaudeOpus47);
    }

    #[test]
    fn chunk_kind_serializes_snake_case() {
        let c = SourceChunk {
            kind: SourceChunkKind::Transcript,
            order: 0,
            text: "hello".into(),
            url: None,
            timestamp_seconds: Some(12.34),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["kind"], "transcript");
        assert_eq!(v["timestampSeconds"], 12.34);
    }

    #[test]
    fn failure_dependency_missing_uses_camelcase_install_hint() {
        // Regression guard for SCA-699 — the L4 review caught that the
        // wire format was emitting `install_hint` (snake_case) because the
        // enum-level `rename_all = "snake_case"` only renames the variant
        // tag, not the fields inside struct variants. The TS contract has
        // always used camelCase (`installHint`), so the UI silently
        // dropped the install hint on every DependencyMissing. The fix is
        // `rename_all_fields = "camelCase"`; this test pins the wire format.
        let f = ExtractionFailure::DependencyMissing {
            name: "yt-dlp".into(),
            install_hint: "brew install yt-dlp".into(),
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["kind"], "dependency_missing");
        assert_eq!(v["name"], "yt-dlp");
        assert_eq!(v["installHint"], "brew install yt-dlp");
        assert!(v.get("install_hint").is_none(), "snake_case field name must NOT appear on the wire");

        // Round trip from the camelCase wire form back into Rust.
        let back: ExtractionFailure = serde_json::from_value(v).unwrap();
        match back {
            ExtractionFailure::DependencyMissing { name, install_hint } => {
                assert_eq!(name, "yt-dlp");
                assert_eq!(install_hint, "brew install yt-dlp");
            }
            _ => panic!("variant mismatch"),
        }
    }

    #[test]
    fn failure_rate_limited_uses_camelcase_reset_at() {
        use chrono::TimeZone;
        let reset = Utc.with_ymd_and_hms(2026, 5, 19, 12, 0, 0).unwrap();
        let f = ExtractionFailure::RateLimited {
            provider: "x_twitter".into(),
            reset_at: Some(reset),
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["kind"], "rate_limited");
        assert!(v["resetAt"].is_string());
        assert!(v.get("reset_at").is_none(), "snake_case field name must NOT appear on the wire");
    }

    #[test]
    fn failure_round_trip() {
        let f = ExtractionFailure::PaywallLikely {
            preview: "members only".into(),
        };
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["kind"], "paywall_likely");
        let back: ExtractionFailure = serde_json::from_value(v).unwrap();
        match back {
            ExtractionFailure::PaywallLikely { preview } => assert_eq!(preview, "members only"),
            _ => panic!("variant mismatch"),
        }
    }
}
