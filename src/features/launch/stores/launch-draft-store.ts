// Per-prompt ephemeral launch draft state.
//
// One draft at a time (the launch drawer is modal per prompt). Values,
// inline tweak, and overrides reset when the drawer closes OR a launch
// starts successfully.
//
// `open(prompt)` (SCA-639) seeds `values` from each variable's
// `defaultValue` so the form arrives pre-populated rather than blank.
// `close()` and `reset()` both clear `promptId` and `values` (SCA-649) so
// stale state isn't readable while the drawer is closed.

import { create } from "zustand";

import type { PromptId } from "@/shared/types/ids";
import type { ResolvedVariableValue } from "@/shared/types/launch";
import type { LaunchDefaults, Prompt } from "@/shared/types/prompt";

export interface LaunchDraftState {
  isOpen: boolean;
  promptId: PromptId | null;
  values: Record<string, ResolvedVariableValue>;
  inlineTweakBody: string | null;
  overrides: Partial<LaunchDefaults>;
  open: (prompt: Prompt) => void;
  close: () => void;
  setValue: (key: string, value: ResolvedVariableValue) => void;
  clearValue: (key: string) => void;
  setInlineTweak: (body: string | null) => void;
  setOverride: <K extends keyof LaunchDefaults>(
    key: K,
    value: LaunchDefaults[K],
  ) => void;
  reset: () => void;
}

/**
 * Build the initial `values` map for a prompt: include one entry per
 * variable whose `defaultValue` is non-null, typed against that variable's
 * `type`. Keys without defaults stay absent and the form falls back to its
 * own placeholder.
 */
function seedValues(prompt: Prompt): Record<string, ResolvedVariableValue> {
  const out: Record<string, ResolvedVariableValue> = {};
  for (const v of prompt.variables) {
    if (v.defaultValue == null) continue;
    switch (v.type) {
      case "file":
      case "folder":
        out[v.key] = {
          key: v.key,
          type: v.type,
          value: v.defaultValue as never,
        };
        break;
      case "text":
      case "multiline":
      case "select":
        out[v.key] = {
          key: v.key,
          type: v.type,
          value: v.defaultValue as string,
        };
        break;
      case "bool":
        out[v.key] = {
          key: v.key,
          type: "bool",
          value: v.defaultValue as boolean,
        };
        break;
      case "number":
        out[v.key] = {
          key: v.key,
          type: "number",
          value: v.defaultValue as number,
        };
        break;
    }
  }
  return out;
}

export const useLaunchDraftStore = create<LaunchDraftState>((set) => ({
  isOpen: false,
  promptId: null,
  values: {},
  inlineTweakBody: null,
  overrides: {},
  open: (prompt) =>
    set({
      isOpen: true,
      promptId: prompt.id,
      values: seedValues(prompt),
      inlineTweakBody: null,
      overrides: {},
    }),
  close: () =>
    set({
      isOpen: false,
      promptId: null,
      values: {},
      inlineTweakBody: null,
      overrides: {},
    }),
  setValue: (key, value) =>
    set((s) => ({ values: { ...s.values, [key]: value } })),
  clearValue: (key) =>
    set((s) => {
      // Remove the key cleanly so validation sees "missing" rather than
      // a stale value. Used by the number input when cleared (SCA-640).
      const next = { ...s.values };
      delete next[key];
      return { values: next };
    }),
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
