// Bottom 30px chrome strip — matches `.statusbar` in the design system.
//
// Reads from the global status store (vault path, model, prompt count, last
// run). The store is populated by feature surfaces — L2 wires the static
// fields; live fields (run state, index progress) land in L3/L5.

import { useStatusLine } from "./use-status-line";

export function StatusLine(): React.JSX.Element {
  const status = useStatusLine();
  return (
    <div className="statusbar" role="status" aria-label="Status bar">
      <span
        className="dot"
        data-state={status.state.toLowerCase()}
        aria-hidden="true"
      />
      <span className="state">{status.state}</span>
      {status.vaultPath != null && (
        <span className="pair">
          <span className="k">vault</span>
          <span className="v">{status.vaultPath}</span>
        </span>
      )}
      <span className="pair">
        <span className="k">model</span>
        <span className="v">{status.model}</span>
      </span>
      <span className="pair">
        <span className="k">prompts</span>
        <span className="v">{status.promptCount}</span>
      </span>
      <span className="right">
        {status.lastRunLabel != null && (
          <>
            <span className="pair">
              <span className="k">last run</span>
              <span className="v">{status.lastRunLabel}</span>
            </span>
            <span className="sep">·</span>
          </>
        )}
        <span className="kbd">⌘K</span> search
        <span className="sep">·</span>
        <span className="kbd">⌘N</span> new
      </span>
    </div>
  );
}
