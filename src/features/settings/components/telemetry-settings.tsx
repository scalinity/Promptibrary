// Telemetry toggle + the two distinct destructive actions per spec §13.
//
// Critically, these two surfaces stay separate — bundling them into one
// "danger zone" button was the V1 mistake the patch reversed.
//   • Clear telemetry cache: single-click confirmation, drops aggregates,
//     keeps runs + transcripts.
//   • Delete all run history: typed "delete" confirmation, drops the runs
//     table AND deletes transcript files.

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import {
  clearTelemetryCache,
  deleteAllRunHistory,
  updateSettings,
  type ClearTelemetryCacheResult,
  type DeleteAllRunHistoryResult,
} from "@/shared/api/ipc";
import { runKeys, settingsKeys } from "@/shared/api/queryKeys";
import type { AppSettings } from "@/shared/types/settings";
import { useToast } from "@/shared/ui/use-toast";

export function TelemetrySettings(): React.JSX.Element {
  const settings = useSettings();
  const queryClient = useQueryClient();
  const current = settings.data;
  const enabled = current?.local.telemetryEnabled ?? false;
  const update = useMutation({
    mutationFn: (next: AppSettings) => updateSettings({ settings: next }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: settingsKeys.all() });
    },
  });

  const toggleTelemetry = (): void => {
    if (current == null) return;
    update.mutate({
      ...current,
      local: { ...current.local, telemetryEnabled: !enabled },
    });
  };

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
          cursor: current == null ? "not-allowed" : "pointer",
        }}
      >
        <input
          type="checkbox"
          checked={enabled}
          disabled={current == null || update.isPending}
          onChange={toggleTelemetry}
          style={{ accentColor: "var(--accent)" }}
        />
        record run telemetry locally (count, duration, exit code)
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
  const queryClient = useQueryClient();
  const { toast, showToast } = useToast(2400);
  const mutation = useMutation({
    mutationFn: () => clearTelemetryCache(),
    onSuccess: (result: ClearTelemetryCacheResult) => {
      queryClient.invalidateQueries({ queryKey: runKeys.all() });
      showToast(`cleared ${result.eventsDeleted} telemetry events`);
    },
    onError: (err) => {
      console.error("clearTelemetryCache failed:", err);
      showToast("clear failed; see console");
    },
  });
  return (
    <DestructiveCard
      label="clear telemetry cache"
      body="Drops aggregated counts. Run records and transcripts are kept."
      confirmLabel={mutation.isPending ? "clearing…" : "clear cache"}
      kind="warn"
      toast={toast}
      disabled={mutation.isPending}
      onConfirm={() => mutation.mutate()}
    />
  );
}

function DeleteHistoryAction(): React.JSX.Element {
  const queryClient = useQueryClient();
  const [typed, setTyped] = useState("");
  const { toast, showToast } = useToast(2800);
  const mutation = useMutation({
    mutationFn: () => deleteAllRunHistory({ confirmation: "delete" }),
    onSuccess: (result: DeleteAllRunHistoryResult) => {
      queryClient.invalidateQueries({ queryKey: runKeys.all() });
      setTyped("");
      showToast(
        `deleted ${result.runsDeleted} runs · ${result.transcriptFilesDeleted} transcripts`,
      );
    },
    onError: (err) => {
      console.error("deleteAllRunHistory failed:", err);
      showToast("delete failed; see console");
    },
  });
  const armed = typed === "delete" && !mutation.isPending;
  return (
    <div
      style={{
        background: "var(--bg-sunken)",
        border: "1px solid var(--border-danger)",
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
          disabled={mutation.isPending}
        />
        <button
          type="button"
          className="btn-stop"
          disabled={!armed}
          onClick={() => mutation.mutate()}
        >
          {mutation.isPending ? "…" : "delete"}
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
  disabled,
  onConfirm,
}: {
  label: string;
  body: string;
  confirmLabel: string;
  kind: "warn" | "danger";
  toast: string | null;
  disabled?: boolean;
  onConfirm: () => void;
}): React.JSX.Element {
  const borderColor =
    kind === "danger" ? "var(--border-danger)" : "var(--border-mid)";
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
        <button
          type="button"
          className="btn"
          disabled={disabled}
          onClick={onConfirm}
        >
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
