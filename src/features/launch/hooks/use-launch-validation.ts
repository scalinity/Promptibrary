// Client-side variable validation — light pre-check before the launch IPC
// runs the canonical Rust validator (`validate_launch_inputs` lands in L3).
//
// Returns per-field error messages keyed by variable key. The Rust side is
// the contract; this is a UX shortcut so the launch button stays disabled
// before a round-trip.

import { useMemo } from "react";

import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { Prompt } from "@/shared/types/prompt";
import type { Variable } from "@/shared/types/variable";
import type { ResolvedVariableValue } from "@/shared/types/launch";

export interface LaunchValidation {
  fieldErrors: Record<string, string | null>;
  ready: boolean;
}

export function useLaunchValidation(prompt: Prompt): LaunchValidation {
  const values = useLaunchDraftStore((s) => s.values);

  return useMemo(() => {
    const fieldErrors: Record<string, string | null> = {};
    for (const variable of prompt.variables) {
      fieldErrors[variable.key] = validateField(variable, values[variable.key]);
    }
    const ready = Object.values(fieldErrors).every((e) => e == null);
    return { fieldErrors, ready };
  }, [prompt.variables, values]);
}

function validateField(
  variable: Variable,
  value: ResolvedVariableValue | undefined,
): string | null {
  if (value == null) {
    return variable.required && variable.defaultValue == null
      ? "required"
      : null;
  }
  if (value.type !== variable.type) return "wrong type";

  switch (variable.type) {
    case "text":
      if (value.type !== "text") return null;
      if (variable.minLength != null && value.value.length < variable.minLength)
        return `at least ${variable.minLength} characters`;
      if (variable.maxLength != null && value.value.length > variable.maxLength)
        return `at most ${variable.maxLength} characters`;
      if (variable.pattern != null) {
        try {
          if (!new RegExp(variable.pattern).test(value.value)) {
            return "doesn't match required pattern";
          }
        } catch {
          // bad regex falls through; rust will reject.
        }
      }
      return null;
    case "multiline":
      if (value.type !== "multiline") return null;
      if (variable.minLength != null && value.value.length < variable.minLength)
        return `at least ${variable.minLength} characters`;
      if (variable.maxLength != null && value.value.length > variable.maxLength)
        return `at most ${variable.maxLength} characters`;
      return null;
    case "number":
      if (value.type !== "number") return null;
      if (variable.integer && !Number.isInteger(value.value))
        return "must be an integer";
      if (variable.min != null && value.value < variable.min)
        return `min ${variable.min}`;
      if (variable.max != null && value.value > variable.max)
        return `max ${variable.max}`;
      return null;
    case "select":
      if (value.type !== "select") return null;
      if (!variable.options.some((o) => o.value === value.value))
        return "not a valid option";
      return null;
    case "file":
    case "folder":
      // Existence + extension checks defer to the Rust validator; the L2
      // surface only checks for a non-empty path.
      if (value.type === variable.type && value.value.length === 0)
        return "required";
      return null;
    case "bool":
      return null;
  }
}
