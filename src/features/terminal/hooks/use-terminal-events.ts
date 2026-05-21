// SCA-918: subscribe to `term:stdout` and `term:finished` Tauri events
// for a given runId and forward stdout chunks (already base64-encoded)
// to a consumer callback. The callback signature deliberately keeps
// xterm.js out of the hook so unit tests can subscribe without
// touching the canvas renderer.

// SCA-918: Tauri event-channel subscription lifecycle is a true
// mount-time external bridge (CLAUDE.md useEffect pattern #5).
// eslint-disable-next-line no-restricted-imports -- pattern #5
import { useEffect, useRef } from "react";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  TermFinishedEvent,
  TermStdoutEvent,
} from "@/shared/api/ipc";
import type { RunId } from "@/shared/types/ids";

export interface TerminalEventHandlers {
  onStdout?: (bytes: Uint8Array) => void;
  onFinished?: (event: TermFinishedEvent) => void;
}

/** Base64-decode in the browser without pulling a polyfill — Tauri's
 *  WebView is always a modern Chromium / WebKit. */
function decodeBase64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function useTerminalEvents(
  runId: RunId | undefined,
  handlers: TerminalEventHandlers,
): void {
  // Keep handlers in a ref so we don't need to re-subscribe on every
  // render — that would lose any events buffered between unsub and
  // re-sub.
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    if (runId == null) return;
    const unlisteners: UnlistenFn[] = [];
    let cancelled = false;

    (async () => {
      const stdoutHandle = await listen<TermStdoutEvent>(
        "term:stdout",
        ({ payload }) => {
          if (payload.runId !== runId) return;
          handlersRef.current.onStdout?.(
            decodeBase64ToBytes(payload.bytesB64),
          );
        },
      );
      const finishedHandle = await listen<TermFinishedEvent>(
        "term:finished",
        ({ payload }) => {
          if (payload.runId !== runId) return;
          handlersRef.current.onFinished?.(payload);
        },
      );
      if (cancelled) {
        stdoutHandle();
        finishedHandle();
      } else {
        unlisteners.push(stdoutHandle, finishedHandle);
      }
    })();

    return () => {
      cancelled = true;
      for (const u of unlisteners) u();
    };
  }, [runId]);
}
