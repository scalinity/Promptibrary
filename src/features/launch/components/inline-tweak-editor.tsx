// "Tweak this launch" — ephemeral body edit that does NOT mutate the saved
// prompt. State lives in the launch draft store; toggling off reverts to the
// saved body. Toggling on seeds with the saved body and re-parses variables
// against the tweaked text on every change.

import { PromptBodyEditor } from "@/features/prompt/components/prompt-body-editor";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
}

export function InlineTweakEditor({ prompt }: Props): React.JSX.Element {
  const tweak = useLaunchDraftStore((s) => s.inlineTweakBody);
  const setTweak = useLaunchDraftStore((s) => s.setInlineTweak);
  const active = tweak != null;

  return (
    <section>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <span className="section-label">tweak for this launch</span>
        <label
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 8,
            cursor: "pointer",
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--ink-tertiary)",
          }}
        >
          <input
            type="checkbox"
            checked={active}
            onChange={(e) =>
              setTweak(e.target.checked ? prompt.body : null)
            }
            style={{ accentColor: "var(--accent)" }}
          />
          {active ? "tweaking" : "use saved prompt"}
        </label>
      </div>
      {active && (
        <PromptBodyEditor
          promptId={prompt.id}
          value={tweak ?? ""}
          onChange={setTweak}
        />
      )}
    </section>
  );
}
