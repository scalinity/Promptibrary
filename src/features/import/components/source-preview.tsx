// Preview of normalized source content: title, author, chunk count, and
// the extract button that fires the mode-aware LLM call.

import type { FetchedSourceContent } from "@/shared/api/ipc";
import { ExtractionModeToggle } from "@/features/import/components/extraction-mode-toggle";

interface Props {
  content: FetchedSourceContent;
  onExtract: () => void;
  extracting: boolean;
}

export function SourcePreview({
  content,
  onExtract,
  extracting,
}: Props): React.JSX.Element {
  const kind = content.source.kind;
  return (
    <section
      aria-label="Source preview"
      style={{
        display: "grid",
        gap: 12,
        padding: "var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
        }}
      >
        <span
          className="source-badge"
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            padding: "4px 8px",
            border: "1px solid var(--accent-deep)",
            background: "var(--accent-tint)",
            borderRadius: "var(--r-xs)",
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--accent-warm)",
            letterSpacing: "0.04em",
            textTransform: "lowercase",
          }}
        >
          detected · {kind.replace("_", "/")}
        </span>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--ink-tertiary)",
            letterSpacing: "0.04em",
          }}
        >
          {content.chunks.length} chunk{content.chunks.length === 1 ? "" : "s"}
          {content.cached ? " · cached" : ""}
        </span>
      </header>

      <div style={{ display: "grid", gap: 4 }}>
        {content.title != null ? (
          <h2
            style={{
              margin: 0,
              fontFamily: "var(--font-display)",
              fontWeight: 500,
              fontSize: 18,
              color: "var(--ink-primary)",
              letterSpacing: "-0.01em",
            }}
          >
            {content.title}
          </h2>
        ) : null}
        {content.author != null ? (
          <div
            style={{
              fontFamily: "var(--font-ui)",
              fontSize: 12,
              color: "var(--ink-secondary)",
            }}
          >
            {content.author}
          </div>
        ) : null}
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--ink-tertiary)",
            marginTop: 6,
            whiteSpace: "pre-wrap",
            maxHeight: 180,
            overflow: "auto",
          }}
        >
          {content.text.slice(0, 1200)}
          {content.text.length > 1200 ? "…" : ""}
        </div>
      </div>

      <footer
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          flexWrap: "wrap",
        }}
      >
        <ExtractionModeToggle />
        <button
          type="button"
          className="btn"
          onClick={onExtract}
          disabled={extracting}
          data-testid="import-extract-btn"
        >
          {extracting ? "extracting…" : "extract candidates"}{" "}
          <span className="kbd">⌘↵</span>
        </button>
      </footer>
    </section>
  );
}
