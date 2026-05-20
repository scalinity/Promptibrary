// Launch button — the ONE drop-shadowed element per the design system.
//
// Wired to `start_launch` IPC; in L2 the IPC returns `not_yet_implemented`,
// which surfaces as a toast-like inline error below the button. L3 fills it.

import { useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useStartLaunch } from "@/features/launch/hooks/use-start-launch";
import type { LaunchValidation } from "@/features/launch/hooks/use-launch-validation";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { Prompt } from "@/shared/types/prompt";
import { isAppError } from "@/shared/api/errors";

interface Props {
  prompt: Prompt;
  validation: LaunchValidation;
}

export function LaunchButton({ prompt, validation }: Props): React.JSX.Element {
  const overrides = useLaunchDraftStore((s) => s.overrides);
  const values = useLaunchDraftStore((s) => s.values);
  const inlineTweak = useLaunchDraftStore((s) => s.inlineTweakBody);
  const startLaunch = useStartLaunch();
  const [error, setError] = useState<string | null>(null);

  // SCA-633 — resolve the effective working directory once and treat null
  // as a hard block, replacing the L2-foundation `("" as never)` cast that
  // let an empty path reach Rust.
  const workingDirectory =
    overrides.workingDirectory ?? prompt.launchDefaults.workingDirectory;
  const disabled =
    !validation.ready || workingDirectory == null || startLaunch.isPending;

  const onLaunch = () => {
    if (workingDirectory == null) {
      setError("Pick a working directory before launching.");
      return;
    }
    setError(null);
    // Map the in-drawer overrides bag onto the per-field StartLaunchArgs
    // shape Rust expects. `workingDirectory` is its own required field
    // and `destination` / `maxTurns` are not part of the IPC contract
    // (destination is a constant ClaudeCodeCli; max_turns is intentionally
    // forward-compat-only per spec §7), so we don't forward them.
    const {
      destination: _destination,
      workingDirectory: _wd,
      maxTurns: _maxTurns,
      ...overrideFields
    } = overrides;
    void _destination;
    void _wd;
    void _maxTurns;
    startLaunch
      .mutateAsync({
        promptId: prompt.id,
        values: Object.values(values),
        workingDirectory,
        inlineTweakBody: inlineTweak,
        ...overrideFields,
      })
      .catch((err: unknown) => {
        if (isAppError(err) && err.message === "not_yet_implemented") {
          setError("Launch pipeline activates in L3.");
        } else if (isAppError(err)) {
          setError(err.message);
        } else {
          setError("Launch failed");
        }
      });
  };

  useHotkeys(
    "meta+enter, ctrl+enter",
    (e) => {
      e.preventDefault();
      if (!disabled) onLaunch();
    },
    { enableOnFormTags: true },
  );

  return (
    <div style={{ display: "grid", gap: 8 }}>
      <button
        type="button"
        className="btn-launch"
        onClick={onLaunch}
        disabled={disabled}
        aria-label="Launch run"
      >
        Launch <span className="kbd-inline">⌘↵</span>
      </button>
      {error != null && (
        <div
          role="alert"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--status-warn)",
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}
