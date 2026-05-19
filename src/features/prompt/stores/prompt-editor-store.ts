// Per-prompt unsaved-draft state — body edits live here until the user
// triggers save, which calls `updatePrompt` (mutation lands with L5 git
// integration). The store is keyed by PromptId so navigating between
// prompts doesn't lose work.

import { create } from "zustand";

import type { PromptId } from "@/shared/types/ids";

export interface PromptEditorState {
  body: Record<string, string>;
  showHistory: boolean;
  setBody: (id: PromptId, body: string) => void;
  clear: (id: PromptId) => void;
  toggleHistory: () => void;
}

export const usePromptEditorStore = create<PromptEditorState>((set) => ({
  body: {},
  showHistory: false,
  setBody: (id, body) =>
    set((s) => ({ body: { ...s.body, [id]: body } })),
  clear: (id) =>
    set((s) => {
      const { [id]: _drop, ...rest } = s.body;
      return { body: rest };
    }),
  toggleHistory: () => set((s) => ({ showHistory: !s.showHistory })),
}));
