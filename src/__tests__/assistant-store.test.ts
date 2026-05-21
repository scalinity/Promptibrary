// SCA-939 — Assistant store: per-prompt conversation keying.
//
// Locks in the invariants that the agent loop hook depends on:
//   - Two prompts have independent conversations.
//   - Switching prompts does not lose history.
//   - resetConversation only clears the target id (and the global draft +
//     status, since both are scoped to the active session).

import { describe, expect, it, beforeEach } from "vitest";

import {
  conversationFor,
  toWireMessage,
  useAssistantStore,
  type UiMessage,
} from "@/features/assistant/store/assistant-store";
import { asPromptId } from "@/shared/types/ids";

function userMessage(id: string, text: string): UiMessage {
  return {
    id,
    role: "user",
    content: [{ type: "text", text }],
    isStreaming: false,
  };
}

function assistantMessage(id: string, text: string): UiMessage {
  return {
    id,
    role: "assistant",
    content: [{ type: "text", text }],
    isStreaming: false,
  };
}

describe("assistant store — per-prompt conversation keying", () => {
  beforeEach(() => {
    useAssistantStore.setState({
      isOpen: false,
      conversationsByPromptId: {},
      selectedPattern: null,
      status: { kind: "idle" },
      draft: "",
    });
  });

  it("starts with an empty conversation for any prompt id", () => {
    const state = useAssistantStore.getState();
    expect(conversationFor(state, asPromptId("prompt_a"))).toEqual([]);
  });

  it("appends and reads back messages for one prompt id", () => {
    const a = asPromptId("prompt_a");
    const { appendMessage } = useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi"));
    appendMessage(a, assistantMessage("m2", "hello"));
    const convo = conversationFor(useAssistantStore.getState(), a);
    expect(convo).toHaveLength(2);
    expect(convo[0].role).toBe("user");
    expect(convo[1].role).toBe("assistant");
  });

  it("keeps prompt-a and prompt-b conversations independent", () => {
    const a = asPromptId("prompt_a");
    const b = asPromptId("prompt_b");
    const { appendMessage } = useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi a"));
    appendMessage(b, userMessage("m2", "hi b"));
    appendMessage(a, assistantMessage("m3", "reply a"));
    const state = useAssistantStore.getState();
    const convoA = conversationFor(state, a);
    const convoB = conversationFor(state, b);
    expect(convoA.map((m) => m.id)).toEqual(["m1", "m3"]);
    expect(convoB.map((m) => m.id)).toEqual(["m2"]);
  });

  it("preserves history when the user navigates away and back", () => {
    const a = asPromptId("prompt_a");
    const b = asPromptId("prompt_b");
    const { appendMessage } = useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi a"));
    // Simulate switching to b — only the read selector changes; the store
    // doesn't mutate just because we look at a different prompt.
    const _other = conversationFor(useAssistantStore.getState(), b);
    expect(_other).toEqual([]);
    // Back to a:
    const convoA = conversationFor(useAssistantStore.getState(), a);
    expect(convoA).toHaveLength(1);
  });

  it("updateMessage patches the targeted message in place", () => {
    const a = asPromptId("prompt_a");
    const { appendMessage, updateMessage } = useAssistantStore.getState();
    appendMessage(a, assistantMessage("m1", "streaming…"));
    updateMessage(a, "m1", (m) => ({
      ...m,
      content: [{ type: "text", text: "done" }],
      isStreaming: false,
    }));
    const convo = conversationFor(useAssistantStore.getState(), a);
    expect(convo[0].content).toEqual([{ type: "text", text: "done" }]);
    expect(convo[0].isStreaming).toBe(false);
  });

  it("resetConversation only clears the target prompt", () => {
    const a = asPromptId("prompt_a");
    const b = asPromptId("prompt_b");
    const { appendMessage, resetConversation, setDraft } =
      useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi a"));
    appendMessage(b, userMessage("m2", "hi b"));
    setDraft("a stray draft");
    resetConversation(a);
    const state = useAssistantStore.getState();
    expect(conversationFor(state, a)).toEqual([]);
    expect(conversationFor(state, b).map((m) => m.id)).toEqual(["m2"]);
    // Draft + status are scoped to the active session, so reset clears
    // them too — the contract from assistant-store.ts.
    expect(state.draft).toBe("");
    expect(state.status.kind).toBe("idle");
  });

  it("resetConversation does NOT clobber an in-flight status that belongs to another prompt (SCA-947)", () => {
    const a = asPromptId("prompt_a");
    const b = asPromptId("prompt_b");
    const { appendMessage, resetConversation, setStatus } =
      useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi a"));
    appendMessage(b, userMessage("m2", "hi b"));
    // A turn is mid-stream on prompt b.
    setStatus({
      kind: "streaming",
      turnId: "turn_b",
      promptId: b,
      startedAt: Date.now(),
    });
    // User clears prompt a's conversation.
    resetConversation(a);
    const state = useAssistantStore.getState();
    expect(conversationFor(state, a)).toEqual([]);
    // The in-flight status for b is preserved — the streaming turn
    // is NOT silently flipped to idle.
    expect(state.status.kind).toBe("streaming");
    if (state.status.kind === "streaming") {
      expect(state.status.turnId).toBe("turn_b");
      expect(state.status.promptId).toBe(b);
    }
  });

  it("resetConversation clears in-flight status when it belongs to the reset target (SCA-947)", () => {
    const a = asPromptId("prompt_a");
    const { appendMessage, resetConversation, setStatus } =
      useAssistantStore.getState();
    appendMessage(a, userMessage("m1", "hi a"));
    setStatus({
      kind: "streaming",
      turnId: "turn_a",
      promptId: a,
      startedAt: Date.now(),
    });
    resetConversation(a);
    const state = useAssistantStore.getState();
    expect(state.status.kind).toBe("idle");
  });

  it("toWireMessage strips ui-only fields and preserves wire content", () => {
    const wire = toWireMessage({
      id: "m1",
      role: "user",
      content: [{ type: "text", text: "hi" }],
      isStreaming: false,
    });
    expect(wire).toEqual({
      role: "user",
      content: [{ type: "text", text: "hi" }],
    });
  });

  it("toggleOpen flips the drawer visibility", () => {
    const initial = useAssistantStore.getState().isOpen;
    useAssistantStore.getState().toggleOpen();
    expect(useAssistantStore.getState().isOpen).toBe(!initial);
    useAssistantStore.getState().toggleOpen();
    expect(useAssistantStore.getState().isOpen).toBe(initial);
  });
});
