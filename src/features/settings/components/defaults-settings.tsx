// Default model / permission mode / verifier mode / destination — editable.
//
// Wires the `update_settings` IPC: each select sends the full current
// settings with one field overwritten. The Rust handler re-reads and
// returns the merged settings; we invalidate the query so the form
// re-renders from the canonical state (handles any default-fill).

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import { updateSettings } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";
import type { AppSettings } from "@/shared/types/settings";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  LaunchDestination,
  VerifierMode,
} from "@/shared/types/enums";

const MODEL_OPTIONS: ClaudeModelId[] = [
  "claude-opus-4-7",
  "claude-sonnet-4-6",
  "opus",
  "sonnet",
];
const PERMISSION_OPTIONS: ClaudePermissionMode[] = [
  "default",
  "acceptEdits",
  "plan",
  "auto",
  "dontAsk",
  "bypassPermissions",
];
const VERIFIER_OPTIONS: VerifierMode[] = ["off", "manual_ultrareview_after_run"];
const DESTINATION_OPTIONS: LaunchDestination[] = ["claude_code_cli"];

const PERMISSION_LABELS: Record<ClaudePermissionMode, string> = {
  default: "default",
  acceptEdits: "accept edits",
  plan: "plan",
  auto: "auto",
  dontAsk: "don't ask",
  bypassPermissions: "bypass permissions",
};
const VERIFIER_LABELS: Record<VerifierMode, string> = {
  off: "off",
  manual_ultrareview_after_run: "manual ultrareview after run",
};
const DESTINATION_LABELS: Record<LaunchDestination, string> = {
  claude_code_cli: "Claude Code CLI",
};

export function DefaultsSettings(): React.JSX.Element {
  const settings = useSettings();
  const queryClient = useQueryClient();
  const update = useMutation({
    mutationFn: (next: AppSettings) => updateSettings({ settings: next }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: settingsKeys.all() });
    },
  });

  const current = settings.data;
  const local = current?.local;
  const isPending = update.isPending || settings.isLoading;

  const patchLocal = (patch: Partial<NonNullable<typeof local>>): void => {
    if (current == null || local == null) return;
    update.mutate({
      ...current,
      local: { ...local, ...patch },
    });
  };

  return (
    <section id="defaults" aria-labelledby="defaults-h">
      <div className="section-label" id="defaults-h">
        defaults
      </div>
      <dl className="kv-grid" style={kvGridStyle}>
        <Row label="default model">
          <Select
            value={local?.defaultModel}
            options={MODEL_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultModel: v })}
          />
        </Row>
        <Row label="permission mode">
          <Select
            value={local?.defaultPermissionMode}
            options={PERMISSION_OPTIONS}
            labels={PERMISSION_LABELS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultPermissionMode: v })}
          />
        </Row>
        <Row label="verifier mode">
          <Select
            value={local?.defaultVerifierMode}
            options={VERIFIER_OPTIONS}
            labels={VERIFIER_LABELS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultVerifierMode: v })}
          />
        </Row>
        <Row label="destination">
          <Select
            value={local?.defaultDestination}
            options={DESTINATION_OPTIONS}
            labels={DESTINATION_LABELS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultDestination: v })}
          />
        </Row>
      </dl>
    </section>
  );
}

const kvGridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "max-content 1fr",
  rowGap: 8,
  columnGap: 16,
  marginTop: 8,
};

function Row({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <>
      <dt
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
          alignSelf: "center",
        }}
      >
        {label}
      </dt>
      <dd style={{ margin: 0 }}>{children}</dd>
    </>
  );
}

function Select<T extends string>({
  value,
  options,
  labels,
  disabled,
  onChange,
}: {
  value: T | undefined;
  options: readonly T[];
  labels?: Record<T, string>;
  disabled?: boolean;
  onChange: (v: T) => void;
}): React.JSX.Element {
  return (
    <select
      value={value ?? ""}
      disabled={disabled || value == null}
      onChange={(e) => onChange(e.target.value as T)}
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: "12.5px",
        color: "var(--ink-primary)",
        background: "var(--bg-sunken)",
        border: "1px solid var(--border-subtle)",
        borderRadius: "var(--r-sm)",
        padding: "4px 8px",
        cursor: disabled ? "not-allowed" : "pointer",
        appearance: "auto",
      }}
    >
      {options.map((opt) => (
        <option key={opt} value={opt}>
          {labels?.[opt] ?? opt}
        </option>
      ))}
    </select>
  );
}
