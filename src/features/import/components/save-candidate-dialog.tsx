// Saved-confirmation panel — rendered inside the modal body after a
// bulk save. Lists every saved prompt with a deep-link, and offers
// "import another" / "done" CTAs in the modal footer (handled by the
// route container, not here).

import { Link } from "react-router-dom";

import type { SavedRecord } from "@/features/import/stores/import-store";

interface Props {
  saved: SavedRecord[];
}

export function SaveCandidateDialog({ saved }: Props): React.JSX.Element {
  if (saved.length === 0) {
    return (
      <div
        className="extract-line"
        style={{ padding: "20px 24px", paddingLeft: "46px" }}
      >
        nothing was saved.
      </div>
    );
  }
  return (
    <div
      style={{ display: "grid", gap: 0, padding: 0 }}
      role="list"
      aria-label="Saved prompts"
    >
      {saved.map((p) => (
        <Link
          key={p.id}
          to={`/prompt/${p.id}`}
          className="cand on"
          style={{ textDecoration: "none" }}
          data-testid="import-saved-link"
        >
          <div className="check" aria-hidden="true" />
          <div>
            <div className="title">{p.title}</div>
            <div className="preview">slug: {p.slug}</div>
          </div>
          <div className="meta">open ↗</div>
        </Link>
      ))}
    </div>
  );
}
