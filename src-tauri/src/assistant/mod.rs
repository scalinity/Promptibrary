//! In-app Claude Agent assistant primed by Fabric AI prompt-improvement
//! patterns.
//!
//! The assistant is wired as a streaming proxy: Rust holds the Anthropic API
//! key (per the project invariant that secrets never leave the backend) and
//! exposes a single IPC, `assistant_stream_turn`, that POSTs to
//! `/v1/messages?stream=true` and forwards SSE events to the calling window
//! via Tauri events. The agent loop itself — message history, tool
//! dispatch, end-turn detection — runs in the frontend so tool handlers
//! can mutate editor state directly without crossing the IPC boundary
//! once per call.
//!
//! L5 scope (parent SCA-920):
//!
//! - `patterns` — three bundled Fabric system prompts + the
//!   `AssistantPattern` enum.
//! - `streaming` — Anthropic SSE event parser (added in a follow-up
//!   commit).
//! - `transport` — streaming HTTP transport trait + impl (follow-up).
//!
//! Everything in this module is independent of the existing
//! `extraction::anthropic` client, which remains the one-shot
//! request/response path L4 needs.

pub mod patterns;
pub mod streaming;
pub mod transport;
