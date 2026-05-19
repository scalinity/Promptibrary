// Route: `/run/:runId` — L2 placeholder.
//
// The live terminal + xterm integration + PTY pipeline land in L3.
// The L2 surface renders an empty-state explaining what's coming so the
// route navigates cleanly from cmdk + library + launch.

import { useParams } from "react-router-dom";

import { EmptyState } from "@/shared/ui/empty-state";

export function RunRoute(): React.JSX.Element {
  const { runId } = useParams<{ runId: string }>();
  return (
    <section className="detail-pane" aria-label="Run terminal">
      <EmptyState
        glyph="⏵"
        title="Terminal activates in L3"
        body={
          runId === "latest"
            ? "Launching a run will land here once L3 wires the PTY pipeline."
            : `No transcript stored for run ${runId}. The launch pipeline is L3.`
        }
      />
    </section>
  );
}
