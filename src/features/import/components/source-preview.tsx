// Source preview badge row — matches the `.source-badge` markup in the
// mockup. Two pills side by side: the live-pulsing "detected · <kind>"
// indicator on the left, the muted "N candidate prompts" pill on the
// right (only shown once candidates exist). The "extract candidates"
// CTA + mode toggle sit beside the badges.

import type { FetchedSourceContent } from "@/shared/api/ipc";
import { ExtractionModeToggle } from "@/features/import/components/extraction-mode-toggle";

interface Props {
  content: FetchedSourceContent;
  candidateCount: number | null;
  onExtract: () => void;
  extracting: boolean;
}

function sourceKindLabel(content: FetchedSourceContent): string {
  // The mockup renders e.g. "detected · github gist · markdown". We
  // approximate with kind + author/title hint where available so the
  // badge stays specific without misrepresenting source-detection.
  const kind = content.source.kind.replace("_", "/");
  if (content.source.kind === "youtube") {
    return `detected · youtube · ${content.author ?? "video"}`;
  }
  if (content.source.kind === "x_twitter") {
    return `detected · x/twitter · ${content.author ?? "post"}`;
  }
  if (content.source.kind === "article") {
    const host = (() => {
      try {
        return new URL(content.canonicalUrl).hostname.replace(/^www\./, "");
      } catch {
        return null;
      }
    })();
    return `detected · article${host ? ` · ${host}` : ""}`;
  }
  return `detected · ${kind}`;
}

export function SourcePreview({
  content,
  candidateCount,
  onExtract,
  extracting,
}: Props): React.JSX.Element {
  return (
    <>
      <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
        <span className="source-badge">
          <span className="dot" aria-hidden="true" />
          {sourceKindLabel(content)}
          {content.cached ? " · cached" : ""}
        </span>
        {candidateCount != null ? (
          <span className="source-badge muted">
            {candidateCount} candidate {candidateCount === 1 ? "prompt" : "prompts"}
          </span>
        ) : null}
      </div>

      {/* Mode toggle + extract CTA only show before extraction. After
          extraction the CTA disappears; users save via the modal footer. */}
      {candidateCount == null ? (
        <div
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
        </div>
      ) : null}
    </>
  );
}
