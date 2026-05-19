// Telemetry toggle + the two distinct destructive actions per spec §13.
//
// Critically, these two surfaces stay separate — bundling them into one
// "danger zone" button was the V1 mistake the patch reversed.
//   • Clear telemetry cache: single-click confirmation, drops aggregates,
//     keeps runs + transcripts.
//   • Delete all run history: typed "delete" confirmation, drops the runs
//     table AND deletes transcript files.

import { useState } from "react";

import { useSettings } from "@/features/settings/hooks/use-settings";

export function TelemetrySettings(): React.JSX.Element {
  const settings = useSettings();
  const enabled = settings.data?.local.telemetryEnabled ?? false;

  return (
    <section id="telemetry" aria-labelledby="telemetry-h">
      <div className="section-label" id="telemetry-h">
        telemetry
      </div>
      <label
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "var(--sp-3) var(--sp-4)",
          background: "var(--bg-sunken)",
          border: "var(--hairline)",
          borderRadius: "var(--r-md)",
          marginTop: 8,
          fontFamily: "var(--font-ui)",
          fontSize: "12.5px",
          color: "var(--ink-primary)",
        }}
      >
        <input
          type="checkbox"
          checked={enabled}
          disabled
          style={{ accentColor: "var(--accent)" }}
        />
        record run telemetry locally (count, duration, exit code)
        <span
          style={{
            marginLeft: "auto",
            fontFamily: "var(--font-mono)",
            fontSize: "10.5px",
            color: "var(--ink-tertiary)",
          }}
        >
          editing activates in L5
        </span>
      </label>

      <div className="section-label" style={{ marginTop: 24 }}>
        data
      </div>
      <div style={{ display: "grid", gap: "var(--sp-3)", marginTop: 8 }}>
        <ClearCacheAction />
        <DeleteHistoryAction />
      </div>
    </section>
  );
}

function ClearCacheAction(): React.JSX.Element {
  const [toast, setToast] = useState<string | null>(null);
  return (
    <DestructiveCard
      label="clear telemetry cache"
      body="Drops aggregated counts. Run records and transcripts are kept."
      confirmLabel="clear cache"
      kind="warn"
      toast={toast}
      onConfirm={() => {
        setToast("available in L5 — UI scaffold only");
        window.setTimeout(() => setToast(null), 2400);
      }}
    />
  );
}

function DeleteHistoryAction(): React.JSX.Element {
  const [typed, setTyped] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const armed = typed === "delete";
  return (
    <div
      style={{
        background: "var(--bg-sunken)",
        border: "1px solid oklch(0.4 0.1 25)",
        borderRadius: "var(--r-md)",
        padding: "var(--sp-3) var(--sp-4)",
        display: "grid",
        gap: 6,
      }}
    >
      <div
        style={{
          fontFamily: "var(--font-ui)",
          fontSize: "13px",
          fontWeight: 500,
          color: "var(--status-error)",
        }}
      >
        delete all run history
      </div>
      <p
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          margin: 0,
        }}
      >
        Drops the <code>runs</code> table AND deletes transcript files. Type
        <code style={{ marginLeft: 4, marginRight: 4 }}>delete</code>
        to confirm.
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          type="text"
          className="input"
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          placeholder='type "delete"'
        />
        <button
          type="button"
          className="btn-stop"
          disabled={!armed}
          onClick={() => {
            setToast("available in L5 — UI scaffold only");
            setTyped("");
            window.setTimeout(() => setToast(null), 2400);
          }}
        >
          delete history
        </button>
      </div>
      {toast != null && (
        <div
          role="status"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--status-warn)",
          }}
        >
          {toast}
        </div>
      )}
    </div>
  );
}

function DestructiveCard({
  label,
  body,
  confirmLabel,
  kind,
  toast,
  onConfirm,
}: {
  label: string;
  body: string;
  confirmLabel: string;
  kind: "warn" | "danger";
  toast: string | null;
  onConfirm: () => void;
}): React.JSX.Element {
  const borderColor =
    kind === "danger" ? "oklch(0.4 0.1 25)" : "var(--border-mid)";
  const labelColor =
    kind === "danger" ? "var(--status-error)" : "var(--ink-primary)";
  return (
    <div
      style={{
        background: "var(--bg-sunken)",
        border: `1px solid ${borderColor}`,
        borderRadius: "var(--r-md)",
        padding: "var(--sp-3) var(--sp-4)",
        display: "grid",
        gap: 6,
      }}
    >
      <div
        style={{
          fontFamily: "var(--font-ui)",
          fontSize: "13px",
          fontWeight: 500,
          color: labelColor,
        }}
      >
        {label}
      </div>
      <p
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          margin: 0,
        }}
      >
        {body}
      </p>
      <div>
        <button type="button" className="btn" onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
      {toast != null && (
        <div
          role="status"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--status-warn)",
          }}
        >
          {toast}
        </div>
      )}
    </div>
  );
}
