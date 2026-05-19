// URL input row inside the import modal. Matches the `.url-row` markup
// from `Promptibrary Design System/screens/03-import.html`.

import { useState } from "react";

import { detectSourceTyped } from "@/shared/api/ipc";
import { isAppError } from "@/shared/api/errors";
import { useImportStore } from "@/features/import/stores/import-store";

interface Props {
  onError?: (message: string) => void;
  onDetectionReady?: () => void;
}

export function SourceUrlForm({ onError, onDetectionReady }: Props): React.JSX.Element {
  const url = useImportStore((s) => s.url);
  const setUrl = useImportStore((s) => s.setUrl);
  const setDetection = useImportStore((s) => s.setDetection);
  const phase = useImportStore((s) => s.phase);
  const reset = useImportStore((s) => s.reset);
  const [pending, setPending] = useState(false);

  // Once a source has been fetched, the URL row shows "change" instead
  // of "detect" (matching the mockup) — clicking resets the store back
  // to the empty phase for a new URL.
  const showChange =
    phase === "preview_ready" ||
    phase === "candidates_ready" ||
    phase === "saving";

  const submit = async () => {
    setPending(true);
    try {
      const detection = await detectSourceTyped(url.trim());
      setDetection(detection);
      if (detection.kind !== "unsupported") {
        onDetectionReady?.();
      }
    } catch (e: unknown) {
      const message = isAppError(e) ? e.message : "detection failed";
      onError?.(message);
      setDetection(null);
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="url-row">
      <input
        className="input"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://…"
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Enter" && !showChange && url.trim().length > 0) {
            e.preventDefault();
            void submit();
          }
        }}
        aria-label="Source URL"
        data-testid="import-url-input"
        readOnly={showChange}
      />
      {showChange ? (
        <button type="button" className="btn" onClick={() => reset()}>
          change
        </button>
      ) : (
        <button
          type="button"
          className="btn"
          disabled={url.trim().length === 0 || pending}
          onClick={() => void submit()}
        >
          {pending ? "detecting…" : "detect"} <span className="kbd">⌘↵</span>
        </button>
      )}
    </div>
  );
}
