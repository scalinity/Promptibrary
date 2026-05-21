// Extraction model + source-cap settings — editable per SCA-906.
//
// Previously these were hard-coded application-level constants. After
// SCA-906 they're real settings fields persisted in settings.json and
// read by extract_prompt_candidates at IPC time.

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import { updateSettings } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";
import { Dropdown, type DropdownOption } from "@/shared/ui/dropdown";
import type { ClaudeModelId } from "@/shared/types/enums";
import type { AppSettings } from "@/shared/types/settings";

const MODEL_OPTIONS: DropdownOption<ClaudeModelId>[] = [
  { value: "claude-opus-4-7", label: "claude-opus-4-7" },
  { value: "claude-sonnet-4-6", label: "claude-sonnet-4-6" },
  { value: "opus", label: "opus" },
  { value: "sonnet", label: "sonnet" },
];

const CAP_MIN = 1_000;
const CAP_MAX = 1_000_000;

export function ExtractionSettings(): React.JSX.Element {
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
    <section id="extraction" aria-labelledby="extraction-h">
      <div className="section-label" id="extraction-h">
        extraction
      </div>
      <div style={{ display: "grid", gap: "var(--sp-3)", marginTop: 8 }}>
        <Row label="standard model">
          <Dropdown
            ariaLabel="extraction model"
            value={local?.extractionModel}
            options={MODEL_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ extractionModel: v })}
          />
        </Row>
        <Row label="deep model">
          <Dropdown
            ariaLabel="deep extraction model"
            value={local?.deepExtractionModel}
            options={MODEL_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ deepExtractionModel: v })}
          />
        </Row>
        <Row label="source-cap (standard)">
          <CapInput
            value={local?.sourceCapStandard}
            disabled={isPending}
            onCommit={(v) => patchLocal({ sourceCapStandard: v })}
          />
        </Row>
        <Row label="source-cap (deep)">
          <CapInput
            value={local?.sourceCapDeep}
            disabled={isPending}
            onCommit={(v) => patchLocal({ sourceCapDeep: v })}
          />
        </Row>
      </div>
    </section>
  );
}

function Row({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "var(--sp-3) var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {label}
      </span>
      {children}
    </div>
  );
}

// Numeric input that commits on blur / Enter so we don't fire a
// settings write on every keystroke. Clamped to [CAP_MIN, CAP_MAX].
function CapInput({
  value,
  disabled,
  onCommit,
}: {
  value: number | undefined;
  disabled: boolean;
  onCommit: (v: number) => void;
}): React.JSX.Element {
  const commit = (raw: string): void => {
    const n = Number.parseInt(raw.replace(/[, ]/g, ""), 10);
    if (Number.isNaN(n)) return;
    const clamped = Math.min(CAP_MAX, Math.max(CAP_MIN, n));
    if (clamped !== value) onCommit(clamped);
  };
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "baseline",
        gap: 4,
      }}
    >
      <input
        type="text"
        inputMode="numeric"
        defaultValue={value?.toLocaleString("en-US") ?? ""}
        // `key` forces remount when the canonical value changes so the
        // input picks up server-side clamping or external edits without
        // becoming a controlled-input state-sync nightmare.
        key={value}
        disabled={disabled}
        onBlur={(e) => commit(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commit(e.currentTarget.value);
            e.currentTarget.blur();
          }
        }}
        style={{
          width: "8ch",
          background: "transparent",
          border: "none",
          outline: "none",
          textAlign: "right",
          fontFamily: "var(--font-mono)",
          fontSize: "12.5px",
          color: "var(--ink-primary)",
        }}
        aria-label="source cap (chars)"
      />
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
        }}
      >
        chars
      </span>
    </span>
  );
}
