// Git history panel for the prompt file.
//
// L5 fills this with real commits; L2 surface renders a placeholder so the
// toggle button has somewhere to land.

import type { Prompt } from "@/shared/types/prompt";
import { EmptyState } from "@/shared/ui/empty-state";

interface Props {
  prompt: Prompt;
}

export function PromptHistoryPanel({ prompt: _prompt }: Props): React.JSX.Element {
  return (
    <div
      style={{
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <EmptyState
        glyph="⌚"
        title="Version history activates in L5"
        body="Git-backed history, diff view, and revert land alongside the search + telemetry surfaces."
      />
    </div>
  );
}
