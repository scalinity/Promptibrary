// SCA-950 — Agent-loop integration tests.
//
// Drives `runAgentLoop` directly (the for-loop / tool-dispatch core of
// `useAssistant`). We mock the assistantStreamTurn IPC by handing the
// loop a `dispatchTool` and `turnContextRef`; the IPC is replaced with
// a vi.fn that invokes the in-progress handler (turnContextRef.current.handle)
// with a canned event sequence, then resolves with the expected output.
//
// Coverage:
//   - text-only turn → conversation has the user + assistant text
//   - tool_use turn → tool dispatched, tool_result appended, loop continues
//   - maxIterations exhausted → status becomes error
//   - error event mid-stream → status becomes error, loop stops
//   - SCA-943 regression: messages payload sent to the IPC never contains
//     a content-empty assistant block

import { describe, expect, it, beforeEach, vi } from "vitest";

import {
  __testOnlyRunAgentLoop as runAgentLoop,
  type __TestOnlyRunArgs as RunArgs,
  type __TestOnlyTurnContextRef as TurnContextRef,
  type ToolDispatcher,
  type ToolDispatchResult,
} from "@/features/assistant/hooks/use-assistant";
import {
  conversationFor,
  useAssistantStore,
  type UiMessage,
} from "@/features/assistant/store/assistant-store";
import { asPromptId } from "@/shared/types/ids";
import type {
  AssistantMessage,
  AssistantStreamEvent,
  AssistantStreamTurnInput,
  AssistantStreamTurnOutput,
  ToolDefinition,
} from "@/shared/types/assistant";

// Mock the IPC module so we control assistantStreamTurn.
vi.mock("@/shared/api/ipc", () => ({
  assistantStreamTurn: vi.fn(),
}));
// Pulled after the mock declaration so the type stays accurate.
import { assistantStreamTurn } from "@/shared/api/ipc";
const mockStreamTurn = vi.mocked(assistantStreamTurn);

const PROMPT_ID = asPromptId("prompt_under_test");

/** Build a `RunArgs` whose store-touching callbacks delegate to the
 * real Zustand store. The test controls the IPC + dispatch. */
function buildArgs(overrides: Partial<RunArgs> = {}): RunArgs {
  const turnContextRef: TurnContextRef = { current: null };
  const store = useAssistantStore.getState;
  return {
    promptId: PROMPT_ID,
    systemPrompt: "be helpful",
    model: "claude-sonnet-4-6",
    tools: [],
    dispatchTool: async () => ({ kind: "unknown_tool" }),
    maxIterations: 3,
    maxTokens: 4096,
    temperature: 0.3,
    turnContextRef,
    getConversation: () =>
      conversationFor(store(), PROMPT_ID).map((m) => ({
        role: m.role,
        content: m.content,
      })) as AssistantMessage[],
    appendMessage: (m) => store().appendMessage(PROMPT_ID, m),
    updateMessage: (mid, patch) => store().updateMessage(PROMPT_ID, mid, patch),
    setStatus: (next) => store().setStatus(next),
    ...overrides,
  };
}

/** Configure mockStreamTurn to emit `events` to turnContextRef.current.handle
 * and resolve with `output`. Returns a promise that resolves once the
 * stub has been called with its input — useful for assertions on the
 * payload after the loop completes. */
function withCannedStream(
  turnContextRef: TurnContextRef,
  events: AssistantStreamEvent[],
  output: Omit<AssistantStreamTurnOutput, "turnId">,
): void {
  mockStreamTurn.mockImplementationOnce(async (input: AssistantStreamTurnInput) => {
    const ctx = turnContextRef.current;
    if (ctx == null) throw new Error("test setup error: no active turnContext");
    for (const ev of events) {
      // Mirror the wire envelope: AssistantChunkPayload = event + turnId.
      ctx.handle({ ...ev, turnId: ctx.turnId } as Parameters<typeof ctx.handle>[0]);
    }
    return {
      turnId: input.turnId,
      stopReason: output.stopReason,
      stopSequence: output.stopSequence,
    };
  });
}

function resetStore(): void {
  useAssistantStore.setState({
    isOpen: false,
    conversationsByPromptId: {},
    selectedPattern: null,
    status: { kind: "idle" },
    draft: "",
  });
}

function userTurnInStore(text: string): void {
  const msg: UiMessage = {
    id: "msg_user",
    role: "user",
    content: [{ type: "text", text }],
    isStreaming: false,
  };
  useAssistantStore.getState().appendMessage(PROMPT_ID, msg);
}

describe("assistant agent loop", () => {
  beforeEach(() => {
    mockStreamTurn.mockReset();
    resetStore();
  });

  it("text-only turn — appends streamed assistant message and idles", async () => {
    userTurnInStore("hi");
    const args = buildArgs();
    withCannedStream(
      args.turnContextRef,
      [
        { type: "message_start", message_id: "m1", model: "claude-sonnet-4-6" },
        { type: "content_block_start", index: 0, block: { type: "text" } },
        { type: "text_delta", index: 0, text: "Hello" },
        { type: "text_delta", index: 0, text: " world" },
        { type: "content_block_stop", index: 0 },
        { type: "message_delta", stop_reason: "end_turn", stop_sequence: null },
        { type: "message_stop" },
      ],
      { stopReason: "end_turn", stopSequence: null },
    );

    await runAgentLoop(args);

    const convo = conversationFor(useAssistantStore.getState(), PROMPT_ID);
    expect(convo).toHaveLength(2);
    expect(convo[1].role).toBe("assistant");
    expect(convo[1].isStreaming).toBe(false);
    expect(convo[1].content).toEqual([{ type: "text", text: "Hello world" }]);
    expect(useAssistantStore.getState().status.kind).toBe("idle");
    expect(mockStreamTurn).toHaveBeenCalledTimes(1);
  });

  it("SCA-943 regression — messages payload never contains an empty assistant content block", async () => {
    userTurnInStore("hi");
    const args = buildArgs();
    withCannedStream(
      args.turnContextRef,
      [
        { type: "text_delta", index: 0, text: "ok" },
        { type: "message_delta", stop_reason: "end_turn", stop_sequence: null },
      ],
      { stopReason: "end_turn", stopSequence: null },
    );

    await runAgentLoop(args);

    expect(mockStreamTurn).toHaveBeenCalledTimes(1);
    const wirePayload = mockStreamTurn.mock.calls[0][0];
    for (const msg of wirePayload.messages) {
      if (msg.role === "assistant") {
        expect(msg.content.length).toBeGreaterThan(0);
      }
    }
  });

  it("tool_use turn — dispatcher invoked, tool_result appended, loop continues to end_turn", async () => {
    userTurnInStore("update body to X");
    const tools: ToolDefinition[] = [
      {
        name: "update_prompt_body",
        description: "test",
        input_schema: { type: "object", properties: {} },
      },
    ];
    const dispatcher = vi.fn<ToolDispatcher>(async () => ({
      kind: "ok",
      content: "Body updated.",
    } as ToolDispatchResult));
    const args = buildArgs({ tools, dispatchTool: dispatcher });

    // Turn 1: emits a tool_use block, ends with stop_reason: tool_use.
    withCannedStream(
      args.turnContextRef,
      [
        {
          type: "content_block_start",
          index: 0,
          block: { type: "tool_use", id: "toolu_1", name: "update_prompt_body" },
        },
        { type: "input_json_delta", index: 0, partial_json: '{"body":' },
        { type: "input_json_delta", index: 0, partial_json: '"X"}' },
        { type: "content_block_stop", index: 0 },
        { type: "message_delta", stop_reason: "tool_use", stop_sequence: null },
      ],
      { stopReason: "tool_use", stopSequence: null },
    );
    // Turn 2: assistant says "done", end_turn.
    withCannedStream(
      args.turnContextRef,
      [
        { type: "text_delta", index: 0, text: "Done." },
        { type: "message_delta", stop_reason: "end_turn", stop_sequence: null },
      ],
      { stopReason: "end_turn", stopSequence: null },
    );

    await runAgentLoop(args);

    expect(dispatcher).toHaveBeenCalledTimes(1);
    expect(dispatcher).toHaveBeenCalledWith("update_prompt_body", { body: "X" });
    const convo = conversationFor(useAssistantStore.getState(), PROMPT_ID);
    // user msg + assistant(tool_use) + user(tool_result) + assistant("Done.")
    expect(convo).toHaveLength(4);
    expect(convo[2].role).toBe("user");
    expect(convo[2].content[0]).toMatchObject({
      type: "tool_result",
      tool_use_id: "toolu_1",
      content: "Body updated.",
    });
    expect(useAssistantStore.getState().status.kind).toBe("idle");
    expect(mockStreamTurn).toHaveBeenCalledTimes(2);
  });

  it("maxIterations exhausted — status becomes error", async () => {
    userTurnInStore("loop forever");
    const tools: ToolDefinition[] = [
      {
        name: "noop",
        description: "test",
        input_schema: { type: "object", properties: {} },
      },
    ];
    const dispatcher: ToolDispatcher = async () => ({ kind: "ok", content: "ok" });
    const args = buildArgs({ tools, dispatchTool: dispatcher, maxIterations: 2 });

    // Every turn says stop_reason: tool_use with a tool_use block —
    // the loop would run forever without the bound.
    const toolUseStream: AssistantStreamEvent[] = [
      {
        type: "content_block_start",
        index: 0,
        block: { type: "tool_use", id: "toolu_x", name: "noop" },
      },
      { type: "input_json_delta", index: 0, partial_json: "{}" },
      { type: "content_block_stop", index: 0 },
      { type: "message_delta", stop_reason: "tool_use", stop_sequence: null },
    ];
    withCannedStream(args.turnContextRef, toolUseStream, {
      stopReason: "tool_use",
      stopSequence: null,
    });
    withCannedStream(args.turnContextRef, toolUseStream, {
      stopReason: "tool_use",
      stopSequence: null,
    });

    await runAgentLoop(args);

    expect(mockStreamTurn).toHaveBeenCalledTimes(2);
    const status = useAssistantStore.getState().status;
    expect(status.kind).toBe("error");
    if (status.kind === "error") {
      expect(status.message).toContain("max iterations");
    }
  });

  it("error event mid-stream — status becomes error, loop stops", async () => {
    userTurnInStore("trigger overload");
    const args = buildArgs();
    withCannedStream(
      args.turnContextRef,
      [
        { type: "text_delta", index: 0, text: "I'll try " },
        {
          type: "error",
          error_type: "overloaded_error",
          message: "Anthropic API is overloaded",
        },
        { type: "message_delta", stop_reason: null, stop_sequence: null },
        { type: "message_stop" },
      ],
      { stopReason: null, stopSequence: null },
    );

    await runAgentLoop(args);

    expect(mockStreamTurn).toHaveBeenCalledTimes(1);
    const status = useAssistantStore.getState().status;
    expect(status.kind).toBe("error");
    if (status.kind === "error") {
      expect(status.message).toContain("overloaded_error");
      expect(status.message).toContain("overloaded");
    }
  });
});
