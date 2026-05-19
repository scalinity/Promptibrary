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
use serde_json::json;
use sqlx::SqlitePool;

use super::cache;
use super::prompts::{build_user_payload, EXTRACTION_PROMPT_VERSION, EXTRACTION_SYSTEM_PROMPT};
use super::response::{parse_and_validate, salvage_first_json_object, ValidationError};
use super::types::{ExtractionFailure, ExtractionInput, ExtractionMode, ExtractionResponse};
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
            Err(e) => return Err(AnthropicTransportError::Other(format!("keychain: {e:?}"))),
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
/// Anthropic surfaces both `retry-after` (seconds until reset, RFC7231)
/// and `anthropic-ratelimit-requests-reset` (ISO-8601 absolute). Prefer
/// the absolute timestamp; fall back to relative.
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
        let model = input.extraction_mode.model().as_wire().to_string();
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
                    Err(final_errors) => Ok(Err(ExtractionFailure::MalformedModelOutput {
                        raw: repaired_raw,
                        errors: final_errors.iter().map(|e| format!("{e:?}")).collect(),
                    })),
                }
            }
        }
    }
}

fn build_request(input: &ExtractionInput) -> AnthropicRequest {
    AnthropicRequest {
        model: input.extraction_mode.model().as_wire().to_string(),
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
    let mut error_lines = String::new();
    for e in errors {
        error_lines.push_str(&format!("- {e:?}\n"));
    }
    let repair_text = format!(
        "Your previous response did not pass validation. Return ONLY valid JSON matching the schema; no markdown, no commentary.\n\nValidation errors:\n{error_lines}\n\nYour previous response was:\n{invalid_raw}\n\nReturn the corrected JSON now."
    );
    AnthropicRequest {
        model: input.extraction_mode.model().as_wire().to_string(),
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
    use crate::domain::source::Source;
    match input.source {
        Source::Manual(_) => "manual",
        Source::Youtube(_) => "youtube",
        Source::XTwitter(_) => "x_twitter",
        Source::Article(_) => "article",
    }
}

// Convenience constructor for IPC wiring.
pub fn http_client_for_extraction() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("reqwest client builds")
}

// Suppress unused warnings on the JSON helper when used downstream.
#[allow(dead_code)]
fn _unused_helper() -> serde_json::Value {
    json!({})
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
        let mock = Arc::new(MockAnthropicTransport::new(vec![Ok(good_response())]));
        let client = AnthropicClient::new(mock.clone());
        client.extract_candidates(deep, false, None).await.unwrap().unwrap();
        let captured = mock.captured().await;
        assert_eq!(captured[0].model, "claude-opus-4-7");
        assert_eq!(captured[0].max_tokens, 12000);
    }

    // Reference CandidatePrompt to silence unused-import warnings.
    #[allow(dead_code)]
    fn _ensure_candidate_imports_compile() -> CandidatePrompt {
        CandidatePrompt {
            title: "x".into(),
            summary: "s".into(),
            body: "b".into(),
            tags: vec![],
            variables: vec![],
            launch_defaults_patch: LaunchDefaultsPatch::default(),
            confidence: CandidateConfidence::Low,
            rationale: "r".into(),
            source_anchors: vec![],
        }
    }
}
