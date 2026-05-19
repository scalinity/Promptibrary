// Working directory picker — native dialog via tauri-plugin-dialog.
//
// Updates the launch draft store. Validation against the Rust side
// (`validate_launch_inputs`) lands in L3; for L2 the picker only enforces
// that the path is non-empty.

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import {
  asAbsolutePath,
  type AbsolutePath,
} from "@/shared/types/ids";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
}

export function WorkingDirPicker({ prompt }: Props): React.JSX.Element {
  const overrides = useLaunchDraftStore((s) => s.overrides);
  const setOverride = useLaunchDraftStore((s) => s.setOverride);
  const value: AbsolutePath | null =
    overrides.workingDirectory ?? prompt.launchDefaults.workingDirectory;

  return (
    <section>
      <div className="section-label">working directory</div>
      <button
        type="button"
        className={value == null ? "picker empty" : "picker"}
        onClick={async () => {
          const result = await openDialog({ directory: true, multiple: false });
          if (typeof result === "string") {
            setOverride("workingDirectory", asAbsolutePath(result));
          }
        }}
      >
        <span className="glyph">⊟</span>
        <span className="path">{value ?? "choose a directory…"}</span>
      </button>
    </section>
  );
}
