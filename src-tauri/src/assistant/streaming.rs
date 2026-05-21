//! Anthropic `text/event-stream` parser and high-level event enum.
//!
//! Anthropic's streaming `/v1/messages` response is plain SSE: lines of
//! `event: <name>` and `data: <json>` separated by a blank line between
//! frames. Each frame's `data` payload is JSON describing one increment of
//! the response — message metadata, a new content block, a text or tool-
//! input delta, a stop signal, etc.
//!
//! `SseParser` is stateful and byte-incremental: it accepts whatever chunks
//! `reqwest` produces (boundaries are not guaranteed to line up with frames),
//! buffers across frame boundaries, and emits parsed `AssistantStreamEvent`s
//! as soon as complete frames are available.
//!
//! Reference event sequence (text + tool use):
//!
//! ```text
//! event: message_start
//! data: {"type":"message_start","message":{"id":"msg_…","model":"claude-…","content":[],"role":"assistant"}}
//!
//! event: content_block_start
//! data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}
//!
//! event: content_block_delta
//! data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}
//!
//! event: content_block_stop
//! data: {"type":"content_block_stop","index":0}
//!
//! event: content_block_start
//! data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_…","name":"x","input":{}}}
//!
//! event: content_block_delta
//! data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"a\":1}"}}
//!
//! event: content_block_stop
//! data: {"type":"content_block_stop","index":1}
//!
//! event: message_delta
//! data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null}}
//!
//! event: message_stop
//! data: {"type":"message_stop"}
//! ```
//!
//! `ping` frames carry no useful payload — we surface them so the IPC layer
//! can use them as a keep-alive heartbeat if it wants, but consumers can
//! safely ignore them.

use serde::{Deserialize, Serialize};

/// High-level event the parser surfaces to consumers. One frame ↔ one event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantStreamEvent {
    MessageStart {
        message_id: String,
        model: String,
    },
    ContentBlockStart {
        index: u32,
        block: ContentBlockHeader,
    },
    /// Plain-text delta — `delta.type == "text_delta"`.
    TextDelta {
        index: u32,
        text: String,
    },
    /// Tool-input JSON delta — `delta.type == "input_json_delta"`. Anthropic
    /// streams the tool's `input` object as a sequence of partial JSON
    /// strings; the consumer must concatenate them in order and parse the
    /// resulting whole.
    InputJsonDelta {
        index: u32,
        partial_json: String,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        stop_reason: Option<String>,
        stop_sequence: Option<String>,
    },
    MessageStop,
    Ping,
    Error {
        error_type: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockHeader {
    Text,
    ToolUse { id: String, name: String },
}

/// Hard error the parser can return — only for malformed inputs we cannot
/// recover from. Unknown event names are surfaced as `Ping`-equivalents
/// (silently dropped) rather than errors, matching Anthropic's "tolerate
/// new event types" guidance.
#[derive(Debug, Clone, PartialEq)]
pub enum SseParseError {
    InvalidUtf8,
    InvalidJson { event: String, raw: String, message: String },
}

/// Stateful, byte-incremental SSE parser.
///
/// `feed(chunk) -> Vec<event>` consumes whatever bytes `reqwest` produces,
/// buffers any incomplete trailing frame, and returns the events for every
/// complete frame seen so far.
///
/// SCA-948 — a single malformed frame (e.g. Anthropic edge case,
/// intermediary corruption) does NOT abort the whole stream. Bad-JSON
/// frames are skipped and counted; the parser only surfaces an error
/// when `consecutive_bad_frames` exceeds `BAD_FRAME_THRESHOLD`. A good
/// frame zeroes the consecutive counter.
#[derive(Default)]
pub struct SseParser {
    /// Accumulated bytes for the partial trailing frame.
    buffer: String,
    /// Bad frames seen in a row. Reset on every good frame. The parser
    /// aborts when this exceeds [`BAD_FRAME_THRESHOLD`].
    consecutive_bad_frames: u32,
    /// Lifetime bad-frame count, surfaced via `bad_frames_total()` for
    /// diagnostics and logging.
    bad_frames_total: u32,
}

/// Maximum consecutive malformed-JSON frames before the parser declares
/// the stream broken. 3 is enough to tolerate a transient blip without
/// hiding a genuine wire-format break.
const BAD_FRAME_THRESHOLD: u32 = 3;

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lifetime count of malformed frames silently skipped. Surfaced for
    /// telemetry / logging only.
    pub fn bad_frames_total(&self) -> u32 {
        self.bad_frames_total
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<AssistantStreamEvent>, SseParseError> {
        let text = std::str::from_utf8(chunk).map_err(|_| SseParseError::InvalidUtf8)?;
        self.buffer.push_str(text);

        let mut events = Vec::new();
        loop {
            let split = find_frame_boundary(&self.buffer);
            let Some((boundary, sep_len)) = split else { break };
            let frame = self.buffer[..boundary].to_string();
            self.buffer.drain(..boundary + sep_len);
            match parse_frame(&frame) {
                Ok(Some(ev)) => {
                    self.consecutive_bad_frames = 0;
                    events.push(ev);
                }
                Ok(None) => {
                    // Empty / pure-comment frame — neutral.
                }
                Err(SseParseError::InvalidJson { .. }) => {
                    // SCA-948 — non-fatal; count + skip. Surface only
                    // when the run of bad frames crosses the threshold.
                    self.bad_frames_total = self.bad_frames_total.saturating_add(1);
                    self.consecutive_bad_frames =
                        self.consecutive_bad_frames.saturating_add(1);
                    if self.consecutive_bad_frames > BAD_FRAME_THRESHOLD {
                        // Re-parse the offending frame to attach its
                        // context to the propagated error.
                        if let Err(e) = parse_frame(&frame) {
                            return Err(e);
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(events)
    }

    /// Drain any remaining buffered frame at end-of-stream. Anthropic
    /// terminates each frame with a blank line, so there should be nothing
    /// left in well-formed streams — but a missing trailing blank line is a
    /// real-world possibility we accommodate.
    pub fn finish(&mut self) -> Result<Vec<AssistantStreamEvent>, SseParseError> {
        let mut events = Vec::new();
        let trailing = std::mem::take(&mut self.buffer);
        let trimmed = trailing.trim_matches(|c| c == '\r' || c == '\n');
        if !trimmed.is_empty() {
            match parse_frame(trimmed) {
                Ok(Some(ev)) => {
                    self.consecutive_bad_frames = 0;
                    events.push(ev);
                }
                Ok(None) => {}
                Err(SseParseError::InvalidJson { .. }) => {
                    // SCA-948 — same tolerance policy at finish().
                    self.bad_frames_total = self.bad_frames_total.saturating_add(1);
                    self.consecutive_bad_frames =
                        self.consecutive_bad_frames.saturating_add(1);
                    if self.consecutive_bad_frames > BAD_FRAME_THRESHOLD {
                        if let Err(e) = parse_frame(trimmed) {
                            return Err(e);
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(events)
    }
}

/// Locate the next frame separator (`\n\n` or `\r\n\r\n`) and return
/// `(position, separator_len)`.
fn find_frame_boundary(buf: &str) -> Option<(usize, usize)> {
    let lf2 = buf.find("\n\n").map(|i| (i, 2));
    let crlf2 = buf.find("\r\n\r\n").map(|i| (i, 4));
    match (lf2, crlf2) {
        (Some(a), Some(b)) if a.0 <= b.0 => Some(a),
        (Some(a), None) => Some(a),
        (_, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Parse one complete SSE frame into an `AssistantStreamEvent`.
///
/// SSE frames are line-oriented: zero or more `event:`, `data:`, `id:`,
/// `retry:` lines. We only care about `event:` (event name) and `data:`
/// (JSON payload). Multiple `data:` lines in the same frame concatenate
/// with newlines per the SSE spec — Anthropic doesn't currently use this
/// but we honor it.
fn parse_frame(frame: &str) -> Result<Option<AssistantStreamEvent>, SseParseError> {
    let mut event_name: Option<&str> = None;
    let mut data = String::new();
    for line in frame.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim_start());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
        // `id:` / `retry:` are spec-defined but unused by Anthropic.
    }

    let Some(name) = event_name else { return Ok(None) };
    if data.is_empty() {
        return Ok(None);
    }

    decode_event(name, &data).map(Some)
}

fn decode_event(name: &str, data: &str) -> Result<AssistantStreamEvent, SseParseError> {
    let to_err = |e: serde_json::Error| SseParseError::InvalidJson {
        event: name.to_string(),
        raw: data.to_string(),
        message: e.to_string(),
    };

    match name {
        "message_start" => {
            let p: MessageStartPayload = serde_json::from_str(data).map_err(to_err)?;
            Ok(AssistantStreamEvent::MessageStart {
                message_id: p.message.id,
                model: p.message.model,
            })
        }
        "content_block_start" => {
            let p: ContentBlockStartPayload = serde_json::from_str(data).map_err(to_err)?;
            let block = match p.content_block {
                ContentBlockHeaderRaw::Text { .. } => ContentBlockHeader::Text,
                ContentBlockHeaderRaw::ToolUse { id, name } => {
                    ContentBlockHeader::ToolUse { id, name }
                }
                ContentBlockHeaderRaw::Other => ContentBlockHeader::Text,
            };
            Ok(AssistantStreamEvent::ContentBlockStart {
                index: p.index,
                block,
            })
        }
        "content_block_delta" => {
            let p: ContentBlockDeltaPayload = serde_json::from_str(data).map_err(to_err)?;
            Ok(match p.delta {
                BlockDelta::TextDelta { text } => AssistantStreamEvent::TextDelta {
                    index: p.index,
                    text,
                },
                BlockDelta::InputJsonDelta { partial_json } => {
                    AssistantStreamEvent::InputJsonDelta {
                        index: p.index,
                        partial_json,
                    }
                }
                BlockDelta::Other => AssistantStreamEvent::TextDelta {
                    index: p.index,
                    text: String::new(),
                },
            })
        }
        "content_block_stop" => {
            let p: IndexPayload = serde_json::from_str(data).map_err(to_err)?;
            Ok(AssistantStreamEvent::ContentBlockStop { index: p.index })
        }
        "message_delta" => {
            let p: MessageDeltaPayload = serde_json::from_str(data).map_err(to_err)?;
            Ok(AssistantStreamEvent::MessageDelta {
                stop_reason: p.delta.stop_reason,
                stop_sequence: p.delta.stop_sequence,
            })
        }
        "message_stop" => Ok(AssistantStreamEvent::MessageStop),
        "ping" => Ok(AssistantStreamEvent::Ping),
        "error" => {
            let p: ErrorPayload = serde_json::from_str(data).map_err(to_err)?;
            Ok(AssistantStreamEvent::Error {
                error_type: p.error.error_type,
                message: p.error.message,
            })
        }
        // Tolerate unknown event names — Anthropic adds them over time.
        _ => Ok(AssistantStreamEvent::Ping),
    }
}

// ─── JSON payload shapes (private; one-to-one with Anthropic's docs) ─────────

#[derive(Deserialize)]
struct MessageStartPayload {
    message: MessageStartMessage,
}

#[derive(Deserialize)]
struct MessageStartMessage {
    id: String,
    model: String,
}

#[derive(Deserialize)]
struct ContentBlockStartPayload {
    index: u32,
    content_block: ContentBlockHeaderRaw,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlockHeaderRaw {
    Text { #[serde(default)] _ignored: serde_json::Value },
    ToolUse { id: String, name: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct ContentBlockDeltaPayload {
    index: u32,
    delta: BlockDelta,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct IndexPayload {
    index: u32,
}

#[derive(Deserialize)]
struct MessageDeltaPayload {
    delta: MessageDeltaInner,
}

#[derive(Deserialize)]
struct MessageDeltaInner {
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    stop_sequence: Option<String>,
}

#[derive(Deserialize)]
struct ErrorPayload {
    error: ErrorInner,
}

#[derive(Deserialize)]
struct ErrorInner {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference text-only stream — message_start, one text block, message_stop.
    const TEXT_STREAM: &str = "\
event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-6\",\"role\":\"assistant\",\"content\":[]}}

event: content_block_start
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}

event: content_block_stop
data: {\"type\":\"content_block_stop\",\"index\":0}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null}}

event: message_stop
data: {\"type\":\"message_stop\"}

";

    /// Reference tool-use stream — text + a tool_use block with an input
    /// JSON delta, ending with stop_reason: tool_use.
    const TOOL_USE_STREAM: &str = "\
event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_2\",\"model\":\"claude-sonnet-4-6\",\"role\":\"assistant\",\"content\":[]}}

event: content_block_start
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"update_prompt_body\",\"input\":{}}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"body\\\":\"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"hi\\\"}\"}}

event: content_block_stop
data: {\"type\":\"content_block_stop\",\"index\":0}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null}}

event: message_stop
data: {\"type\":\"message_stop\"}

";

    #[test]
    fn parses_text_stream_in_one_chunk() {
        let mut p = SseParser::new();
        let events = p.feed(TEXT_STREAM.as_bytes()).unwrap();
        assert_eq!(
            events,
            vec![
                AssistantStreamEvent::MessageStart {
                    message_id: "msg_1".into(),
                    model: "claude-sonnet-4-6".into(),
                },
                AssistantStreamEvent::ContentBlockStart {
                    index: 0,
                    block: ContentBlockHeader::Text,
                },
                AssistantStreamEvent::TextDelta {
                    index: 0,
                    text: "Hello".into(),
                },
                AssistantStreamEvent::TextDelta {
                    index: 0,
                    text: " world".into(),
                },
                AssistantStreamEvent::ContentBlockStop { index: 0 },
                AssistantStreamEvent::MessageDelta {
                    stop_reason: Some("end_turn".into()),
                    stop_sequence: None,
                },
                AssistantStreamEvent::MessageStop,
            ]
        );
    }

    #[test]
    fn parses_tool_use_stream() {
        let mut p = SseParser::new();
        let events = p.feed(TOOL_USE_STREAM.as_bytes()).unwrap();
        assert!(matches!(
            events[0],
            AssistantStreamEvent::MessageStart { .. }
        ));
        assert_eq!(
            events[1],
            AssistantStreamEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockHeader::ToolUse {
                    id: "toolu_1".into(),
                    name: "update_prompt_body".into(),
                },
            }
        );
        // Two input_json_delta frames concatenate to a complete JSON object.
        let mut input_json = String::new();
        for ev in &events {
            if let AssistantStreamEvent::InputJsonDelta { partial_json, .. } = ev {
                input_json.push_str(partial_json);
            }
        }
        assert_eq!(input_json, r#"{"body":"hi"}"#);
        assert!(events.iter().any(|e| matches!(
            e,
            AssistantStreamEvent::MessageDelta { stop_reason: Some(s), .. } if s == "tool_use"
        )));
        assert_eq!(events.last(), Some(&AssistantStreamEvent::MessageStop));
    }

    #[test]
    fn handles_chunk_boundary_mid_frame() {
        // Split the stream at a byte that lands inside a `data:` value to
        // exercise the buffering. We choose 100 to land mid-payload.
        let bytes = TEXT_STREAM.as_bytes();
        let split = 100.min(bytes.len() - 1);
        let (a, b) = bytes.split_at(split);
        let mut p = SseParser::new();
        let mut got = p.feed(a).unwrap();
        got.extend(p.feed(b).unwrap());
        // Same as the single-chunk case.
        assert_eq!(got.len(), 7);
        assert!(matches!(got.last(), Some(AssistantStreamEvent::MessageStop)));
    }

    #[test]
    fn handles_chunk_boundary_in_separator() {
        // Place the cut between the two newlines that separate frames.
        // First frame is "event: message_start\ndata: {…}\n", then "\n".
        let stream = TEXT_STREAM.as_bytes();
        let first_frame_end = find_substring(stream, b"\n\n").unwrap();
        // Cut between the two newlines of the first separator.
        let cut = first_frame_end + 1;
        let (a, b) = stream.split_at(cut);
        let mut p = SseParser::new();
        let mut got = p.feed(a).unwrap();
        got.extend(p.feed(b).unwrap());
        assert!(matches!(got[0], AssistantStreamEvent::MessageStart { .. }));
        assert_eq!(got.len(), 7);
    }

    #[test]
    fn ping_and_unknown_events_are_pings() {
        let stream = "event: ping\ndata: {}\n\nevent: future_event\ndata: {\"x\":1}\n\n";
        let mut p = SseParser::new();
        let events = p.feed(stream.as_bytes()).unwrap();
        assert_eq!(
            events,
            vec![AssistantStreamEvent::Ping, AssistantStreamEvent::Ping]
        );
    }

    #[test]
    fn surfaces_error_event() {
        let stream = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"try again\"}}\n\n";
        let mut p = SseParser::new();
        let events = p.feed(stream.as_bytes()).unwrap();
        assert_eq!(
            events,
            vec![AssistantStreamEvent::Error {
                error_type: "overloaded_error".into(),
                message: "try again".into()
            }]
        );
    }

    #[test]
    fn invalid_json_reports_event_and_raw() {
        // SCA-948 — a single bad-JSON frame is tolerated (skipped and
        // counted). The parser no longer aborts the stream on the first
        // malformed frame; only when consecutive bad frames cross the
        // threshold does it propagate the error.
        let stream = "event: message_start\ndata: not json\n\n";
        let mut p = SseParser::new();
        let events = p.feed(stream.as_bytes()).unwrap();
        assert!(events.is_empty(), "no events emitted for bad frame");
        assert_eq!(
            p.bad_frames_total(),
            1,
            "bad frame counted for diagnostics"
        );
    }

    #[test]
    fn consecutive_bad_frames_above_threshold_abort_with_context() {
        // SCA-948 — threshold-many bad frames in a row DOES abort, and
        // the error carries the offending frame's `event:` name + raw
        // payload so the caller / log has something to debug from.
        let bad_frame = "event: message_start\ndata: not json\n\n";
        let mut p = SseParser::new();
        // BAD_FRAME_THRESHOLD = 3 — the 4th consecutive bad frame trips it.
        p.feed(bad_frame.as_bytes()).unwrap();
        p.feed(bad_frame.as_bytes()).unwrap();
        p.feed(bad_frame.as_bytes()).unwrap();
        let err = p.feed(bad_frame.as_bytes()).unwrap_err();
        match err {
            SseParseError::InvalidJson { event, raw, .. } => {
                assert_eq!(event, "message_start");
                assert_eq!(raw, "not json");
            }
            other => panic!("expected InvalidJson, got {other:?}"),
        }
        assert!(p.bad_frames_total() >= 4);
    }

    #[test]
    fn good_frame_resets_consecutive_bad_counter() {
        // SCA-948 — a transient blip (e.g. one corrupted frame followed
        // by recovery) must not eventually trip the threshold after a
        // long-running stream has accumulated occasional bad frames.
        let bad = "event: message_start\ndata: not json\n\n";
        let good =
            "event: ping\ndata: {}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let mut p = SseParser::new();
        for _ in 0..10 {
            p.feed(bad.as_bytes()).unwrap();
            let evs = p.feed(good.as_bytes()).unwrap();
            // ping + message_stop = 2 events from each good chunk.
            assert_eq!(evs.len(), 2);
        }
        assert_eq!(p.bad_frames_total(), 10);
    }

    #[test]
    fn finish_drains_trailing_frame_without_blank_line() {
        let stream =
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n"; // no trailing blank line
        let mut p = SseParser::new();
        assert!(p.feed(stream.as_bytes()).unwrap().is_empty());
        assert_eq!(p.finish().unwrap(), vec![AssistantStreamEvent::MessageStop]);
    }

    /// Byte-level substring search for the SSE separator search in tests.
    fn find_substring(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }
}
