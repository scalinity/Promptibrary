// Per-prompt ephemeral launch draft state.
//
// One draft at a time (per the design: the launch drawer is modal per prompt).
// Values, inline tweak, and overrides are stored here; they reset when the
// drawer closes OR a launch starts successfully.

import { create } from "zustand";

import type { PromptId } from "@/shared/types/ids";
import type { ResolvedVariableValue } from "@/shared/types/launch";
import type { LaunchDefaults } from "@/shared/types/prompt";

export interface LaunchDraftState {
  isOpen: boolean;
  promptId: PromptId | null;
  values: Record<string, ResolvedVariableValue>;
  inlineTweakBody: string | null;
  overrides: Partial<LaunchDefaults>;
  open: (promptId: PromptId) => void;
  close: () => void;
  setValue: (key: string, value: ResolvedVariableValue) => void;
  setInlineTweak: (body: string | null) => void;
  setOverride: <K extends keyof LaunchDefaults>(
    key: K,
    value: LaunchDefaults[K],
  ) => void;
  reset: () => void;
}

export const useLaunchDraftStore = create<LaunchDraftState>((set) => ({
  isOpen: false,
  promptId: null,
  values: {},
  inlineTweakBody: null,
  overrides: {},
  open: (promptId) =>
    set({
      isOpen: true,
      promptId,
      values: {},
      inlineTweakBody: null,
      overrides: {},
    }),
  close: () => set({ isOpen: false }),
  setValue: (key, value) =>
    set((s) => ({ values: { ...s.values, [key]: value } })),
  setInlineTweak: (inlineTweakBody) => set({ inlineTweakBody }),
  setOverride: (key, value) =>
    set((s) => ({ overrides: { ...s.overrides, [key]: value } })),
  reset: () =>
    set({
      isOpen: false,
      promptId: null,
      values: {},
      inlineTweakBody: null,
      overrides: {},
    }),
}));
