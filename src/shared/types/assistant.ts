// SCA-935 — TS counterparts of `src-tauri/src/assistant/transport.rs` +
// `src-tauri/src/assistant/streaming.rs` + `src-tauri/src/commands/assistant.rs`.
//
// Every type here mirrors a Rust struct/enum exactly. Wire formats use
// snake_case for tagged unions to match serde's `rename_all = "snake_case"`
// attribute on the Rust side.

import type { ClaudeModelId } from "@/shared/types/enums";
import type { AssistantPattern } from "@/shared/types/settings";

// ─── Conversation messages ──────────────────────────────────────────────────

/** One conversation message. `role` is `"user"` or `"assistant"`. */
export interface AssistantMessage {
  role: "user" | "assistant";
  content: MessageContent[];
}

/** A typed content block inside a message. Mirrors Rust's MessageContent. */
export type MessageContent =
  | { type: "text"; text: string }
  | {
      type: "tool_use";
      id: string;
      name: string;
      input: Record<string, unknown>;
    }
  | {
      type: "tool_result";
      tool_use_id: string;
      content: string;
      is_error?: boolean;
    };

// ─── Tool definitions ───────────────────────────────────────────────────────

export interface ToolDefinition {
  name: string;
  description: string;
  /**
   * JSON Schema describing the tool's input. Shape matches Anthropic's
   * tool-use API directly — the simplest case is
   * `{ type: "object", properties: {} }` for a no-arg tool.
   */
  input_schema: Record<string, unknown>;
}

// ─── IPC ────────────────────────────────────────────────────────────────────

export interface AssistantStreamTurnInput {
  turnId: string;
  system: string;
  messages: AssistantMessage[];
  tools: ToolDefinition[];
  model: ClaudeModelId;
  maxTokens: number;
  temperature: number;
}

export interface AssistantStreamTurnOutput {
  turnId: string;
  stopReason: string | null;
  stopSequence: string | null;
}

// ─── Streaming events ──────────────────────────────────────────────────────

export type ContentBlockHeader =
  | { type: "text" }
  | { type: "tool_use"; id: string; name: string };

/** Tagged on `type` to match the Rust serde representation. */
export type AssistantStreamEvent =
  | { type: "message_start"; message_id: string; model: string }
  | {
      type: "content_block_start";
      index: number;
      block: ContentBlockHeader;
    }
  | { type: "text_delta"; index: number; text: string }
  | { type: "input_json_delta"; index: number; partial_json: string }
  | { type: "content_block_stop"; index: number }
  | {
      type: "message_delta";
      stop_reason: string | null;
      stop_sequence: string | null;
    }
  | { type: "message_stop" }
  | { type: "ping" }
  | { type: "error"; error_type: string; message: string };

/** Event payload emitted on the `assistant:chunk` Tauri event. */
export type AssistantChunkPayload = AssistantStreamEvent & {
  turnId: string;
};

export const ASSISTANT_CHUNK_EVENT = "assistant:chunk";

// ─── Internal store types ──────────────────────────────────────────────────

/** Status of an in-progress turn. */
export type AssistantTurnStatus =
  | { kind: "idle" }
  | { kind: "streaming"; turnId: string; promptId: string; startedAt: number }
  | { kind: "tool_dispatch"; turnId: string; promptId: string }
  | { kind: "error"; message: string; turnId: string | null };

/** Re-export so downstream code can `import type { AssistantPattern }`
 * from the assistant types module without also pulling from settings. */
export type { AssistantPattern };
