// Default model / permission mode / verifier mode / destination — editable.
//
// Wires the `update_settings` IPC: each dropdown change sends the full
// current settings with one field overwritten. The Rust handler re-reads
// and returns the merged settings; we invalidate the query so the form
// re-renders from the canonical state.

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import { updateSettings } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";
import { Dropdown, type DropdownOption } from "@/shared/ui/dropdown";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  LaunchDestination,
  VerifierMode,
} from "@/shared/types/enums";
import type { AppSettings } from "@/shared/types/settings";

const MODEL_OPTIONS: DropdownOption<ClaudeModelId>[] = [
  { value: "claude-opus-4-7", label: "claude-opus-4-7" },
  { value: "claude-sonnet-4-6", label: "claude-sonnet-4-6" },
  { value: "opus", label: "opus" },
  { value: "sonnet", label: "sonnet" },
];
const PERMISSION_OPTIONS: DropdownOption<ClaudePermissionMode>[] = [
  { value: "default", label: "default" },
  { value: "acceptEdits", label: "accept edits" },
  { value: "plan", label: "plan" },
  { value: "auto", label: "auto" },
  { value: "dontAsk", label: "don't ask" },
  { value: "bypassPermissions", label: "bypass permissions" },
];
const VERIFIER_OPTIONS: DropdownOption<VerifierMode>[] = [
  { value: "off", label: "off" },
  {
    value: "manual_ultrareview_after_run",
    label: "manual ultrareview after run",
  },
];
const DESTINATION_OPTIONS: DropdownOption<LaunchDestination>[] = [
  { value: "claude_code_cli", label: "Claude Code CLI" },
];

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
          <Dropdown
            ariaLabel="default model"
            value={local?.defaultModel}
            options={MODEL_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultModel: v })}
          />
        </Row>
        <Row label="permission mode">
          <Dropdown
            ariaLabel="permission mode"
            value={local?.defaultPermissionMode}
            options={PERMISSION_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultPermissionMode: v })}
          />
        </Row>
        <Row label="verifier mode">
          <Dropdown
            ariaLabel="verifier mode"
            value={local?.defaultVerifierMode}
            options={VERIFIER_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ defaultVerifierMode: v })}
          />
        </Row>
        <Row label="destination">
          <Dropdown
            ariaLabel="destination"
            value={local?.defaultDestination}
            options={DESTINATION_OPTIONS}
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
