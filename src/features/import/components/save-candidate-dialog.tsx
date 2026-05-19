// Saved confirmation panel — shown after `save_extracted_prompt` succeeds.
// Provides the slug + path the prompt was written to and a button to
// jump straight to the prompt detail view.

import { Link } from "react-router-dom";

interface Props {
  promptId: string;
  title: string;
  slug: string;
  onImportAnother: () => void;
}

export function SaveCandidateDialog({
  promptId,
  title,
  slug,
  onImportAnother,
}: Props): React.JSX.Element {
  return (
    <section
      aria-label="Saved"
      style={{
        display: "grid",
        gap: 12,
        padding: "var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "1px solid var(--accent-deep)",
        borderRadius: "var(--r-md)",
      }}
    >
      <header style={{ display: "grid", gap: 4 }}>
        <span className="section-label" style={{ color: "var(--accent)", padding: 0 }}>
          saved
        </span>
        <h2
          style={{
            margin: 0,
            fontFamily: "var(--font-display)",
            fontWeight: 500,
            fontSize: 18,
            color: "var(--ink-primary)",
          }}
        >
          {title}
        </h2>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--ink-tertiary)",
          }}
        >
          slug: {slug}
        </div>
      </header>
      <footer style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <button type="button" className="btn" onClick={onImportAnother}>
          import another
        </button>
        <Link
          to={`/prompt/${promptId}`}
          className="btn-launch"
          style={{ textDecoration: "none" }}
          data-testid="import-saved-link"
        >
          open prompt
        </Link>
      </footer>
    </section>
  );
}
