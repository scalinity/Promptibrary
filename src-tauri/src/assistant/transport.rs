//! Streaming Anthropic `/v1/messages` HTTP client for the assistant.
//!
//! Independent of `extraction::anthropic` because the two clients have
//! different request shapes (this one carries `tools` and supports
//! tool-result/tool-use content blocks) and different response handling
//! (this one is streaming SSE, that one is single-shot JSON). The
//! extraction client stays untouched.
//!
//! Streaming is plumbed through a `tokio::sync::mpsc::Sender` rather than
//! returning an `impl Stream` so the trait is easy to mock and the IPC
//! layer can forward events to Tauri without re-pinning a stream type.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::streaming::{AssistantStreamEvent, SseParseError, SseParser};
use crate::settings::keychain::{get_secret, SecretKey};
use crate::settings::secret_store::SecretStore;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

// ─── Request schema ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct StreamingAnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system: String,
    pub messages: Vec<AssistantMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    /// Always `true` for this transport. Serialized so the field appears in
    /// the request body; Anthropic requires the explicit `stream` flag.
    pub stream: bool,
}

impl StreamingAnthropicRequest {
    /// Construct a request with the streaming flag set. Callers fill the
    /// remaining fields and pass to `StreamingAnthropicTransport::stream`.
    pub fn new(
        model: impl Into<String>,
        max_tokens: u32,
        temperature: f32,
        system: impl Into<String>,
    ) -> Self {
        Self {
            model: model.into(),
            max_tokens,
            temperature,
            system: system.into(),
            messages: Vec::new(),
            tools: Vec::new(),
            stream: true,
        }
    }
}

/// Conversation message. `role` is `"user"` or `"assistant"`; `content` is
/// a list of typed blocks (text, tool_use, tool_result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String,
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Tool definition advertised to Anthropic. Shape matches the Anthropic
/// Messages API tool schema exactly so frontends can author tools without
/// translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StreamingError {
    KeyMissing,
    AuthInvalid,
    RateLimited {
        reset_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    Network(String),
    InvalidStream(String),
    Other(String),
}

impl From<SseParseError> for StreamingError {
    fn from(e: SseParseError) -> Self {
        StreamingError::InvalidStream(format!("{e:?}"))
    }
}

// ─── Transport trait ────────────────────────────────────────────────────────

/// Sends a streaming request and pushes each parsed event into `events_tx`.
/// Returns once the stream completes or an unrecoverable error occurs.
///
/// If the receiver is dropped mid-stream the transport stops reading the
/// HTTP body (the next `send` will fail) — that's the cancellation hook.
#[async_trait]
pub trait StreamingAnthropicTransport: Send + Sync {
    async fn stream(
        &self,
        request: StreamingAnthropicRequest,
        events_tx: mpsc::Sender<AssistantStreamEvent>,
    ) -> Result<(), StreamingError>;
}

// ─── Production HTTP implementation ─────────────────────────────────────────

pub struct HttpStreamingAnthropicTransport {
    http: reqwest::Client,
    secret_store: Arc<dyn SecretStore>,
}

impl HttpStreamingAnthropicTransport {
    pub fn new(http: reqwest::Client, secret_store: Arc<dyn SecretStore>) -> Self {
        Self { http, secret_store }
    }
}

#[async_trait]
impl StreamingAnthropicTransport for HttpStreamingAnthropicTransport {
    async fn stream(
        &self,
        request: StreamingAnthropicRequest,
        events_tx: mpsc::Sender<AssistantStreamEvent>,
    ) -> Result<(), StreamingError> {
        let api_key = match get_secret(self.secret_store.as_ref(), SecretKey::AnthropicApiKey) {
            Ok(Some(k)) => k,
            Ok(None) => return Err(StreamingError::KeyMissing),
            Err(e) => return Err(StreamingError::Other(format!("keychain: {e:?}"))),
        };

        let resp = self
            .http
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    StreamingError::Network(format!("{e}"))
                } else {
                    StreamingError::Other(format!("{e}"))
                }
            })?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(StreamingError::AuthInvalid);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let reset_at = parse_rate_limit_reset(resp.headers());
            return Err(StreamingError::RateLimited { reset_at });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(StreamingError::Other(format!(
                "anthropic API returned {status}: {body}"
            )));
        }

        let mut parser = SseParser::new();
        let mut resp = resp;
        let mut pump_error: Option<StreamingError> = None;
        let mut cancelled = false;

        // Pump loop — collect bytes, feed parser, forward events. Any
        // error (network or SSE parse) breaks out into a single drain
        // point below so SCA-955 holds: parser.finish() runs no matter
        // how the loop exits.
        'pump: loop {
            let chunk_result = resp.chunk().await;
            match chunk_result {
                Ok(None) => break 'pump,
                Ok(Some(bytes)) => match parser.feed(&bytes) {
                    Ok(events) => {
                        for ev in events {
                            if events_tx.send(ev).await.is_err() {
                                cancelled = true;
                                break 'pump;
                            }
                        }
                    }
                    Err(e) => {
                        pump_error = Some(StreamingError::from(e));
                        break 'pump;
                    }
                },
                Err(e) => {
                    pump_error = Some(StreamingError::Network(format!("{e}")));
                    break 'pump;
                }
            }
        }

        // SCA-955 — always drain the trailing frame. If the pump
        // succeeded, propagate any finish() error. If the pump failed,
        // finish's error (if any) is subordinated to the pump error so
        // the original cause stays visible.
        match parser.finish() {
            Ok(events) => {
                if !cancelled {
                    for ev in events {
                        if events_tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                if pump_error.is_none() {
                    return Err(StreamingError::from(e));
                }
                // pump_error wins; drop finish's error.
            }
        }

        if let Some(e) = pump_error {
            return Err(e);
        }
        Ok(())
    }
}

/// Same logic as `extraction::anthropic::parse_anthropic_rate_limit_reset`,
/// duplicated here to keep the assistant transport independent of the
/// extraction module's internals.
fn parse_rate_limit_reset(
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

// ─── Mock transport for tests + IPC integration ─────────────────────────────

/// Replays a canned event sequence. Used by tests and by the streaming-IPC
/// integration suite in `commands::assistant`.
pub struct CannedStreamingTransport {
    events: tokio::sync::Mutex<Option<Vec<AssistantStreamEvent>>>,
    initial_error: tokio::sync::Mutex<Option<StreamingError>>,
    captured: tokio::sync::Mutex<Vec<StreamingAnthropicRequest>>,
}

impl CannedStreamingTransport {
    pub fn new(events: Vec<AssistantStreamEvent>) -> Self {
        Self {
            events: tokio::sync::Mutex::new(Some(events)),
            initial_error: tokio::sync::Mutex::new(None),
            captured: Default::default(),
        }
    }

    pub fn with_error(error: StreamingError) -> Self {
        Self {
            events: tokio::sync::Mutex::new(None),
            initial_error: tokio::sync::Mutex::new(Some(error)),
            captured: Default::default(),
        }
    }

    pub async fn captured(&self) -> Vec<StreamingAnthropicRequest> {
        self.captured.lock().await.clone()
    }
}

#[async_trait]
impl StreamingAnthropicTransport for CannedStreamingTransport {
    async fn stream(
        &self,
        request: StreamingAnthropicRequest,
        events_tx: mpsc::Sender<AssistantStreamEvent>,
    ) -> Result<(), StreamingError> {
        self.captured.lock().await.push(request);
        if let Some(err) = self.initial_error.lock().await.take() {
            return Err(err);
        }
        let events = self.events.lock().await.take().unwrap_or_default();
        for ev in events {
            if events_tx.send(ev).await.is_err() {
                return Ok(());
            }
        }
        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> StreamingAnthropicRequest {
        let mut r = StreamingAnthropicRequest::new("claude-sonnet-4-6", 4096, 0.3, "sys");
        r.messages.push(AssistantMessage {
            role: "user".into(),
            content: vec![MessageContent::Text {
                text: "hi".into(),
            }],
        });
        r
    }

    #[tokio::test]
    async fn canned_transport_emits_events_in_order() {
        let canned = vec![
            AssistantStreamEvent::MessageStart {
                message_id: "m1".into(),
                model: "claude-sonnet-4-6".into(),
            },
            AssistantStreamEvent::TextDelta {
                index: 0,
                text: "ok".into(),
            },
            AssistantStreamEvent::MessageStop,
        ];
        let transport = CannedStreamingTransport::new(canned.clone());
        let (tx, mut rx) = mpsc::channel(16);
        transport.stream(req(), tx).await.unwrap();
        let mut received = Vec::new();
        while let Some(ev) = rx.recv().await {
            received.push(ev);
        }
        assert_eq!(received, canned);
    }

    #[tokio::test]
    async fn canned_transport_returns_initial_error() {
        let transport = CannedStreamingTransport::with_error(StreamingError::AuthInvalid);
        let (tx, _rx) = mpsc::channel::<AssistantStreamEvent>(16);
        let err = transport.stream(req(), tx).await.unwrap_err();
        assert!(matches!(err, StreamingError::AuthInvalid));
    }

    #[tokio::test]
    async fn dropping_receiver_stops_stream_gracefully() {
        let many: Vec<_> = (0..100)
            .map(|i| AssistantStreamEvent::TextDelta {
                index: 0,
                text: format!("{i}"),
            })
            .collect();
        let transport = CannedStreamingTransport::new(many);
        let (tx, rx) = mpsc::channel::<AssistantStreamEvent>(4);
        drop(rx);
        // Stream should return Ok even though the receiver is gone.
        transport.stream(req(), tx).await.unwrap();
    }

    #[test]
    fn request_serializes_with_tools_and_stream_true() {
        let mut r = req();
        r.tools.push(ToolDefinition {
            name: "get_current_prompt".into(),
            description: "Read the open prompt".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        });
        let body = serde_json::to_value(&r).unwrap();
        assert_eq!(body["stream"], true);
        assert_eq!(body["tools"][0]["name"], "get_current_prompt");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    }

    #[test]
    fn tools_omitted_when_empty() {
        let body = serde_json::to_value(req()).unwrap();
        assert!(body.get("tools").is_none(), "tools should be skipped when empty");
    }

    #[test]
    fn tool_use_and_tool_result_serialize_correctly() {
        let r = StreamingAnthropicRequest {
            model: "m".into(),
            max_tokens: 1,
            temperature: 0.0,
            system: String::new(),
            messages: vec![
                AssistantMessage {
                    role: "assistant".into(),
                    content: vec![MessageContent::ToolUse {
                        id: "toolu_1".into(),
                        name: "x".into(),
                        input: serde_json::json!({"a": 1}),
                    }],
                },
                AssistantMessage {
                    role: "user".into(),
                    content: vec![MessageContent::ToolResult {
                        tool_use_id: "toolu_1".into(),
                        content: "done".into(),
                        is_error: None,
                    }],
                },
            ],
            tools: vec![],
            stream: true,
        };
        let body = serde_json::to_value(&r).unwrap();
        assert_eq!(body["messages"][0]["content"][0]["type"], "tool_use");
        assert_eq!(body["messages"][0]["content"][0]["id"], "toolu_1");
        assert_eq!(body["messages"][1]["content"][0]["type"], "tool_result");
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "toolu_1");
        // is_error: None should be omitted.
        assert!(body["messages"][1]["content"][0].get("is_error").is_none());
    }
}
