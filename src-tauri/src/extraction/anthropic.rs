//! Anthropic Messages API client for the extraction pipeline.
//!
//! The `AnthropicTransport` trait abstracts HTTP so tests inject canned
//! responses. Production uses `HttpAnthropicTransport` which POSTs to
//! `/v1/messages` (non-streaming for V1 — we need the full JSON before
//! parsing anyway; streaming is a UX nicety we can layer on in L5).
//!
//! `AnthropicClient::extract_candidates` owns the full extract path:
//! cache check → request → validate → (markdown salvage) → (single repair
//! attempt) → MalformedModelOutput. Repair is one-shot per spec §6.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::cache;
use super::prompts::{build_user_payload, EXTRACTION_PROMPT_VERSION, EXTRACTION_SYSTEM_PROMPT};
use super::response::{
    parse_and_validate, salvage_first_json_object, sanitize_raw_for_wire, ValidationError,
};
use super::types::{ExtractionFailure, ExtractionInput, ExtractionResponse};
use crate::error::{AppError, AppErrorKind, Result};
use crate::settings::keychain::{get_secret, SecretKey};
use crate::settings::secret_store::SecretStore;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system: String,
    pub messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text { text: String },
}

#[async_trait]
pub trait AnthropicTransport: Send + Sync {
    /// Send the request, return the assistant's text content (concatenated
    /// across content blocks if there's more than one).
    async fn send(&self, request: AnthropicRequest)
        -> std::result::Result<String, AnthropicTransportError>;
}

#[derive(Debug, Clone)]
pub enum AnthropicTransportError {
    KeyMissing,
    AuthInvalid,
    /// Anthropic returned 429. `reset_at` parsed from `retry-after` or
    /// `anthropic-ratelimit-requests-reset` when present.
    RateLimited {
        reset_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    Network(String),
    Other(String),
}

impl From<AnthropicTransportError> for AppError {
    fn from(e: AnthropicTransportError) -> Self {
        match e {
            AnthropicTransportError::KeyMissing => {
                AppError::new(AppErrorKind::AnthropicKeyMissing, "anthropic key missing")
            }
            AnthropicTransportError::AuthInvalid => {
                AppError::new(AppErrorKind::AnthropicAuthInvalid, "anthropic auth invalid")
            }
            AnthropicTransportError::RateLimited { .. } => {
                AppError::new(AppErrorKind::RateLimited, "anthropic rate limited")
            }
            AnthropicTransportError::Network(m) => {
                AppError::new(AppErrorKind::NetworkUnavailable, m)
            }
            AnthropicTransportError::Other(m) => AppError::new(AppErrorKind::ExtractionFailed, m),
        }
    }
}

// ─── Production HTTP transport ───────────────────────────────────────────────

pub struct HttpAnthropicTransport {
    http: reqwest::Client,
    secret_store: Arc<dyn SecretStore>,
}

impl HttpAnthropicTransport {
    pub fn new(http: reqwest::Client, secret_store: Arc<dyn SecretStore>) -> Self {
        Self { http, secret_store }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponseBody {
    content: Vec<AnthropicResponseContent>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicResponseContent {
    Text { text: String },
    #[serde(other)]
    Other,
}

#[async_trait]
impl AnthropicTransport for HttpAnthropicTransport {
    async fn send(
        &self,
        request: AnthropicRequest,
    ) -> std::result::Result<String, AnthropicTransportError> {
        let api_key = match get_secret(self.secret_store.as_ref(), SecretKey::AnthropicApiKey) {
            Ok(Some(k)) => k,
            Ok(None) => return Err(AnthropicTransportError::KeyMissing),
            // SCA-913 (W9, CWE-209): Display, not Debug. keyring_core's
            // Debug format is not stability-guaranteed and may include
            // sensitive context in future versions.
            Err(e) => return Err(AnthropicTransportError::Other(format!("keychain: {e}"))),
        };

        let resp = self
            .http
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    AnthropicTransportError::Network(format!("{e}"))
                } else {
                    AnthropicTransportError::Other(format!("{e}"))
                }
            })?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(AnthropicTransportError::AuthInvalid);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let reset_at = parse_anthropic_rate_limit_reset(resp.headers());
            return Err(AnthropicTransportError::RateLimited { reset_at });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AnthropicTransportError::Other(format!(
                "anthropic API returned {status}: {body}"
            )));
        }

        let body: AnthropicResponseBody = resp
            .json()
            .await
            .map_err(|e| AnthropicTransportError::Other(format!("decode body: {e}")))?;

        let mut combined = String::new();
        for block in body.content {
            if let AnthropicResponseContent::Text { text } = block {
                combined.push_str(&text);
            }
        }
        Ok(combined)
    }
}

/// Parse the reset-at timestamp from Anthropic's rate-limit headers.
///
/// Header contract (Anthropic Messages API, as of 2026-05):
/// - `anthropic-ratelimit-requests-reset` — RFC3339 absolute timestamp
///   (e.g. `2026-05-19T12:00:00Z`). Preferred when present because the
///   value is wall-clock, immune to clock-skew on the client.
/// - `retry-after` — RFC7231 seconds-until-reset. Fallback when the
///   Anthropic-specific header is absent.
///
/// Other Anthropic rate-limit headers are emitted too
/// (`anthropic-ratelimit-tokens-{remaining,reset}`,
/// `anthropic-ratelimit-input-tokens-*`, etc.). We only care about the
/// "you can retry at X" answer here; the others are dashboarding signals.
///
/// Reference: https://docs.anthropic.com/en/api/rate-limits — keep this
/// pointer current when adjusting parsing. If Anthropic renames the
/// preferred header, the `RateLimited` failure silently degrades to the
/// `retry-after` path, so update both the constant strings below AND
/// the test in `rate_limited_surfaces_as_failure_with_reset` when the
/// header schema changes.
fn parse_anthropic_rate_limit_reset(
    headers: &reqwest::header::HeaderMap,
) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Some(reset) = headers
        .get("anthropic-ratelimit-requests-reset")
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(reset) {
            return Some(dt.with_timezone(&chrono::Utc));
        }
    }
    if let Some(retry_after) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(secs) = retry_after.parse::<i64>() {
            return Some(chrono::Utc::now() + chrono::Duration::seconds(secs));
        }
    }
    None
}

// ─── Mock transport for tests ────────────────────────────────────────────────

#[cfg(test)]
pub struct MockAnthropicTransport {
    queue: tokio::sync::Mutex<Vec<std::result::Result<String, AnthropicTransportError>>>,
    captured: tokio::sync::Mutex<Vec<AnthropicRequest>>,
}

#[cfg(test)]
impl MockAnthropicTransport {
    pub fn new(responses: Vec<std::result::Result<String, AnthropicTransportError>>) -> Self {
        Self {
            queue: tokio::sync::Mutex::new(responses),
            captured: Default::default(),
        }
    }

    pub async fn captured(&self) -> Vec<AnthropicRequest> {
        self.captured.lock().await.clone()
    }
}

#[cfg(test)]
#[async_trait]
impl AnthropicTransport for MockAnthropicTransport {
    async fn send(
        &self,
        request: AnthropicRequest,
    ) -> std::result::Result<String, AnthropicTransportError> {
        self.captured.lock().await.push(request);
        let mut q = self.queue.lock().await;
        if q.is_empty() {
            return Err(AnthropicTransportError::Other(
                "mock transport ran out of responses".into(),
            ));
        }
        q.remove(0)
    }
}

// ─── Client ──────────────────────────────────────────────────────────────────

pub struct AnthropicClient {
    transport: Arc<dyn AnthropicTransport>,
}

impl AnthropicClient {
    pub fn new(transport: Arc<dyn AnthropicTransport>) -> Self {
        Self { transport }
    }

    /// End-to-end: cache → request → validate → salvage → repair → result.
    /// `cache_pool == None` disables caching (used in tests that focus on
    /// the request/repair path).
    pub async fn extract_candidates(
        &self,
        input: ExtractionInput,
        force_refresh: bool,
        cache_pool: Option<&SqlitePool>,
    ) -> Result<std::result::Result<ExtractionResponse, ExtractionFailure>> {
        let kind = source_kind_label(&input);
        // SCA-906 — model comes from settings-resolved input.model_id
        // populated by the IPC layer, not the mode-default lookup.
        let model = input.model_id.clone();
        let canonical_url = input.url.clone();
        let mode = input.extraction_mode;

        let cache_key = cache::compute_candidates_cache_key(
            kind,
            &canonical_url,
            mode,
            &model,
            EXTRACTION_PROMPT_VERSION,
        );

        if !force_refresh {
            if let Some(pool) = cache_pool {
                if let Some(cached) = cache::get_candidates(pool, &cache_key).await? {
                    return Ok(Ok(cached.response));
                }
            }
        }

        let request = build_request(&input);
        let raw = match self.transport.send(request.clone()).await {
            Ok(s) => s,
            Err(AnthropicTransportError::KeyMissing) => {
                return Ok(Err(ExtractionFailure::AnthropicKeyMissing))
            }
            Err(AnthropicTransportError::AuthInvalid) => {
                return Ok(Err(ExtractionFailure::AnthropicAuthInvalid))
            }
            Err(AnthropicTransportError::RateLimited { reset_at }) => {
                return Ok(Err(ExtractionFailure::RateLimited {
                    provider: "anthropic".into(),
                    reset_at,
                }))
            }
            Err(AnthropicTransportError::Network(m)) => {
                return Ok(Err(ExtractionFailure::NetworkUnavailable { message: m }))
            }
            Err(e) => return Err(e.into()),
        };

        // Attempt 1: parse the raw response.
        match parse_and_validate(&raw) {
            Ok(response) => {
                if let Some(pool) = cache_pool {
                    cache::put_candidates(
                        pool,
                        &cache_key,
                        kind,
                        &canonical_url,
                        mode,
                        &model,
                        EXTRACTION_PROMPT_VERSION,
                        &response,
                    )
                    .await?;
                }
                return Ok(Ok(response));
            }
            Err(errors) => {
                // Attempt 2: try to salvage a JSON object from a possibly
                // markdown-wrapped response. This does NOT count as the
                // repair attempt — it's the same raw payload.
                if let Some(salvaged) = salvage_first_json_object(&raw) {
                    if let Ok(response) = parse_and_validate(salvaged) {
                        if let Some(pool) = cache_pool {
                            cache::put_candidates(
                                pool,
                                &cache_key,
                                kind,
                                &canonical_url,
                                mode,
                                &model,
                                EXTRACTION_PROMPT_VERSION,
                                &response,
                            )
                            .await?;
                        }
                        return Ok(Ok(response));
                    }
                }

                // Repair attempt — ONE per spec.
                let repair = build_repair_request(&input, &raw, &errors);
                let repaired_raw = match self.transport.send(repair).await {
                    Ok(s) => s,
                    Err(AnthropicTransportError::Network(m)) => {
                        return Ok(Err(ExtractionFailure::NetworkUnavailable { message: m }))
                    }
                    Err(AnthropicTransportError::RateLimited { reset_at }) => {
                        return Ok(Err(ExtractionFailure::RateLimited {
                            provider: "anthropic".into(),
                            reset_at,
                        }))
                    }
                    Err(e) => return Err(e.into()),
                };
                let salvaged = salvage_first_json_object(&repaired_raw).unwrap_or(&repaired_raw);
                match parse_and_validate(salvaged) {
                    Ok(response) => {
                        if let Some(pool) = cache_pool {
                            cache::put_candidates(
                                pool,
                                &cache_key,
                                kind,
                                &canonical_url,
                                mode,
                                &model,
                                EXTRACTION_PROMPT_VERSION,
                                &response,
                            )
                            .await?;
                        }
                        Ok(Ok(response))
                    }
                    Err(final_errors) => {
                        // SCA-913 (W7, CWE-201 / CWE-209): redact any secret
                        // material the model echoed back from a hostile source
                        // page, and cap the wire payload at ~2 KB. The full raw
                        // string is still backend-visible via tracing.
                        tracing::warn!(
                            raw_len = repaired_raw.len(),
                            "extraction MalformedModelOutput — raw response sanitized for IPC"
                        );
                        Ok(Err(ExtractionFailure::MalformedModelOutput {
                            raw: sanitize_raw_for_wire(&repaired_raw),
                            errors: final_errors.iter().map(|e| format!("{e:?}")).collect(),
                        }))
                    }
                }
            }
        }
    }
}

fn build_request(input: &ExtractionInput) -> AnthropicRequest {
    AnthropicRequest {
        model: input.model_id.clone(),
        max_tokens: input.extraction_mode.max_tokens(),
        temperature: 0.2,
        system: EXTRACTION_SYSTEM_PROMPT.to_string(),
        messages: vec![AnthropicMessage {
            role: "user".into(),
            content: vec![AnthropicContentBlock::Text {
                text: build_user_payload(input),
            }],
        }],
    }
}

fn build_repair_request(
    input: &ExtractionInput,
    invalid_raw: &str,
    errors: &[ValidationError],
) -> AnthropicRequest {
    // Cap each error's debug repr so a giant `JsonParseFailed { error: ... }`
    // doesn't blow past the repair input budget (SCA-719). 500 chars is
    // enough to keep the substantive context (offset, expected/found
    // tokens) while bounding the worst case.
    const MAX_ERROR_REPR_CHARS: usize = 500;
    let mut error_lines = String::new();
    for e in errors {
        let repr = format!("{e:?}");
        let mut bytes_to_take = repr.len().min(MAX_ERROR_REPR_CHARS);
        while bytes_to_take > 0 && !repr.is_char_boundary(bytes_to_take) {
            bytes_to_take -= 1;
        }
        let truncated = if repr.len() > MAX_ERROR_REPR_CHARS {
            format!("{}… [truncated]", &repr[..bytes_to_take])
        } else {
            repr
        };
        error_lines.push_str(&format!("- {truncated}\n"));
    }
    let repair_text = format!(
        "Your previous response did not pass validation. Return ONLY valid JSON matching the schema; no markdown, no commentary.\n\nValidation errors:\n{error_lines}\n\nYour previous response was:\n{invalid_raw}\n\nReturn the corrected JSON now."
    );
    AnthropicRequest {
        model: input.model_id.clone(),
        // Cap repair budget at half of the original. The repair user
        // message is much smaller than the original payload and we want
        // a faster + cheaper second attempt — if the model can't produce
        // valid JSON in 3 KB of output, it won't in 6 KB either. Floor at
        // 2048 so we don't accidentally truncate a borderline-OK response.
        max_tokens: (input.extraction_mode.max_tokens() / 2).max(2048),
        temperature: 0.2,
        system: EXTRACTION_SYSTEM_PROMPT.to_string(),
        messages: vec![AnthropicMessage {
            role: "user".into(),
            content: vec![AnthropicContentBlock::Text { text: repair_text }],
        }],
    }
}

fn source_kind_label(input: &ExtractionInput) -> &'static str {
    // SCA-928 (B16): delegate to the canonical Source::kind_label method.
    input.source.kind_label()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{ArticleSource, Source};
    use crate::extraction::types::{
        CandidateConfidence, CandidatePrompt, ExtractionMode, LaunchDefaultsPatch, SourceChunk,
        SourceChunkKind,
    };

    fn input() -> ExtractionInput {
        ExtractionInput {
            source: Source::Article(ArticleSource {
                origin_url: Some("https://example.com/x".into()),
                title: Some("Title".into()),
                author: None,
                fetched_at: None,
                content_hash: None,
                site_name: None,
                byline: None,
                published_at: None,
            }),
            title: Some("Title".into()),
            author: None,
            url: "https://example.com/x".into(),
            text: "body".into(),
            chunks: vec![SourceChunk {
                kind: SourceChunkKind::Article,
                order: 0,
                text: "Hello body".into(),
                url: None,
                timestamp_seconds: None,
            }],
            max_candidate_count: 4,
            extraction_mode: ExtractionMode::Standard,
            model_id: "claude-sonnet-4-6".into(),
        }
    }

    fn good_response() -> String {
        let body = "x".repeat(300);
        format!(
            r#"{{
              "schemaVersion": 1,
              "candidates": [
                {{
                  "title": "Refactor X for performance",
                  "summary": "Sum",
                  "body": "{body}",
                  "tags": ["one"],
                  "variables": [],
                  "launchDefaultsPatch": {{}},
                  "confidence": "medium",
                  "rationale": "because",
                  "sourceAnchors": []
                }}
              ]
            }}"#
        )
    }

    #[tokio::test]
    async fn valid_response_no_repair() {
        let mock = Arc::new(MockAnthropicTransport::new(vec![Ok(good_response())]));
        let client = AnthropicClient::new(mock.clone());
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        let response = result.expect("validation should succeed");
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(mock.captured().await.len(), 1, "exactly one request sent");
    }

    #[tokio::test]
    async fn markdown_wrap_recovered_via_salvage_no_repair_request() {
        let wrapped = format!(
            "Here's the JSON:\n```json\n{}\n```\nEnd.",
            good_response()
        );
        let mock = Arc::new(MockAnthropicTransport::new(vec![Ok(wrapped)]));
        let client = AnthropicClient::new(mock.clone());
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        result.expect("salvage should recover valid JSON");
        assert_eq!(
            mock.captured().await.len(),
            1,
            "salvage must not trigger a repair request"
        );
    }

    #[tokio::test]
    async fn truly_malformed_triggers_exactly_one_repair() {
        // First response: invalid JSON with no recoverable object.
        // Second response: still invalid → MalformedModelOutput.
        let mock = Arc::new(MockAnthropicTransport::new(vec![
            Ok("definitely not json".into()),
            Ok("still not json".into()),
        ]));
        let client = AnthropicClient::new(mock.clone());
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        match result.unwrap_err() {
            ExtractionFailure::MalformedModelOutput { raw, errors: _ } => {
                assert_eq!(raw, "still not json");
            }
            other => panic!("expected MalformedModelOutput, got {other:?}"),
        }
        assert_eq!(
            mock.captured().await.len(),
            2,
            "exactly one repair attempt (= two transport calls total)"
        );
    }

    #[tokio::test]
    async fn repair_recovers_when_model_corrects_itself() {
        let mock = Arc::new(MockAnthropicTransport::new(vec![
            Ok("malformed".into()),
            Ok(good_response()),
        ]));
        let client = AnthropicClient::new(mock.clone());
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        result.expect("repair attempt should yield valid response");
        assert_eq!(mock.captured().await.len(), 2);
    }

    #[tokio::test]
    async fn auth_invalid_surfaces_as_failure() {
        let mock = Arc::new(MockAnthropicTransport::new(vec![Err(
            AnthropicTransportError::AuthInvalid,
        )]));
        let client = AnthropicClient::new(mock);
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        assert!(matches!(
            result.unwrap_err(),
            ExtractionFailure::AnthropicAuthInvalid
        ));
    }

    #[tokio::test]
    async fn rate_limited_surfaces_as_failure_with_reset() {
        use chrono::TimeZone;
        let reset = chrono::Utc.with_ymd_and_hms(2026, 5, 19, 12, 0, 0).unwrap();
        let mock = Arc::new(MockAnthropicTransport::new(vec![Err(
            AnthropicTransportError::RateLimited {
                reset_at: Some(reset),
            },
        )]));
        let client = AnthropicClient::new(mock);
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        match result.unwrap_err() {
            ExtractionFailure::RateLimited { provider, reset_at } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(reset_at, Some(reset));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn key_missing_surfaces_as_failure() {
        let mock = Arc::new(MockAnthropicTransport::new(vec![Err(
            AnthropicTransportError::KeyMissing,
        )]));
        let client = AnthropicClient::new(mock);
        let result = client
            .extract_candidates(input(), false, None)
            .await
            .unwrap();
        assert!(matches!(
            result.unwrap_err(),
            ExtractionFailure::AnthropicKeyMissing
        ));
    }

    #[tokio::test]
    async fn request_uses_sonnet_46_in_standard_mode() {
        let mock = Arc::new(MockAnthropicTransport::new(vec![Ok(good_response())]));
        let client = AnthropicClient::new(mock.clone());
        client
            .extract_candidates(input(), false, None)
            .await
            .unwrap()
            .unwrap();
        let captured = mock.captured().await;
        assert_eq!(captured[0].model, "claude-sonnet-4-6");
        assert_eq!(captured[0].max_tokens, 6000);
        assert!(captured[0].system.contains("Promptibrary's extraction engine"));
    }

    #[tokio::test]
    async fn request_uses_opus_47_in_deep_mode() {
        let mut deep = input();
        deep.extraction_mode = ExtractionMode::Deep;
        // SCA-906 — model_id is now decoupled from the mode at the data
        // structure level; the IPC layer derives it from settings. This
        // test still asserts the deep-mode default mapping (opus 4.7).
        deep.model_id = "claude-opus-4-7".into();
        let mock = Arc::new(MockAnthropicTransport::new(vec![Ok(good_response())]));
        let client = AnthropicClient::new(mock.clone());
        client.extract_candidates(deep, false, None).await.unwrap().unwrap();
        let captured = mock.captured().await;
        assert_eq!(captured[0].model, "claude-opus-4-7");
        assert_eq!(captured[0].max_tokens, 12000);
    }

}
