// Updater status + manual check.
//
// L5 implements the actual tauri-plugin-updater check; L2 surface only
// renders the panel structure and the "check" button (which surfaces a
// pending-layer toast on click).

import { useState } from "react";

export function UpdaterSettings(): React.JSX.Element {
  const [toast, setToast] = useState<string | null>(null);
  return (
    <section id="updater" aria-labelledby="updater-h">
      <div className="section-label" id="updater-h">
        updater
      </div>
      <div
        style={{
          display: "grid",
          gap: "var(--sp-3)",
          marginTop: 8,
          background: "var(--bg-sunken)",
          border: "var(--hairline)",
          borderRadius: "var(--r-md)",
          padding: "var(--sp-3) var(--sp-4)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <span
            style={{
              fontFamily: "var(--font-ui)",
              fontSize: "12.5px",
              color: "var(--ink-primary)",
            }}
          >
            current version
          </span>
          <span
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: "12.5px",
              color: "var(--accent)",
            }}
          >
            0.0.0 (L2 dev)
          </span>
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button
            type="button"
            className="btn"
            onClick={() => {
              setToast("auto-update check activates in L5");
              window.setTimeout(() => setToast(null), 2000);
            }}
          >
            check for updates
          </button>
        </div>
        {toast != null && (
          <div
            role="status"
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: "11px",
              color: "var(--status-warn)",
              textAlign: "right",
            }}
          >
            {toast}
          </div>
        )}
      </div>
    </section>
  );
}
