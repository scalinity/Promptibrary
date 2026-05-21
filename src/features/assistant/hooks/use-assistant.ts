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
   * tool_use cycles. Default 8. */
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

export function useAssistant(opts: UseAssistantOptions): UseAssistantApi {
  const {
    promptId,
    systemPrompt,
    model,
    tools,
    dispatchTool,
    maxIterations = 8,
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

      try {
        await runAgentLoop({
          promptId,
          systemPrompt,
          model,
          tools,
          dispatchTool,
          maxIterations,
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
    getConversation,
    appendMessage,
    updateMessage,
    setStatus,
  } = args;

  for (let iteration = 0; iteration < maxIterations; iteration += 1) {
    const turnId = generateId("turn");
    const assistantMessageId = generateId("msg");

    // Seed an empty streaming assistant message so the UI can render
    // tokens as they arrive.
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

    const unlisten: UnlistenFn = await listen<AssistantChunkPayload>(
      ASSISTANT_CHUNK_EVENT,
      (event) => {
        const p = event.payload;
        if (p.turnId !== turnId) return;
        switch (p.type) {
          case "content_block_start": {
            if (p.block.type === "tool_use") {
              pendingTools.set(p.index, {
                id: p.block.id,
                name: p.block.name,
                inputJson: "",
              });
            }
            break;
          }
          case "text_delta": {
            textSoFar += p.text;
            updateMessage(assistantMessageId, (m) => ({
              ...m,
              content: textSoFar.length
                ? [{ type: "text", text: textSoFar }]
                : m.content,
            }));
            break;
          }
          case "input_json_delta": {
            const t = pendingTools.get(p.index);
            if (t) t.inputJson += p.partial_json;
            break;
          }
          default:
            // message_start, content_block_stop, message_delta,
            // message_stop, ping, error — all observable in the IPC
            // return value or covered by status updates already.
            break;
        }
      },
    );

    let result;
    try {
      result = await assistantStreamTurn({
        turnId,
        system: systemPrompt,
        messages: getConversation(),
        tools,
        model,
        maxTokens: 4096,
        temperature: 0.3,
      });
    } finally {
      unlisten();
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
