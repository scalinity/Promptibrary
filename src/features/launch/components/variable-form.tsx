// Variable form — one VariableControl per parsed variable, in `order`.
//
// Validates client-side via `useLaunchValidation`. Order is the source of
// truth for rendering (the parser preserves source-order in `refs` but
// frontmatter can reorder declarations).

import { useMemo } from "react";

import { VariableControl } from "./variable-control";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { LaunchValidation } from "@/features/launch/hooks/use-launch-validation";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
  validation: LaunchValidation;
}

export function VariableForm({ prompt, validation }: Props): React.JSX.Element {
  const values = useLaunchDraftStore((s) => s.values);
  const setValue = useLaunchDraftStore((s) => s.setValue);
  const clearValue = useLaunchDraftStore((s) => s.clearValue);

  const ordered = useMemo(
    () => [...prompt.variables].sort((a, b) => a.order - b.order),
    [prompt.variables],
  );

  if (ordered.length === 0) {
    return (
      <section>
        <div className="section-label">variables</div>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--ink-tertiary)",
            letterSpacing: "0.01em",
          }}
        >
          no variables declared
        </div>
      </section>
    );
  }

  const required = ordered.filter((v) => v.required).length;

  return (
    <section>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 12,
        }}
      >
        <span className="section-label">variables</span>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "10.5px",
            color: "var(--ink-dim)",
            letterSpacing: "0.04em",
          }}
        >
          {ordered.length} fields · {required} required
        </span>
      </div>
      <div className="var-grid">
        {ordered.map((variable) => (
          <VariableControl
            key={variable.key}
            variable={variable}
            value={values[variable.key]}
            error={validation.fieldErrors[variable.key] ?? null}
            onChange={(v) => setValue(variable.key, v)}
            onClear={clearValue}
          />
        ))}
      </div>
    </section>
  );
}
