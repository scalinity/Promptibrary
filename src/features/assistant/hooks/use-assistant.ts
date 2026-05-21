// SCA-935 — Agent loop hook.
//
// Encapsulates one turn → emit → drain → maybe-tool-dispatch → loop cycle
// per the architecture in /Users/danny/.claude/plans/check-out-users-danny-
// documents-codez-ap-optimized-torvalds.md:
//
//   1. Build messages + tools + system prompt.
//   2. Generate a turnId, call `assistantStreamTurn` IPC.
//   3. Listen for `assistant:chunk` events filtered by turnId. Accumulate
//      text deltas into the streaming assistant message; accumulate
//      input_json_delta frames into the partial JSON of each pending
//      tool_use block.
//   4. When the IPC resolves, inspect `stopReason`:
//        - `end_turn` / `stop_sequence` / `max_tokens` → terminal.
//        - `tool_use` → dispatch each accumulated tool_use to the caller-
//          supplied handler, append a tool_result message, and recurse.
//   5. Errors surface via the store's `status.kind === "error"` slot.
//
// SCA-946 — the `assistant:chunk` Tauri listener is registered ONCE per
// `send()` call (covering every iteration of the agent loop) rather than
// once per turn inside the loop. A `TurnContext` held in a closed-over
// ref tells the listener handler where to route the current event. This
// avoids per-turn listen/unlisten churn and prepares the ground for the
// next iteration to share infrastructure.

import { useCallback, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { assistantStreamTurn } from "@/shared/api/ipc";
import type { PromptId } from "@/shared/types/ids";
import {
  ASSISTANT_CHUNK_EVENT,
  type AssistantChunkPayload,
  type AssistantMessage,
  type AssistantPattern,
  type MessageContent,
  type ToolDefinition,
} from "@/shared/types/assistant";
import {
  conversationFor,
  toWireMessage,
  useAssistantStore,
  type UiMessage,
} from "@/features/assistant/store/assistant-store";
import type { ClaudeModelId } from "@/shared/types/enums";

/** What a tool handler returns. Stringified content goes back as the
 * tool_result content body; `isError: true` flags the call as failed. */
export interface ToolDispatchResult {
  content: string;
  isError?: boolean;
}

/** Caller-supplied tool dispatcher. Returns `null` for an unknown tool
 * name so the hook can synthesize a generic "no such tool" tool_result
 * rather than throwing. */
export type ToolDispatcher = (
  name: string,
  input: Record<string, unknown>,
) => Promise<ToolDispatchResult | null>;

export interface UseAssistantOptions {
  promptId: PromptId | null;
  systemPrompt: string;
  pattern: AssistantPattern;
  model: ClaudeModelId;
  tools: ToolDefinition[];
  dispatchTool: ToolDispatcher;
  /** Cap on agent-loop iterations — defensive bound against runaway
   * tool_use cycles. Default 3 (SCA-945) bounds the blast radius of a
   * coerced tool-use sequence; raise per-call if a workflow genuinely
   * needs more hops. */
  maxIterations?: number;
}

/** Result the hook surfaces to the component. */
export interface UseAssistantApi {
  send: (text: string) => Promise<void>;
}

/** Generate a short opaque ID. Crypto-random when available, time-based
 * fallback otherwise — only needs to be unique within the running app. */
function generateId(prefix: string): string {
  const rand =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}_${rand}`;
}

interface PendingToolUse {
  id: string;
  name: string;
  inputJson: string;
}

/** Per-turn handler bundle. The send-scoped listener reads `current` and
 * routes events into the active turn's accumulators. */
interface TurnContext {
  turnId: string;
  handle: (payload: AssistantChunkPayload) => void;
}

type TurnContextRef = { current: TurnContext | null };

export function useAssistant(opts: UseAssistantOptions): UseAssistantApi {
  const {
    promptId,
    systemPrompt,
    model,
    tools,
    dispatchTool,
    maxIterations = 3,
  } = opts;
  const appendMessage = useAssistantStore((s) => s.appendMessage);
  const updateMessage = useAssistantStore((s) => s.updateMessage);
  const setStatus = useAssistantStore((s) => s.setStatus);
  // Use a ref to read the latest conversation without subscribing the hook
  // to every keystroke — the agent loop reads on demand.
  const storeRef = useRef(useAssistantStore.getState);
  storeRef.current = useAssistantStore.getState;

  const send = useCallback(
    async (text: string): Promise<void> => {
      if (promptId == null) return;
      const trimmed = text.trim();
      if (trimmed.length === 0) return;

      // 1. Stage the user message immediately so it appears in the UI
      //    before any network activity.
      const userMessage: UiMessage = {
        id: generateId("msg"),
        role: "user",
        content: [{ type: "text", text: trimmed }],
        isStreaming: false,
      };
      appendMessage(promptId, userMessage);

      // SCA-946 — one listener for the whole send() call. The agent loop
      // updates `turnContextRef.current` per iteration; the listener
      // routes events into whichever turn is active.
      const turnContextRef: TurnContextRef = { current: null };
      const unlisten: UnlistenFn = await listen<AssistantChunkPayload>(
        ASSISTANT_CHUNK_EVENT,
        (event) => {
          const ctx = turnContextRef.current;
          if (ctx == null) return;
          const payload = event.payload;
          if (payload.turnId !== ctx.turnId) return;
          ctx.handle(payload);
        },
      );

      try {
        await runAgentLoop({
          promptId,
          systemPrompt,
          model,
          tools,
          dispatchTool,
          maxIterations,
          turnContextRef,
          getConversation: () =>
            conversationFor(storeRef.current(), promptId).map(toWireMessage),
          appendMessage: (m) => appendMessage(promptId, m),
          updateMessage: (mid, patch) => updateMessage(promptId, mid, patch),
          setStatus,
        });
      } catch (err) {
        setStatus({
          kind: "error",
          message: err instanceof Error ? err.message : String(err),
          turnId: null,
        });
      } finally {
        unlisten();
      }
    },
    [
      promptId,
      systemPrompt,
      model,
      tools,
      dispatchTool,
      maxIterations,
      appendMessage,
      updateMessage,
      setStatus,
    ],
  );

  return { send };
}

interface RunArgs {
  promptId: PromptId;
  systemPrompt: string;
  model: ClaudeModelId;
  tools: ToolDefinition[];
  dispatchTool: ToolDispatcher;
  maxIterations: number;
  turnContextRef: TurnContextRef;
  getConversation: () => AssistantMessage[];
  appendMessage: (m: UiMessage) => void;
  updateMessage: (mid: string, patch: (m: UiMessage) => UiMessage) => void;
  setStatus: (next: ReturnType<typeof useAssistantStore.getState>["status"]) => void;
}

async function runAgentLoop(args: RunArgs): Promise<void> {
  const {
    systemPrompt,
    model,
    tools,
    dispatchTool,
    maxIterations,
    turnContextRef,
    getConversation,
    appendMessage,
    updateMessage,
    setStatus,
  } = args;

  for (let iteration = 0; iteration < maxIterations; iteration += 1) {
    const turnId = generateId("turn");
    const assistantMessageId = generateId("msg");

    // SCA-943 — snapshot the wire conversation BEFORE we seed the
    // streaming placeholder below. If we read the store AFTER the
    // append, the request body ends with `{role:"assistant", content:[]}`
    // and Anthropic rejects it with HTTP 400.
    const conversationSnapshot = getConversation();

    // Seed an empty streaming assistant message so the UI can render
    // tokens as they arrive. NOTE: this UI message is never sent over
    // the wire — conversationSnapshot above was taken before it landed.
    appendMessage({
      id: assistantMessageId,
      role: "assistant",
      content: [],
      isStreaming: true,
    });
    setStatus({ kind: "streaming", turnId, startedAt: Date.now() });

    // Per-turn accumulators.
    let textSoFar = "";
    const pendingTools = new Map<number, PendingToolUse>();

    // SCA-946 — register this turn's handler with the send-scoped
    // listener. The listener (in send() above) reads turnContextRef
    // on each event and dispatches here.
    turnContextRef.current = {
      turnId,
      handle: (payload) => {
        switch (payload.type) {
          case "content_block_start": {
            if (payload.block.type === "tool_use") {
              pendingTools.set(payload.index, {
                id: payload.block.id,
                name: payload.block.name,
                inputJson: "",
              });
            }
            break;
          }
          case "text_delta": {
            textSoFar += payload.text;
            updateMessage(assistantMessageId, (m) => ({
              ...m,
              content: textSoFar.length
                ? [{ type: "text", text: textSoFar }]
                : m.content,
            }));
            break;
          }
          case "input_json_delta": {
            const t = pendingTools.get(payload.index);
            if (t) t.inputJson += payload.partial_json;
            break;
          }
          default:
            // message_start, content_block_stop, message_delta,
            // message_stop, ping, error — all observable in the IPC
            // return value or covered by status updates already.
            break;
        }
      },
    };

    let result;
    try {
      result = await assistantStreamTurn({
        turnId,
        system: systemPrompt,
        messages: conversationSnapshot,
        tools,
        model,
        maxTokens: 4096,
        temperature: 0.3,
      });
    } finally {
      // SCA-946 — clear the active turn so any late-delivered events
      // (e.g. a stragger message_stop) for this turnId aren't routed
      // into the next iteration's accumulators.
      turnContextRef.current = null;
    }

    // Finalize the streamed assistant content: text block (if any) plus
    // each tool_use block (in index order) with its parsed input.
    const finalContent: MessageContent[] = [];
    if (textSoFar.length > 0) {
      finalContent.push({ type: "text", text: textSoFar });
    }
    const orderedToolIndexes = [...pendingTools.keys()].sort((a, b) => a - b);
    const toolUses = orderedToolIndexes.map((i) => {
      const t = pendingTools.get(i)!;
      let parsed: Record<string, unknown> = {};
      if (t.inputJson.trim().length > 0) {
        try {
          parsed = JSON.parse(t.inputJson) as Record<string, unknown>;
        } catch {
          parsed = { __raw: t.inputJson };
        }
      }
      const block: MessageContent = {
        type: "tool_use",
        id: t.id,
        name: t.name,
        input: parsed,
      };
      finalContent.push(block);
      return { id: t.id, name: t.name, input: parsed };
    });
    updateMessage(assistantMessageId, (m) => ({
      ...m,
      content: finalContent,
      isStreaming: false,
    }));

    if (result.stopReason !== "tool_use" || toolUses.length === 0) {
      setStatus({ kind: "idle" });
      return;
    }

    // Dispatch each tool, accumulate tool_result content blocks.
    setStatus({ kind: "tool_dispatch", turnId });
    const toolResults: MessageContent[] = [];
    for (const t of toolUses) {
      let r: ToolDispatchResult | null;
      try {
        r = await dispatchTool(t.name, t.input);
      } catch (err) {
        r = {
          content: err instanceof Error ? err.message : String(err),
          isError: true,
        };
      }
      if (r == null) {
        toolResults.push({
          type: "tool_result",
          tool_use_id: t.id,
          content: `unknown tool: ${t.name}`,
          is_error: true,
        });
      } else {
        toolResults.push({
          type: "tool_result",
          tool_use_id: t.id,
          content: r.content,
          is_error: r.isError ?? undefined,
        });
      }
    }

    // The tool_result blocks must be wrapped in a "user" message per
    // Anthropic's tool-use protocol. We push it both to the UI store
    // (so the rendering layer can show a "✓ ran update_prompt_body"
    // affordance) and to the next turn's conversation history.
    appendMessage({
      id: generateId("msg"),
      role: "user",
      content: toolResults,
      isStreaming: false,
    });
    // Loop continues with the updated conversation history.
  }

  // Bound exceeded — surface as a terminal error so the user can see why
  // the assistant stopped.
  setStatus({
    kind: "error",
    message: `assistant: max iterations (${maxIterations}) reached without end_turn`,
    turnId: null,
  });
}
