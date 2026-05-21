// SCA-918: live xterm.js terminal pane. Mounts the Terminal via
// useXterm, subscribes to term:stdout / term:finished, forwards user
// keystrokes to the PTY via send_terminal_input, and resyncs the PTY
// dimensions on container resize.

// SCA-918: xterm.js stdin subscription + ResizeObserver attach are
// mount-time DOM bridges (CLAUDE.md useEffect pattern #5).
// eslint-disable-next-line no-restricted-imports -- pattern #5
import { useEffect, useRef } from "react";

import {
  resizeTerminal,
  sendTerminalInput,
  type TermFinishedEvent,
} from "@/shared/api/ipc";
import { defaultMessage, isAppError } from "@/shared/api/errors";
import type { RunId } from "@/shared/types/ids";

import { useXterm } from "../hooks/use-xterm";
import { useTerminalEvents } from "../hooks/use-terminal-events";

export interface TerminalPaneProps {
  runId: RunId;
  /** Called when the backend emits term:finished. */
  onFinished?: (event: TermFinishedEvent) => void;
}

export function TerminalPane({
  runId,
  onFinished,
}: TerminalPaneProps): React.JSX.Element {
  const { containerRef, getTerminal, fit } = useXterm();
  const utf8Decoder = useRef(new TextDecoder("utf-8", { fatal: false }));
  const utf8Encoder = useRef(new TextEncoder());

  // Wire stdin: every keystroke xterm sees gets forwarded to the PTY.
  useEffect(() => {
    const term = getTerminal();
    if (term == null) return;
    const sub = term.onData((data) => {
      // Tauri's IPC encodes strings as UTF-8; we send the raw stdin
      // bytes verbatim. send_terminal_input on the Rust side
      // forwards into the PTY master.
      const _bytes = utf8Encoder.current.encode(data);
      sendTerminalInput({ runId, bytes: data }).catch((err) => {
        console.error("send_terminal_input failed:", err);
      });
      void _bytes;
    });
    return () => sub.dispose();
  }, [runId, getTerminal]);

  // Wire stdout: backend pushes term:stdout, we write into xterm.
  useTerminalEvents(runId, {
    onStdout: (bytes) => {
      const term = getTerminal();
      if (term == null) return;
      // xterm.write accepts a string OR Uint8Array. Strings get
      // re-encoded — we have raw PTY bytes, decode once via TextDecoder
      // and let xterm handle ANSI parsing.
      term.write(utf8Decoder.current.decode(bytes, { stream: true }));
    },
    onFinished: (ev) => onFinished?.(ev),
  });

  // Resize: ResizeObserver on the container → fit() + push cols/rows
  // to the PTY master so claude lays out output correctly.
  useEffect(() => {
    const term = getTerminal();
    if (term == null || containerRef.current == null) return;
    const observer = new ResizeObserver(() => {
      fit();
      const cols = term.cols;
      const rows = term.rows;
      resizeTerminal({ runId, cols, rows }).catch((err) => {
        // Resize failures are not fatal; log only.
        if (isAppError(err)) {
          console.warn("resizeTerminal:", defaultMessage(err.kind));
        }
      });
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [containerRef, fit, getTerminal, runId]);

  return (
    <div
      ref={containerRef}
      data-testid="terminal-pane"
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
