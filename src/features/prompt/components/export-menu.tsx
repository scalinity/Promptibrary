// Export-as menu — calls exportPrompt with a save-dialog destination.
//
// L1 wires the IPC; this surface picks markdown vs JSON and prompts for a
// destination path.

import { useState } from "react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

import { exportPrompt, type ExportFormat } from "@/shared/api/ipc";
import { asAbsolutePath } from "@/shared/types/ids";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
}

export function ExportMenu({ prompt }: Props): React.JSX.Element {
  const [pending, setPending] = useState<ExportFormat | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const onExport = async (format: ExportFormat) => {
    const ext = format === "markdown" ? "md" : "json";
    const result = await saveDialog({
      defaultPath: `${prompt.slug}.${ext}`,
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (typeof result !== "string") return;
    setPending(format);
    try {
      const dest = await exportPrompt({
        id: prompt.id,
        format,
        destination: asAbsolutePath(result),
      });
      setToast(`exported to ${dest}`);
    } catch {
      setToast("export failed");
    } finally {
      setPending(null);
      window.setTimeout(() => setToast(null), 2400);
    }
  };

  return (
    <div style={{ padding: "0 var(--sp-5) var(--sp-5)", display: "grid", gap: 8 }}>
      <div className="section-label">export</div>
      <div style={{ display: "flex", gap: 8 }}>
        <button
          type="button"
          className="btn"
          disabled={pending === "markdown"}
          onClick={() => onExport("markdown")}
        >
          {pending === "markdown" ? "exporting…" : "markdown"}
        </button>
        <button
          type="button"
          className="btn"
          disabled={pending === "json"}
          onClick={() => onExport("json")}
        >
          {pending === "json" ? "exporting…" : "json"}
        </button>
      </div>
      {toast != null && (
        <div
          role="status"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--ink-tertiary)",
          }}
        >
          {toast}
        </div>
      )}
    </div>
  );
}
