// SCA-935 — Assistant store.
//
// Holds a per-prompt conversation map plus the in-flight turn status,
// current pattern selection, and current draft input text. State is
// in-memory only for V1 — persisting conversations across app restarts
// is in docs/V2-CANDIDATES.md.

import { create } from "zustand";

import type { PromptId } from "@/shared/types/ids";
import type {
  AssistantMessage,
  AssistantPattern,
  AssistantTurnStatus,
  MessageContent,
} from "@/shared/types/assistant";

/** A single completed user-or-assistant turn the UI renders. */
export interface UiMessage {
  /** Stable id for React keys + selective re-renders. */
  id: string;
  role: "user" | "assistant";
  /** Mirrors the wire content blocks — what gets sent back as history. */
  content: MessageContent[];
  /** True while the assistant message is still streaming. */
  isStreaming: boolean;
}

interface AssistantState {
  /** Whether the drawer is visible. ⌘I toggles. */
  isOpen: boolean;
  /** Conversation history keyed by the prompt the user is editing. */
  conversationsByPromptId: Record<string, UiMessage[]>;
  /** Pattern the user has picked for the current session. Falls back to
   * the global `assistantDefaultPattern` setting at hook level. */
  selectedPattern: AssistantPattern | null;
  /** Status of any in-flight turn. */
  status: AssistantTurnStatus;
  /** Composer input. */
  draft: string;
  // ─── mutators ────────────────────────────────────────────────────────
  setOpen: (next: boolean) => void;
  toggleOpen: () => void;
  setDraft: (next: string) => void;
  setSelectedPattern: (pattern: AssistantPattern | null) => void;
  setStatus: (next: AssistantTurnStatus) => void;
  appendMessage: (promptId: PromptId, message: UiMessage) => void;
  updateMessage: (
    promptId: PromptId,
    messageId: string,
    patch: (m: UiMessage) => UiMessage,
  ) => void;
  resetConversation: (promptId: PromptId) => void;
}

export const useAssistantStore = create<AssistantState>((set) => ({
  isOpen: false,
  conversationsByPromptId: {},
  selectedPattern: null,
  status: { kind: "idle" },
  draft: "",
  setOpen: (next) => set({ isOpen: next }),
  toggleOpen: () => set((s) => ({ isOpen: !s.isOpen })),
  setDraft: (next) => set({ draft: next }),
  setSelectedPattern: (pattern) => set({ selectedPattern: pattern }),
  setStatus: (next) => set({ status: next }),
  appendMessage: (promptId, message) =>
    set((s) => {
      const existing = s.conversationsByPromptId[promptId] ?? [];
      return {
        conversationsByPromptId: {
          ...s.conversationsByPromptId,
          [promptId]: [...existing, message],
        },
      };
    }),
  updateMessage: (promptId, messageId, patch) =>
    set((s) => {
      const existing = s.conversationsByPromptId[promptId] ?? [];
      return {
        conversationsByPromptId: {
          ...s.conversationsByPromptId,
          [promptId]: existing.map((m) => (m.id === messageId ? patch(m) : m)),
        },
      };
    }),
  resetConversation: (promptId) =>
    set((s) => {
      const next = { ...s.conversationsByPromptId };
      delete next[promptId];
      return { conversationsByPromptId: next, draft: "", status: { kind: "idle" } };
    }),
}));

/** Selector: full conversation for a prompt id, or an empty list. */
export function conversationFor(
  state: AssistantState,
  promptId: PromptId | null,
): UiMessage[] {
  if (promptId == null) return [];
  return state.conversationsByPromptId[promptId] ?? [];
}

/** Convert a UiMessage's content blocks back into wire-form AssistantMessage,
 * for inclusion in the next turn's message history. */
export function toWireMessage(message: UiMessage): AssistantMessage {
  return { role: message.role, content: message.content };
}
