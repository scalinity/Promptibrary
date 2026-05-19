// Launch draft store lifecycle:
//   open(promptId) seeds an empty draft, setValue accumulates per-key,
//   setInlineTweak toggles the tweak body, reset clears everything.

import { describe, expect, it, beforeEach } from "vitest";

import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { asPromptId } from "@/shared/types/ids";

describe("useLaunchDraftStore", () => {
  beforeEach(() => {
    useLaunchDraftStore.getState().reset();
  });

  it("opens with an empty draft for the given prompt", () => {
    const id = asPromptId("01TEST");
    useLaunchDraftStore.getState().open(id);
    const state = useLaunchDraftStore.getState();
    expect(state.isOpen).toBe(true);
    expect(state.promptId).toBe(id);
    expect(state.values).toEqual({});
    expect(state.inlineTweakBody).toBeNull();
    expect(state.overrides).toEqual({});
  });

  it("accumulates values by key", () => {
    useLaunchDraftStore.getState().open(asPromptId("01TEST"));
    useLaunchDraftStore.getState().setValue("branch", {
      key: "branch",
      type: "text",
      value: "main",
    });
    useLaunchDraftStore.getState().setValue("retries", {
      key: "retries",
      type: "number",
      value: 3,
    });
    const state = useLaunchDraftStore.getState();
    expect(state.values.branch).toEqual({
      key: "branch",
      type: "text",
      value: "main",
    });
    expect(state.values.retries).toEqual({
      key: "retries",
      type: "number",
      value: 3,
    });
  });

  it("setInlineTweak toggles between body and null", () => {
    useLaunchDraftStore.getState().setInlineTweak("hello");
    expect(useLaunchDraftStore.getState().inlineTweakBody).toBe("hello");
    useLaunchDraftStore.getState().setInlineTweak(null);
    expect(useLaunchDraftStore.getState().inlineTweakBody).toBeNull();
  });

  it("reset clears every field", () => {
    useLaunchDraftStore.getState().open(asPromptId("01TEST"));
    useLaunchDraftStore.getState().setValue("k", {
      key: "k",
      type: "text",
      value: "v",
    });
    useLaunchDraftStore.getState().setInlineTweak("body");
    useLaunchDraftStore.getState().reset();
    const state = useLaunchDraftStore.getState();
    expect(state.isOpen).toBe(false);
    expect(state.promptId).toBeNull();
    expect(state.values).toEqual({});
    expect(state.inlineTweakBody).toBeNull();
    expect(state.overrides).toEqual({});
  });
});
