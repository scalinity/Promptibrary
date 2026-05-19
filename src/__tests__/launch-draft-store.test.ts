// Launch draft store lifecycle:
//   open(prompt) seeds values from variable defaults, setValue accumulates
//   per-key, clearValue removes the key, setInlineTweak toggles the tweak
//   body, close() and reset() clear everything.

import { describe, expect, it, beforeEach } from "vitest";

import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { PROMPT_FIXTURES } from "../../tests/fixtures/visual/prompts";
import type { Prompt, LaunchDefaults } from "@/shared/types/prompt";
import type { TextVariable, NumberVariable, BoolVariable } from "@/shared/types/variable";

function buildPrompt(): Prompt {
  // Use the first fixture as a base and overlay variables with defaults so
  // we exercise the seeding branch.
  const base = PROMPT_FIXTURES[0];
  const textWithDefault: TextVariable = {
    key: "branch",
    type: "text",
    label: "branch",
    description: null,
    required: false,
    defaultValue: "main",
    order: 1,
    source: "frontmatter",
    minLength: null,
    maxLength: null,
    pattern: null,
    trim: true,
  };
  const numberNoDefault: NumberVariable = {
    key: "retries",
    type: "number",
    label: "retries",
    description: null,
    required: true,
    defaultValue: null,
    order: 2,
    source: "frontmatter",
    min: 0,
    max: 5,
    step: 1,
    integer: true,
  };
  const boolWithDefault: BoolVariable = {
    key: "enable_compat",
    type: "bool",
    label: "compat loader",
    description: null,
    required: false,
    defaultValue: false,
    order: 3,
    source: "frontmatter",
    renderTrue: "enabled",
    renderFalse: "disabled",
  };
  return {
    ...base,
    variables: [textWithDefault, numberNoDefault, boolWithDefault],
    launchDefaults: base.launchDefaults as LaunchDefaults,
  };
}

describe("useLaunchDraftStore", () => {
  beforeEach(() => {
    useLaunchDraftStore.getState().reset();
  });

  it("open(prompt) seeds values from variable defaults", () => {
    const prompt = buildPrompt();
    useLaunchDraftStore.getState().open(prompt);
    const state = useLaunchDraftStore.getState();
    expect(state.isOpen).toBe(true);
    expect(state.promptId).toBe(prompt.id);
    expect(state.values).toEqual({
      branch: { key: "branch", type: "text", value: "main" },
      enable_compat: { key: "enable_compat", type: "bool", value: false },
    });
    // The required `retries` variable has no defaultValue → no entry.
    expect(state.values.retries).toBeUndefined();
  });

  it("accumulates values by key", () => {
    useLaunchDraftStore.getState().open(buildPrompt());
    useLaunchDraftStore.getState().setValue("retries", {
      key: "retries",
      type: "number",
      value: 3,
    });
    const state = useLaunchDraftStore.getState();
    expect(state.values.retries).toEqual({
      key: "retries",
      type: "number",
      value: 3,
    });
  });

  it("clearValue removes the key", () => {
    useLaunchDraftStore.getState().open(buildPrompt());
    useLaunchDraftStore.getState().setValue("retries", {
      key: "retries",
      type: "number",
      value: 3,
    });
    useLaunchDraftStore.getState().clearValue("retries");
    expect(useLaunchDraftStore.getState().values.retries).toBeUndefined();
  });

  it("setInlineTweak toggles between body and null", () => {
    useLaunchDraftStore.getState().setInlineTweak("hello");
    expect(useLaunchDraftStore.getState().inlineTweakBody).toBe("hello");
    useLaunchDraftStore.getState().setInlineTweak(null);
    expect(useLaunchDraftStore.getState().inlineTweakBody).toBeNull();
  });

  it("close() clears every field", () => {
    useLaunchDraftStore.getState().open(buildPrompt());
    useLaunchDraftStore.getState().setInlineTweak("body");
    useLaunchDraftStore.getState().close();
    const state = useLaunchDraftStore.getState();
    expect(state.isOpen).toBe(false);
    expect(state.promptId).toBeNull();
    expect(state.values).toEqual({});
    expect(state.inlineTweakBody).toBeNull();
    expect(state.overrides).toEqual({});
  });

  it("reset() clears every field", () => {
    useLaunchDraftStore.getState().open(buildPrompt());
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
