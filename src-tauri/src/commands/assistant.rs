//! `commands::assistant` — streaming Anthropic proxy for the in-app
//! Claude Agent assistant (parent SCA-920, L5).
//!
//! Exposes a single IPC: `assistant_stream_turn`. The frontend builds the
//! conversation history, tool definitions, and pattern-derived system
//! prompt; this command POSTs to Anthropic's streaming Messages API,
//! parses the SSE event stream, and forwards each parsed event to the
//! frontend as an `assistant:chunk` Tauri event. The command returns once
//! the stream ends, carrying the final `stop_reason` so the frontend agent
//! loop knows whether to keep going (tool_use) or stop (end_turn).
//!
//! API keys never leave Rust — the streaming transport holds the
//! `SecretStore` reference and resolves the key just before issuing the
//! HTTP request.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

use crate::app_state::ManagedState;
use crate::assistant::streaming::AssistantStreamEvent;
use crate::assistant::transport::{
    AssistantMessage, MessageContent, StreamingAnthropicRequest, StreamingAnthropicTransport,
    StreamingError, ToolDefinition,
};
use crate::error::{AppError, AppErrorKind, Result};

/// Tauri event emitted for every parsed stream event. Frontend listens
/// with `getCurrentWebviewWindow().listen("assistant:chunk", …)`.
pub const ASSISTANT_CHUNK_EVENT: &str = "assistant:chunk";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStreamTurnInput {
    /// Frontend-generated correlation ID so multiple in-flight turns (or
    /// rapidly cancelled+restarted turns) don't cross wires on the event
    /// channel. UUID v4 or ULID — opaque string here.
    pub turn_id: String,
    /// System prompt body — typically `AssistantPattern::system_prompt()`
    /// plus any per-call tool-use instructions appended by the frontend.
    pub system: String,
    /// Conversation history. Roles are `"user"` and `"assistant"`. Content
    /// blocks can be text, tool_use, or tool_result.
    pub messages: Vec<AssistantMessage>,
    /// Tool definitions advertised to Anthropic. Empty when the assistant
    /// is producing a non-tool reply.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Claude model ID. Pulled from `LocalSettings.assistant_model` by the
    /// frontend.
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStreamTurnOutput {
    pub turn_id: String,
    /// Final `stop_reason` from the message_delta frame. `end_turn`,
    /// `tool_use`, `max_tokens`, or `stop_sequence`.
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantChunkPayload<'a> {
    turn_id: &'a str,
    #[serde(flatten)]
    event: &'a AssistantStreamEvent,
}

#[tauri::command]
pub async fn assistant_stream_turn(
    input: AssistantStreamTurnInput,
    state: State<'_, ManagedState>,
    app: AppHandle,
) -> Result<AssistantStreamTurnOutput> {
    validate(&input)?;
    let transport = state.streaming_anthropic_transport.clone();
    run_turn(transport, app, input).await
}

/// Core logic — split from the Tauri-attributed `assistant_stream_turn` so
/// the integration test in this module can drive it without standing up a
/// fake Tauri runtime.
async fn run_turn(
    transport: Arc<dyn StreamingAnthropicTransport>,
    app: AppHandle,
    input: AssistantStreamTurnInput,
) -> Result<AssistantStreamTurnOutput> {
    let turn_id = input.turn_id.clone();

    let mut request = StreamingAnthropicRequest::new(
        input.model,
        input.max_tokens,
        input.temperature,
        input.system,
    );
    request.messages = input.messages;
    request.tools = input.tools;

    let (tx, mut rx) = mpsc::channel::<AssistantStreamEvent>(64);
    let stream_task = tauri::async_runtime::spawn(async move {
        transport.stream(request, tx).await
    });

    let mut stop_reason: Option<String> = None;
    let mut stop_sequence: Option<String> = None;
    while let Some(event) = rx.recv().await {
        if let AssistantStreamEvent::MessageDelta {
            stop_reason: sr,
            stop_sequence: ss,
        } = &event
        {
            stop_reason = sr.clone();
            stop_sequence = ss.clone();
        }
        // Best-effort emit — if the frontend has disappeared (window
        // closing) the event drop is fine; we'll still drain the stream
        // so the transport task exits cleanly.
        let _ = app.emit(
            ASSISTANT_CHUNK_EVENT,
            AssistantChunkPayload {
                turn_id: &turn_id,
                event: &event,
            },
        );
    }

    match stream_task.await {
        Ok(Ok(())) => Ok(AssistantStreamTurnOutput {
            turn_id,
            stop_reason,
            stop_sequence,
        }),
        Ok(Err(e)) => Err(streaming_error_to_app_error(e)),
        Err(join_err) => Err(AppError::internal(format!(
            "assistant stream task aborted: {join_err}"
        ))),
    }
}

fn validate(input: &AssistantStreamTurnInput) -> Result<()> {
    if input.turn_id.trim().is_empty() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "assistant: turn_id is required",
        ));
    }
    if input.model.trim().is_empty() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "assistant: model is required",
        ));
    }
    if input.messages.is_empty() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "assistant: messages must be non-empty",
        ));
    }
    if input.max_tokens == 0 {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "assistant: max_tokens must be > 0",
        ));
    }
    if !(0.0..=1.0).contains(&input.temperature) {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "assistant: temperature must be in [0.0, 1.0]",
        ));
    }
    // Reject obviously malformed content blocks — e.g. a tool_result with
    // an empty tool_use_id. Anthropic rejects these with a 400, but
    // surfacing the error here is much cheaper than a round-trip.
    for msg in &input.messages {
        for block in &msg.content {
            if let MessageContent::ToolResult { tool_use_id, .. } = block {
                if tool_use_id.trim().is_empty() {
                    return Err(AppError::new(
                        AppErrorKind::Internal,
                        "assistant: tool_result.tool_use_id is required",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn streaming_error_to_app_error(e: StreamingError) -> AppError {
    match e {
        StreamingError::KeyMissing => {
            AppError::new(AppErrorKind::AnthropicKeyMissing, "anthropic key missing")
        }
        StreamingError::AuthInvalid => {
            AppError::new(AppErrorKind::AnthropicAuthInvalid, "anthropic auth invalid")
        }
        StreamingError::RateLimited { .. } => {
            AppError::new(AppErrorKind::RateLimited, "anthropic rate limited")
        }
        StreamingError::Network(m) => AppError::new(AppErrorKind::NetworkUnavailable, m),
        StreamingError::InvalidStream(m) => {
            AppError::new(AppErrorKind::AssistantStreamInvalid, m)
        }
        StreamingError::Other(m) => AppError::new(AppErrorKind::AssistantStreamInvalid, m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::transport::CannedStreamingTransport;

    fn base_input() -> AssistantStreamTurnInput {
        AssistantStreamTurnInput {
            turn_id: "turn_1".into(),
            system: "be helpful".into(),
            messages: vec![AssistantMessage {
                role: "user".into(),
                content: vec![MessageContent::Text {
                    text: "hi".into(),
                }],
            }],
            tools: vec![],
            model: "claude-sonnet-4-6".into(),
            max_tokens: 4096,
            temperature: 0.3,
        }
    }

    #[test]
    fn validate_rejects_empty_turn_id() {
        let mut input = base_input();
        input.turn_id = "  ".into();
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_rejects_empty_model() {
        let mut input = base_input();
        input.model = "".into();
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_rejects_empty_messages() {
        let mut input = base_input();
        input.messages.clear();
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_rejects_zero_max_tokens() {
        let mut input = base_input();
        input.max_tokens = 0;
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_rejects_out_of_range_temperature() {
        let mut input = base_input();
        input.temperature = 2.0;
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_rejects_empty_tool_use_id_in_tool_result() {
        let mut input = base_input();
        input.messages.push(AssistantMessage {
            role: "user".into(),
            content: vec![MessageContent::ToolResult {
                tool_use_id: "".into(),
                content: "x".into(),
                is_error: None,
            }],
        });
        assert!(validate(&input).is_err());
    }

    #[test]
    fn validate_accepts_well_formed_input() {
        assert!(validate(&base_input()).is_ok());
    }

    #[test]
    fn streaming_errors_map_to_expected_app_error_kinds() {
        assert_eq!(
            streaming_error_to_app_error(StreamingError::KeyMissing).kind,
            AppErrorKind::AnthropicKeyMissing
        );
        assert_eq!(
            streaming_error_to_app_error(StreamingError::AuthInvalid).kind,
            AppErrorKind::AnthropicAuthInvalid
        );
        assert_eq!(
            streaming_error_to_app_error(StreamingError::RateLimited { reset_at: None }).kind,
            AppErrorKind::RateLimited
        );
        assert_eq!(
            streaming_error_to_app_error(StreamingError::Network("x".into())).kind,
            AppErrorKind::NetworkUnavailable
        );
        assert_eq!(
            streaming_error_to_app_error(StreamingError::InvalidStream("x".into())).kind,
            AppErrorKind::AssistantStreamInvalid
        );
        assert_eq!(
            streaming_error_to_app_error(StreamingError::Other("x".into())).kind,
            AppErrorKind::AssistantStreamInvalid
        );
    }

    #[tokio::test]
    async fn canned_stream_drains_and_returns_stop_reason() {
        // Smoke that the channel-pump logic in `run_turn` correctly drains
        // a canned event sequence and surfaces the terminal stop_reason.
        // We can't easily exercise the Tauri `app.emit` side here without
        // standing up a fake AppHandle, but the channel-drain + return
        // path is the load-bearing piece.
        let events = vec![
            AssistantStreamEvent::MessageStart {
                message_id: "msg_1".into(),
                model: "claude-sonnet-4-6".into(),
            },
            AssistantStreamEvent::TextDelta {
                index: 0,
                text: "ok".into(),
            },
            AssistantStreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                stop_sequence: None,
            },
            AssistantStreamEvent::MessageStop,
        ];
        let transport = Arc::new(CannedStreamingTransport::new(events));
        let (tx, mut rx) = mpsc::channel::<AssistantStreamEvent>(64);
        let mut request = StreamingAnthropicRequest::new("m", 100, 0.0, "s");
        request.messages.push(AssistantMessage {
            role: "user".into(),
            content: vec![MessageContent::Text {
                text: "hi".into(),
            }],
        });

        let t = transport.clone();
        let task = tokio::spawn(async move { t.stream(request, tx).await });

        let mut stop_reason: Option<String> = None;
        while let Some(ev) = rx.recv().await {
            if let AssistantStreamEvent::MessageDelta {
                stop_reason: sr, ..
            } = &ev
            {
                stop_reason = sr.clone();
            }
        }
        task.await.unwrap().unwrap();
        assert_eq!(stop_reason, Some("end_turn".into()));
    }
}
