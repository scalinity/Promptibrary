// Lightweight editor for a selected candidate. Mirrors the L2 prompt
// editor's intent (title + summary + tags + body + variable-ref list)
// but inline so the user can review-and-edit before save without
// leaving the import flow.

import { useMemo } from "react";

import type { CandidatePrompt } from "@/shared/api/ipc";
import { useImportStore } from "@/features/import/stores/import-store";

interface Props {
  candidate: CandidatePrompt;
  onConfirm: () => void;
}

export function CandidateEditor({ candidate, onConfirm }: Props): React.JSX.Element {
  const patchDraft = useImportStore((s) => s.patchDraft);

  const tagsString = useMemo(() => candidate.tags.join(", "), [candidate.tags]);

  return (
    <section
      aria-label="Candidate editor"
      style={{
        display: "grid",
        gap: 12,
        padding: "var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <header style={{ display: "grid", gap: 6 }}>
        <label
          className="section-label"
          htmlFor="cand-title"
          style={{ padding: 0, color: "var(--ink-tertiary)" }}
        >
          title
        </label>
        <input
          id="cand-title"
          className="input"
          value={candidate.title}
          onChange={(e) => patchDraft({ title: e.target.value })}
        />
      </header>

      <div style={{ display: "grid", gap: 6 }}>
        <label className="section-label" htmlFor="cand-summary" style={{ padding: 0 }}>
          summary
        </label>
        <input
          id="cand-summary"
          className="input"
          value={candidate.summary}
          onChange={(e) => patchDraft({ summary: e.target.value })}
        />
      </div>

      <div style={{ display: "grid", gap: 6 }}>
        <label className="section-label" htmlFor="cand-tags" style={{ padding: 0 }}>
          tags (comma-separated)
        </label>
        <input
          id="cand-tags"
          className="input"
          value={tagsString}
          onChange={(e) =>
            patchDraft({
              tags: e.target.value
                .split(",")
                .map((t) => t.trim())
                .filter((t) => t.length > 0),
            })
          }
        />
      </div>

      <div style={{ display: "grid", gap: 6 }}>
        <label className="section-label" htmlFor="cand-body" style={{ padding: 0 }}>
          body
        </label>
        <textarea
          id="cand-body"
          className="input"
          rows={12}
          value={candidate.body}
          onChange={(e) => patchDraft({ body: e.target.value })}
          style={{ fontFamily: "var(--font-mono)", fontSize: 12, lineHeight: 1.55 }}
        />
      </div>

      {candidate.variables.length > 0 ? (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: 6,
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--ink-secondary)",
          }}
        >
          {candidate.variables.map((v) => (
            <span
              key={v.key}
              style={{
                padding: "2px 6px",
                border: "var(--hairline)",
                borderRadius: "var(--r-xs)",
              }}
            >
              {v.type}:{v.key}
            </span>
          ))}
        </div>
      ) : null}

      <footer
        style={{
          display: "flex",
          justifyContent: "flex-end",
          alignItems: "center",
          gap: 8,
        }}
      >
        <span
          style={{
            marginRight: "auto",
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--ink-tertiary)",
            letterSpacing: "0.02em",
          }}
        >
          rationale: {candidate.rationale}
        </span>
        <button
          type="button"
          className="btn-launch"
          onClick={onConfirm}
          data-testid="import-save-btn"
        >
          save to vault <span className="kbd">⌘↵</span>
        </button>
      </footer>
    </section>
  );
}
