// Right rail listing parsed variable references.
//
// Calls `parseVariables` against the live body and lists each ref. Clicking
// a row scrolls/focuses the corresponding `.pb-var-chip` in the CodeMirror
// editor (DOM-level lookup since chips are widgets, not React).

import { useEffect, useState } from "react";

import { parseVariables, type ParseVariablesResult } from "@/shared/api/ipc";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
  body: string;
}

export function VariableReferenceList({
  prompt: _prompt,
  body,
}: Props): React.JSX.Element {
  const [result, setResult] = useState<ParseVariablesResult | null>(null);

  // Direct useEffect: bridges the body string into an async backend call.
  // 200ms debounce matches the editor. SCA-631 — cancelled flag prevents
  // stale responses from overwriting newer state.
  useEffect(() => {
    let cancelled = false;
    const handle = window.setTimeout(() => {
      parseVariables(body)
        .then((r) => {
          if (cancelled) return;
          setResult(r);
        })
        .catch(() => {
          if (cancelled) return;
          setResult(null);
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [body]);

  const variables = result?.variables ?? [];

  return (
    <div style={{ padding: "var(--sp-5)", display: "grid", gap: 16 }}>
      <div className="section-label">variables</div>
      {variables.length === 0 ? (
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11.5,
            color: "var(--ink-tertiary)",
          }}
        >
          no variable references found
        </div>
      ) : (
        <ul style={{ listStyle: "none", padding: 0, margin: 0, display: "grid", gap: 10 }}>
          {variables.map((v) => (
            <li
              key={v.key}
              onClick={() => focusChip(v.key)}
              style={{
                cursor: "pointer",
                background: "var(--bg-sunken)",
                border: "var(--hairline)",
                borderRadius: "var(--r-md)",
                padding: "var(--sp-3) var(--sp-4)",
                display: "grid",
                gap: 4,
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "baseline",
                }}
              >
                <span
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 13,
                    color: "var(--ink-primary)",
                  }}
                >
                  {v.key}
                </span>
                <span
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 9.5,
                    letterSpacing: "0.08em",
                    color: "var(--accent)",
                    textTransform: "uppercase",
                  }}
                >
                  {v.type}
                  {v.required ? " ·" : ""}
                  {v.required ? <span className="req"> *</span> : ""}
                </span>
              </div>
              {v.description && (
                <p
                  style={{
                    fontFamily: "var(--font-ui)",
                    fontSize: 12,
                    color: "var(--ink-tertiary)",
                    margin: 0,
                  }}
                >
                  {v.description}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
      {result?.errors && result.errors.length > 0 && (
        <div
          role="alert"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--status-error)",
          }}
        >
          {result.errors[0].message}
        </div>
      )}
    </div>
  );
}

const FOCUS_TIMERS = new Map<string, number>();

function focusChip(key: string): void {
  // SCA-642 — CSS.escape protects against future relaxation of the
  // variable key grammar (today restricted to [A-Za-z_][A-Za-z0-9_]*, but
  // pinning the safe form here doesn't cost anything).
  const safeKey = typeof CSS !== "undefined" && CSS.escape
    ? CSS.escape(key)
    : key.replace(/"/g, '\\"');
  const chip = document.querySelector<HTMLElement>(
    `.pb-var-chip[data-var-key="${safeKey}"]`,
  );
  if (chip == null) return;
  chip.scrollIntoView({ block: "center", behavior: "smooth" });
  chip.style.boxShadow = "0 0 0 2px var(--accent)";

  // Clear any existing highlight timer for this key so rapid re-clicks
  // don't race — the most recent click owns the halo's lifetime.
  const prev = FOCUS_TIMERS.get(key);
  if (prev != null) window.clearTimeout(prev);
  const id = window.setTimeout(() => {
    chip.style.boxShadow = "";
    FOCUS_TIMERS.delete(key);
  }, 1200);
  FOCUS_TIMERS.set(key, id);
}
