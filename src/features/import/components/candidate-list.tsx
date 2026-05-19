// Candidate list with multi-select checkboxes, matching `.cand` from
// `Promptibrary Design System/screens/03-import.html`. Each row is a
// button so it's keyboard-accessible (space/enter toggles selection).

import type { CandidatePrompt } from "@/shared/api/ipc";
import { useImportStore } from "@/features/import/stores/import-store";

interface Props {
  candidates: CandidatePrompt[];
}

function previewBody(body: string): string {
  // The mockup shows the first ~80 chars of the body as a single-line
  // teaser; preserve template variables visually but collapse newlines.
  return body.replace(/\s+/g, " ").trim();
}

function wordCount(body: string): number {
  return body.trim().split(/\s+/).filter(Boolean).length;
}

export function CandidateList({ candidates }: Props): React.JSX.Element {
  const selected = useImportStore((s) => s.selectedIndices);
  const toggle = useImportStore((s) => s.toggleCandidate);

  if (candidates.length === 0) {
    return (
      <div
        className="extract-line"
        style={{ padding: "20px 24px", paddingLeft: "46px" }}
      >
        no candidates returned — try deep mode or a different source
      </div>
    );
  }

  return (
    <div
      className="candidates"
      role="group"
      aria-label="Extraction candidates"
      data-testid="import-candidate-list"
    >
      {candidates.map((c, i) => {
        const isOn = selected.has(i);
        return (
          <button
            key={`${c.title}-${i}`}
            type="button"
            className={isOn ? "cand on" : "cand"}
            onClick={() => toggle(i)}
            aria-pressed={isOn}
            aria-label={`${isOn ? "Unselect" : "Select"} ${c.title}`}
          >
            <div className="check" aria-hidden="true" />
            <div>
              <div className="title">{c.title}</div>
              <div className="preview">{previewBody(c.body)}</div>
            </div>
            <div className="meta">
              {c.variables.length} {c.variables.length === 1 ? "var" : "vars"}
              {" · "}
              {wordCount(c.body)}w
            </div>
          </button>
        );
      })}
    </div>
  );
}
