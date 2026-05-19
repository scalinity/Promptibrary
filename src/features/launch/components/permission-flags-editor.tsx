// Permission overrides for the launch.
//
// L2 surface is a compact summary + dropdown; the full advanced rule editor
// is V2 polish. The `mode` field maps to claude's `--permission-mode`.

import type { ClaudePermissionMode } from "@/shared/types/enums";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { Prompt } from "@/shared/types/prompt";

const MODE_LABELS: Record<ClaudePermissionMode, string> = {
  default: "default — ask each tool",
  acceptEdits: "accept edits — auto-approve writes",
  plan: "plan — read only",
  auto: "auto — accept everything",
  dontAsk: "don't ask — silent allow",
  bypassPermissions: "bypass — danger zone",
};

interface Props {
  prompt: Prompt;
}

export function PermissionFlagsEditor({ prompt }: Props): React.JSX.Element {
  const overrides = useLaunchDraftStore((s) => s.overrides);
  const setOverride = useLaunchDraftStore((s) => s.setOverride);
  const mode = overrides.permissionMode ?? prompt.launchDefaults.permissionMode;

  return (
    <section>
      <div className="section-label">permissions</div>
      <div className="select-wrap">
        <select
          className="select"
          value={mode}
          onChange={(e) =>
            setOverride("permissionMode", e.target.value as ClaudePermissionMode)
          }
        >
          {(Object.keys(MODE_LABELS) as ClaudePermissionMode[]).map((m) => (
            <option key={m} value={m}>
              {MODE_LABELS[m]}
            </option>
          ))}
        </select>
      </div>
    </section>
  );
}
