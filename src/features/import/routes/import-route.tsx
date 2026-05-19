// Route: `/import` — full L4 state machine per spec §12.
//
// Phases: empty → detected → preview_ready → extracting → candidates_ready
//                → editing_candidate → saved
//                                    \
//                                     fetch_failed | extraction_failed

import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";

import {
  extractPromptCandidates,
  fetchSourcePreview,
  saveExtractedPrompt,
  type ExtractionFailure,
} from "@/shared/api/ipc";
import { isAppError } from "@/shared/api/errors";
import { EmptyState } from "@/shared/ui/empty-state";

import { useImportStore } from "@/features/import/stores/import-store";
import { CandidateEditor } from "@/features/import/components/candidate-editor";
import { CandidateList } from "@/features/import/components/candidate-list";
import { ImportFailurePanel } from "@/features/import/components/import-failure-panel";
import { SaveCandidateDialog } from "@/features/import/components/save-candidate-dialog";
import { SourcePreview } from "@/features/import/components/source-preview";
import { SourceUrlForm } from "@/features/import/components/source-url-form";

export function ImportRoute(): React.JSX.Element {
  const navigate = useNavigate();
  const phase = useImportStore((s) => s.phase);
  const detection = useImportStore((s) => s.detection);
  const preview = useImportStore((s) => s.preview);
  const candidates = useImportStore((s) => s.candidates);
  const draft = useImportStore((s) => s.draft);
  const failure = useImportStore((s) => s.failure);
  const url = useImportStore((s) => s.url);
  const extractionMode = useImportStore((s) => s.extractionMode);
  const savedPromptId = useImportStore((s) => s.savedPromptId);

  const setPreview = useImportStore((s) => s.setPreview);
  const setExtracting = useImportStore((s) => s.setExtracting);
  const setCandidates = useImportStore((s) => s.setCandidates);
  const setFetchFailed = useImportStore((s) => s.setFetchFailed);
  const setExtractionFailed = useImportStore((s) => s.setExtractionFailed);
  const setSaved = useImportStore((s) => s.setSaved);
  const reset = useImportStore((s) => s.reset);

  const [topError, setTopError] = useState<string | null>(null);

  const fetchPreview = useCallback(async () => {
    if (!detection || detection.kind === "unsupported") return;
    setTopError(null);
    try {
      const result = await fetchSourcePreview({ url });
      if (result.outcome === "ok") {
        setPreview(result.content);
      } else {
        setFetchFailed(result.failure);
      }
    } catch (e: unknown) {
      const message = isAppError(e) ? e.message : "fetch failed";
      setFetchFailed({ kind: "network_unavailable", message });
    }
  }, [detection, url, setPreview, setFetchFailed]);

  const runExtraction = useCallback(async () => {
    if (!preview) return;
    setExtracting();
    try {
      const result = await extractPromptCandidates({
        content: preview,
        extractionMode,
      });
      if (result.outcome === "ok") {
        setCandidates(result.response.candidates);
      } else {
        setExtractionFailed(result.failure);
      }
    } catch (e: unknown) {
      const f: ExtractionFailure = isAppError(e)
        ? { kind: "extraction_failed", reason: e.message }
        : { kind: "extraction_failed", reason: "unknown" };
      setExtractionFailed(f);
    }
  }, [
    preview,
    extractionMode,
    setExtracting,
    setCandidates,
    setExtractionFailed,
  ]);

  const saveDraft = useCallback(async () => {
    if (!draft || !preview) return;
    try {
      const saved = await saveExtractedPrompt({
        candidate: draft,
        source: preview.source,
      });
      setSaved(saved.id);
    } catch (e: unknown) {
      setExtractionFailed({
        kind: "extraction_failed",
        reason: isAppError(e) ? e.message : "save failed",
      });
    }
  }, [draft, preview, setSaved, setExtractionFailed]);

  // ⌘Enter advances by phase; Esc cancels back to empty.
  useHotkeys(
    "meta+enter, ctrl+enter",
    (e) => {
      e.preventDefault();
      if (phase === "preview_ready") void runExtraction();
      else if (phase === "editing_candidate") void saveDraft();
    },
    { enableOnFormTags: true },
    [phase, runExtraction, saveDraft],
  );

  useHotkeys(
    "esc",
    (e) => {
      if (phase !== "empty") {
        e.preventDefault();
        reset();
      }
    },
    { enableOnFormTags: true },
    [phase, reset],
  );

  return (
    <section
      className="detail-pane"
      aria-label="Import"
      style={{ padding: "var(--sp-7) var(--sp-7)", overflow: "auto" }}
    >
      <div style={{ maxWidth: 720, margin: "0 auto", display: "grid", gap: 20 }}>
        <header>
          <div className="section-label">import</div>
          <h1
            style={{
              fontFamily: "var(--font-display)",
              fontWeight: 500,
              fontSize: 26,
              letterSpacing: "-0.015em",
              margin: "4px 0 8px",
            }}
          >
            Pull a prompt from a URL
          </h1>
          <p
            style={{
              fontFamily: "var(--font-ui)",
              fontSize: 13.5,
              color: "var(--ink-secondary)",
              lineHeight: 1.55,
            }}
          >
            YouTube, X/Twitter, or any article. The extractor is the LLM —
            it reads the page and proposes one or more launch-profile
            candidates you can edit before saving.
          </p>
        </header>

        <StageBar phase={phase} />

        <SourceUrlForm
          onError={setTopError}
          onDetectionReady={() => void fetchPreview()}
        />

        {detection != null && <DetectionChip detection={detection} />}

        {topError != null && (
          <div
            role="alert"
            style={{
              padding: "var(--sp-3) var(--sp-4)",
              background: "var(--bg-sunken)",
              border: "1px solid var(--status-warn)",
              borderRadius: "var(--r-md)",
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--status-warn)",
            }}
          >
            {topError}
          </div>
        )}

        {phase === "preview_ready" && preview != null && (
          <SourcePreview
            content={preview}
            onExtract={() => void runExtraction()}
            extracting={false}
          />
        )}

        {phase === "extracting" && preview != null && (
          <SourcePreview content={preview} onExtract={() => {}} extracting={true} />
        )}

        {phase === "candidates_ready" && (
          <CandidateList candidates={candidates} />
        )}

        {phase === "editing_candidate" && draft != null && (
          <CandidateEditor candidate={draft} onConfirm={() => void saveDraft()} />
        )}

        {phase === "saved" && savedPromptId != null && draft != null && (
          <SaveCandidateDialog
            promptId={savedPromptId}
            title={draft.title}
            slug={draft.title
              .toLowerCase()
              .replace(/[^a-z0-9]+/g, "-")
              .replace(/^-|-$/g, "")}
            onImportAnother={() => {
              reset();
              navigate("/import");
            }}
          />
        )}

        {(phase === "fetch_failed" || phase === "extraction_failed") &&
          failure != null && (
            <ImportFailurePanel
              failure={failure}
              onRetry={
                phase === "fetch_failed"
                  ? () => void fetchPreview()
                  : phase === "extraction_failed"
                    ? () => void runExtraction()
                    : undefined
              }
              onReset={() => reset()}
            />
          )}

        {phase === "empty" && (
          <EmptyState
            glyph="∿"
            title="Paste any URL to start"
            body="YouTube videos, X threads, blog posts — Promptibrary turns them into launch profiles."
          />
        )}
      </div>
    </section>
  );
}

function DetectionChip({
  detection,
}: {
  detection: NonNullable<
    ReturnType<typeof useImportStore.getState>["detection"]
  >;
}) {
  return (
    <div
      style={{
        padding: "var(--sp-3) var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
        display: "flex",
        alignItems: "center",
        gap: 12,
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10.5,
          letterSpacing: "0.06em",
          textTransform: "uppercase",
          color: "var(--accent)",
        }}
      >
        {detection.kind}
      </span>
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 12.5,
          color: "var(--ink-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {detection.kind === "unsupported"
          ? `unsupported: ${detection.reason}`
          : (detection.canonicalUrl ?? "")}
      </span>
    </div>
  );
}

function StageBar({ phase }: { phase: string }) {
  const order: { id: string; label: string; phases: string[] }[] = [
    {
      id: "source",
      label: "source",
      phases: ["empty", "detected", "preview_ready"],
    },
    { id: "extract", label: "extract", phases: ["extracting", "candidates_ready"] },
    { id: "review", label: "review", phases: ["editing_candidate"] },
    { id: "save", label: "save", phases: ["saved"] },
  ];
  const activeIdx = order.findIndex((s) => s.phases.includes(phase));
  return (
    <div
      role="navigation"
      aria-label="Import stages"
      style={{
        display: "flex",
        gap: 18,
        fontFamily: "var(--font-mono)",
        fontSize: 10.5,
        letterSpacing: "0.04em",
        color: "var(--ink-dim)",
        textTransform: "lowercase",
        marginBottom: 4,
      }}
    >
      {order.map((s, i) => {
        const isActive = i === activeIdx;
        const isDone = i < activeIdx && activeIdx >= 0;
        const color = isActive
          ? "var(--accent-warm)"
          : isDone
            ? "var(--ink-secondary)"
            : "var(--ink-dim)";
        return (
          <span
            key={s.id}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              color,
            }}
          >
            <span
              style={{
                display: "inline-grid",
                placeItems: "center",
                width: 18,
                height: 18,
                borderRadius: "50%",
                border: isActive
                  ? "1px solid var(--accent)"
                  : isDone
                    ? "1px solid var(--accent-deep)"
                    : "1px solid var(--border-subtle)",
                color: isActive
                  ? "var(--bg-base)"
                  : isDone
                    ? "var(--accent)"
                    : "var(--ink-dim)",
                background: isActive
                  ? "var(--accent)"
                  : isDone
                    ? "var(--accent-tint)"
                    : "transparent",
              }}
            >
              {i + 1}
            </span>
            {s.label}
            {i < order.length - 1 ? (
              <span style={{ color: "var(--ink-dim)", marginLeft: 4 }}>→</span>
            ) : null}
          </span>
        );
      })}
    </div>
  );
}
