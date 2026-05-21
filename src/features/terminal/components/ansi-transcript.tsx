// SCA-918: read-only ANSI viewer for completed runs. Uses xterm.js
// non-interactively so the same SGR + OSC 8 sequences render
// identically to the live pane. No stdin wiring, no resize push.

// SCA-918: write into the xterm host on content change is a DOM
// bridge (CLAUDE.md useEffect pattern #5).
// eslint-disable-next-line no-restricted-imports -- pattern #5
import { useEffect } from "react";

import { useXterm } from "../hooks/use-xterm";

export interface AnsiTranscriptProps {
  /** Raw transcript bytes from fetch_transcript IPC. ANSI sequences
   *  are preserved verbatim. */
  content: string;
}

export function AnsiTranscript({
  content,
}: AnsiTranscriptProps): React.JSX.Element {
  const { containerRef, getTerminal, fit } = useXterm();

  useEffect(() => {
    const term = getTerminal();
    if (term == null) return;
    term.reset();
    term.write(content);
    fit();
  }, [content, fit, getTerminal]);

  return (
    <div
      ref={containerRef}
      data-testid="ansi-transcript"
      style={{
        flex: 1,
        width: "100%",
        minHeight: "300px",
        background: "var(--bg-deep)",
        padding: "8px",
      }}
    />
  );
}
