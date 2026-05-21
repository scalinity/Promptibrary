// SCA-934 — Assistant defaults: pattern + model.
//
// Mirrors `defaults-settings.tsx`'s edit pattern. Each dropdown change
// sends the full current settings with one field overwritten; the Rust
// handler returns the merged settings and the query refresh re-renders
// from canonical state.

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import { updateSettings } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";
import { Dropdown, type DropdownOption } from "@/shared/ui/dropdown";
import type { ClaudeModelId } from "@/shared/types/enums";
import type {
  AppSettings,
  AssistantPattern,
} from "@/shared/types/settings";

const PATTERN_OPTIONS: DropdownOption<AssistantPattern>[] = [
  { value: "improve_prompt", label: "Improve Prompt" },
  { value: "improve_prompt_xml", label: "Improve Prompt XML" },
  { value: "improve_writing", label: "Improve Writing" },
];

const MODEL_OPTIONS: DropdownOption<ClaudeModelId>[] = [
  { value: "claude-opus-4-7", label: "claude-opus-4-7" },
  { value: "claude-sonnet-4-6", label: "claude-sonnet-4-6" },
  { value: "opus", label: "opus" },
  { value: "sonnet", label: "sonnet" },
];

export function AssistantSettings(): React.JSX.Element {
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
    <section id="assistant" aria-labelledby="assistant-h">
      <div className="section-label" id="assistant-h">
        assistant
      </div>
      <dl className="kv-grid" style={kvGridStyle}>
        <Row label="default pattern">
          <Dropdown
            ariaLabel="assistant default pattern"
            value={local?.assistantDefaultPattern}
            options={PATTERN_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ assistantDefaultPattern: v })}
          />
        </Row>
        <Row label="model">
          <Dropdown
            ariaLabel="assistant model"
            value={local?.assistantModel}
            options={MODEL_OPTIONS}
            disabled={isPending}
            onChange={(v) => patchLocal({ assistantModel: v })}
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
