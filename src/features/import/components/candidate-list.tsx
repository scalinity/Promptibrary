// List of extraction candidates with confidence chip + summary + variable
// count. Selecting one transitions the store into `editing_candidate`.

import type { CandidatePrompt } from "@/shared/api/ipc";
import { useImportStore } from "@/features/import/stores/import-store";

interface Props {
  candidates: CandidatePrompt[];
}

const CONFIDENCE_COLOR: Record<CandidatePrompt["confidence"], string> = {
  low: "var(--ink-tertiary)",
  medium: "var(--accent-warm)",
  high: "var(--accent)",
};

export function CandidateList({ candidates }: Props): React.JSX.Element {
  const selectCandidate = useImportStore((s) => s.selectCandidate);
  const selected = useImportStore((s) => s.selectedCandidateIndex);

  if (candidates.length === 0) {
    return (
      <div
        style={{
          padding: "var(--sp-4)",
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          color: "var(--ink-tertiary)",
        }}
      >
        no candidates returned — try deep mode or a different source
      </div>
    );
  }

  return (
    <ul
      data-testid="import-candidate-list"
      style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: 6 }}
      aria-label="Extraction candidates"
    >
      {candidates.map((c, i) => {
        const isActive = selected === i;
        return (
          <li key={`${c.title}-${i}`}>
            <button
              type="button"
              onClick={() => selectCandidate(i)}
              style={{
                width: "100%",
                display: "grid",
                gridTemplateColumns: "1fr auto",
                gap: 12,
                alignItems: "flex-start",
                padding: "var(--sp-3) var(--sp-4)",
                background: isActive ? "var(--bg-sunken)" : "transparent",
                border: isActive ? "1px solid var(--accent-deep)" : "var(--hairline)",
                borderRadius: "var(--r-md)",
                cursor: "pointer",
                textAlign: "left",
              }}
            >
              <div style={{ display: "grid", gap: 4 }}>
                <div
                  style={{
                    fontFamily: "var(--font-ui)",
                    fontWeight: 500,
                    fontSize: 13.5,
                    color: "var(--ink-primary)",
                  }}
                >
                  {c.title}
                </div>
                <div
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 11,
                    color: "var(--ink-tertiary)",
                    lineHeight: 1.4,
                  }}
                >
                  {c.summary}
                </div>
                <div
                  style={{
                    display: "flex",
                    gap: 6,
                    flexWrap: "wrap",
                    marginTop: 2,
                  }}
                >
                  {c.tags.map((t) => (
                    <span
                      key={t}
                      className="tag"
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: 10,
                        color: "var(--ink-secondary)",
                        padding: "2px 6px",
                        border: "var(--hairline)",
                        borderRadius: "var(--r-xs)",
                      }}
                    >
                      {t}
                    </span>
                  ))}
                </div>
              </div>
              <div
                style={{
                  display: "grid",
                  gap: 4,
                  textAlign: "right",
                  fontFamily: "var(--font-mono)",
                  fontSize: 10,
                  letterSpacing: "0.04em",
                  color: "var(--ink-dim)",
                }}
              >
                <span
                  style={{
                    color: CONFIDENCE_COLOR[c.confidence],
                    textTransform: "uppercase",
                  }}
                >
                  {c.confidence}
                </span>
                <span>{c.variables.length} vars</span>
                <span>{Math.round(c.body.split(/\s+/).length)} w</span>
              </div>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
