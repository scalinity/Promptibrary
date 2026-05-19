// Route: `/import` — renders the canonical modal overlay per
// `Promptibrary Design System/screens/03-import.html`. The modal floats
// above the app shell (which keeps painting in the background) and
// closes by navigating back to the previous route.
//
// Phases: empty → detected → preview_ready → extracting →
// candidates_ready → saving → saved (+ fetch_failed / extraction_failed).
// All transitions live in `useImportStore`. The route owns side-effects
// (IPC calls) and keyboard shortcuts; everything else is pure render.

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

import "@/features/import/import.css";

import { useImportStore } from "@/features/import/stores/import-store";
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
  const selectedIndices = useImportStore((s) => s.selectedIndices);
  const savedRecords = useImportStore((s) => s.savedRecords);
  const failure = useImportStore((s) => s.failure);
  const url = useImportStore((s) => s.url);
  const extractionMode = useImportStore((s) => s.extractionMode);
  const statusLine = useImportStore((s) => s.statusLine);

  const setPreview = useImportStore((s) => s.setPreview);
  const setExtracting = useImportStore((s) => s.setExtracting);
  const setCandidates = useImportStore((s) => s.setCandidates);
  const setSaving = useImportStore((s) => s.setSaving);
  const setSaved = useImportStore((s) => s.setSaved);
  const setFetchFailed = useImportStore((s) => s.setFetchFailed);
  const setExtractionFailed = useImportStore((s) => s.setExtractionFailed);
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

  const saveSelected = useCallback(async () => {
    if (!preview || selectedIndices.size === 0) return;
    setSaving();
    try {
      const chosen = Array.from(selectedIndices)
        .sort((a, b) => a - b)
        .map((idx) => candidates[idx])
        .filter(
          (c): c is (typeof candidates)[number] => c != null,
        );
      const saved = await Promise.all(
        chosen.map((candidate) =>
          saveExtractedPrompt({ candidate, source: preview.source }),
        ),
      );
      setSaved(
        saved.map((p) => ({ id: p.id, slug: p.slug, title: p.title })),
      );
    } catch (e: unknown) {
      setExtractionFailed({
        kind: "extraction_failed",
        reason: isAppError(e) ? e.message : "save failed",
      });
    }
  }, [
    preview,
    candidates,
    selectedIndices,
    setSaving,
    setSaved,
    setExtractionFailed,
  ]);

  const close = useCallback(() => {
    reset();
    navigate(-1);
  }, [reset, navigate]);

  // ⌘Enter advances by phase; Esc closes the modal (equivalent to
  // pressing the X button in the header).
  useHotkeys(
    "meta+enter, ctrl+enter",
    (e) => {
      e.preventDefault();
      if (phase === "preview_ready") void runExtraction();
      else if (phase === "candidates_ready" && selectedIndices.size > 0) {
        void saveSelected();
      }
    },
    { enableOnFormTags: true },
    [phase, selectedIndices.size, runExtraction, saveSelected],
  );

  useHotkeys(
    "esc",
    (e) => {
      e.preventDefault();
      close();
    },
    { enableOnFormTags: true },
    [close],
  );

  const candidateCount =
    phase === "candidates_ready" ||
    phase === "saving" ||
    phase === "saved"
      ? candidates.length
      : null;
  const selectedCount = selectedIndices.size;
  const saveLabel =
    phase === "saving"
      ? "saving…"
      : selectedCount === 1
        ? "save 1 prompt"
        : `save ${selectedCount} prompts`;

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="import-modal-title"
      onClick={(e) => {
        // Click-outside closes; clicks inside the modal stop propagation.
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="modal import-modal">
        <div className="modal-header">
          <h2 className="modal-title" id="import-modal-title">
            Import prompts from link
          </h2>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close"
            onClick={close}
          >
            ✕
          </button>
        </div>

        <div className="modal-body">
          <div className="import-stage">
            <StageBar phase={phase} />

            <SourceUrlForm
              onError={setTopError}
              onDetectionReady={() => void fetchPreview()}
            />

            {detection && detection.kind !== "unsupported" && preview != null ? (
              <SourcePreview
                content={preview}
                candidateCount={candidateCount}
                onExtract={() => void runExtraction()}
                extracting={phase === "extracting"}
              />
            ) : null}

            {statusLine != null ? (
              <div className="extract-line">{statusLine}</div>
            ) : null}

            {topError != null ? (
              <div
                className="extract-line"
                role="alert"
                style={{ color: "var(--status-warn)" }}
              >
                {topError}
              </div>
            ) : null}
          </div>

          {phase === "candidates_ready" || phase === "saving" ? (
            <CandidateList candidates={candidates} />
          ) : null}

          {phase === "saved" ? <SaveCandidateDialog saved={savedRecords} /> : null}

          {(phase === "fetch_failed" || phase === "extraction_failed") &&
          failure != null ? (
            <div style={{ padding: "20px 24px" }}>
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
            </div>
          ) : null}
        </div>

        <ImportFooter
          phase={phase}
          selectedCount={selectedCount}
          totalCount={candidates.length}
          saveLabel={saveLabel}
          onCancel={close}
          onSave={() => void saveSelected()}
          onImportAnother={() => reset()}
        />
      </div>
    </div>
  );
}

function ImportFooter({
  phase,
  selectedCount,
  totalCount,
  saveLabel,
  onCancel,
  onSave,
  onImportAnother,
}: {
  phase: string;
  selectedCount: number;
  totalCount: number;
  saveLabel: string;
  onCancel: () => void;
  onSave: () => void;
  onImportAnother: () => void;
}): React.JSX.Element | null {
  if (phase === "candidates_ready" || phase === "saving") {
    return (
      <div className="modal-footer">
        <span className="import-footer-status">
          {selectedCount} of {totalCount} selected · will be tagged{" "}
          <span className="tag" style={{ margin: "0 4px" }}>
            imported
          </span>
        </span>
        <button type="button" className="btn" onClick={onCancel}>
          cancel
        </button>
        <button
          type="button"
          className="btn-launch compact"
          onClick={onSave}
          disabled={selectedCount === 0 || phase === "saving"}
        >
          {saveLabel}
        </button>
      </div>
    );
  }
  if (phase === "saved") {
    return (
      <div className="modal-footer">
        <span className="import-footer-status">imported.</span>
        <button type="button" className="btn" onClick={onImportAnother}>
          import another
        </button>
        <button type="button" className="btn-launch compact" onClick={onCancel}>
          done
        </button>
      </div>
    );
  }
  // Empty / detected / preview_ready / extracting / failure phases —
  // no footer actions yet; closing happens via the X or Esc.
  return null;
}

function StageBar({ phase }: { phase: string }) {
  const order: { id: string; label: string; phases: string[] }[] = [
    { id: "source", label: "source", phases: ["empty", "detected", "preview_ready"] },
    { id: "extracting", label: "extracting", phases: ["extracting"] },
    {
      id: "review",
      label: "review",
      phases: ["candidates_ready", "saving"],
    },
    { id: "save", label: "save", phases: ["saved"] },
  ];
  const activeIdx = order.findIndex((s) => s.phases.includes(phase));
  return (
    <div className="stage-bar" aria-label="Import stages">
      {order.map((s, i) => {
        const className =
          i === activeIdx ? "step now" : i < activeIdx ? "step done" : "step";
        return (
          <span key={s.id} className={className}>
            <span className="n">{i + 1}</span>
            {s.label}
            {i < order.length - 1 ? <span className="sep">→</span> : null}
          </span>
        );
      })}
    </div>
  );
}
