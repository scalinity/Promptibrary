// Route: `/run/:runId` — terminal pane.
//
// SCA-918 (B8): the L0 scaffold "Terminal activates in L3" placeholder
// is replaced with the real implementation now that L3 Rust + L5
// runs::* IPC have both landed. While the run is in a non-terminal
// state we show the live TerminalPane (xterm.js attached to the PTY
// event stream). When it finishes, we swap to AnsiTranscript reading
// the persisted vault transcript.

import { useParams } from "react-router-dom";

import { EmptyState } from "@/shared/ui/empty-state";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";
import { defaultMessage, isAppError } from "@/shared/api/errors";
import { asRunId } from "@/shared/types/ids";

import { AnsiTranscript } from "../components/ansi-transcript";
import { TerminalPane } from "../components/terminal-pane";
import { useRun } from "../hooks/use-run";
import { useTranscript } from "../hooks/use-transcript";

function isTerminalStatus(status: string | undefined): boolean {
  // Canonical RunStatus from SCA-910.
  return status === "finished" || status === "errored";
}

export function RunRoute(): React.JSX.Element {
  const { runId: raw } = useParams<{ runId: string }>();
  const runId = raw && raw !== "latest" ? asRunId(raw) : undefined;
  const runQuery = useRun(runId);

  if (runId == null) {
    return (
      <section className="detail-pane" aria-label="Run terminal">
        <EmptyState
          glyph="⏵"
          title="No run selected"
          body="Launch a prompt from the library to land here."
        />
      </section>
    );
  }

  if (runQuery.isLoading) {
    return (
      <section
        className="detail-pane"
        aria-label="Run terminal"
        style={{ display: "grid", placeItems: "center" }}
      >
        <LoadingSpinner label="loading run…" />
      </section>
    );
  }

  if (runQuery.isError) {
    const message = isAppError(runQuery.error)
      ? defaultMessage(runQuery.error.kind)
      : "Could not load this run.";
    return (
      <section className="detail-pane" aria-label="Run terminal">
        <EmptyState glyph="⌖" title="Run unavailable" body={message} />
      </section>
    );
  }

  const run = runQuery.data;
  if (run == null) {
    return (
      <section className="detail-pane" aria-label="Run terminal">
        <EmptyState
          glyph="⌖"
          title="Run not found"
          body={`Run ${runId} doesn't exist.`}
        />
      </section>
    );
  }

  const finished = isTerminalStatus(run.status);

  if (finished) {
    return <RunRouteTranscript runId={runId} />;
  }

  return (
    <section
      className="detail-pane"
      aria-label="Run terminal"
      style={{ display: "flex", flexDirection: "column" }}
    >
      <TerminalPane
        runId={runId}
        onFinished={() => {
          // Invalidate the run query so the route flips to the
          // transcript path on the next tick.
          runQuery.refetch();
        }}
      />
    </section>
  );
}

function RunRouteTranscript({ runId }: { runId: ReturnType<typeof asRunId> }): React.JSX.Element {
  const transcript = useTranscript(runId);
  if (transcript.isLoading) {
    return (
      <section
        className="detail-pane"
        aria-label="Run transcript"
        style={{ display: "grid", placeItems: "center" }}
      >
        <LoadingSpinner label="loading transcript…" />
      </section>
    );
  }
  if (transcript.isError || transcript.data == null) {
    const message = isAppError(transcript.error)
      ? defaultMessage(transcript.error.kind)
      : "Transcript unavailable.";
    return (
      <section className="detail-pane" aria-label="Run transcript">
        <EmptyState glyph="∿" title="No transcript" body={message} />
      </section>
    );
  }
  return (
    <section
      className="detail-pane"
      aria-label="Run transcript"
      style={{ display: "flex", flexDirection: "column" }}
    >
      <AnsiTranscript content={transcript.data.content} />
    </section>
  );
}
