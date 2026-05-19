// URL input + "detect" button. Calls `detectSourceTyped` and hands the
// result to the store; the detected chip renders below.

import { useState } from "react";

import { detectSourceTyped } from "@/shared/api/ipc";
import { isAppError } from "@/shared/api/errors";
import { useImportStore } from "@/features/import/stores/import-store";

interface Props {
  onError?: (message: string) => void;
}

export function SourceUrlForm({ onError }: Props): React.JSX.Element {
  const url = useImportStore((s) => s.url);
  const setUrl = useImportStore((s) => s.setUrl);
  const setDetection = useImportStore((s) => s.setDetection);
  const [pending, setPending] = useState(false);

  const submit = async () => {
    setPending(true);
    try {
      const detection = await detectSourceTyped(url.trim());
      setDetection(detection);
    } catch (e: unknown) {
      const message = isAppError(e) ? e.message : "detection failed";
      onError?.(message);
      setDetection(null);
    } finally {
      setPending(false);
    }
  };

  return (
    <div
      className="url-row"
      style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 10 }}
    >
      <input
        className="input"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://…"
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Enter" && url.trim().length > 0) {
            e.preventDefault();
            void submit();
          }
        }}
        aria-label="Source URL"
        data-testid="import-url-input"
      />
      <button
        type="button"
        className="btn"
        disabled={url.trim().length === 0 || pending}
        onClick={() => void submit()}
      >
        {pending ? "detecting…" : "detect"} <span className="kbd">⌘↵</span>
      </button>
    </div>
  );
}
