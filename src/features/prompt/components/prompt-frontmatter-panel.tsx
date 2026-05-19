// Frontmatter panel — title, summary, tags, source info.
//
// L2 surface is read-mostly: title + summary + tags display, source is
// read-only when the prompt was imported (non-manual). Mutations land in
// L5 with the prompt-save IPC.

import type { Prompt } from "@/shared/types/prompt";
import { formatRelative } from "@/shared/lib/dates";

interface Props {
  prompt: Prompt;
}

export function PromptFrontmatterPanel({ prompt }: Props): React.JSX.Element {
  return (
    <div style={{ padding: "var(--sp-5)", display: "grid", gap: 20 }}>
      <Field label="title">
        <div
          style={{
            fontFamily: "var(--font-display)",
            fontWeight: 500,
            fontSize: 17,
            color: "var(--ink-primary)",
            letterSpacing: "-0.01em",
          }}
        >
          {prompt.title}
        </div>
      </Field>
      {prompt.summary && (
        <Field label="summary">
          <div
            style={{
              fontFamily: "var(--font-ui)",
              fontSize: 13,
              color: "var(--ink-secondary)",
              lineHeight: 1.5,
            }}
          >
            {prompt.summary}
          </div>
        </Field>
      )}
      <Field label="tags">
        <div className="tag-row">
          {prompt.tags.length === 0 ? (
            <span style={{ color: "var(--ink-tertiary)", fontSize: 11.5 }}>
              none
            </span>
          ) : (
            prompt.tags.map((t) => (
              <span key={t} className="tag">
                {t}
              </span>
            ))
          )}
        </div>
      </Field>
      <Field label="source">
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            color: "var(--ink-secondary)",
          }}
        >
          {prompt.source.kind === "manual"
            ? "manual entry"
            : prompt.source.originUrl ?? prompt.source.kind}
        </div>
      </Field>
      <Field label="last edited">
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11.5,
            color: "var(--ink-tertiary)",
          }}
        >
          {formatRelative(prompt.updatedAt)}
        </div>
      </Field>
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <div style={{ display: "grid", gap: 6 }}>
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10.5,
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {label}
      </span>
      {children}
    </div>
  );
}
