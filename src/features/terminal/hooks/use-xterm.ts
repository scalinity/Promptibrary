// SCA-918: xterm.js Terminal lifecycle hook. Owns the Terminal
// instance, addon registration, and theme synchronization. The
// caller passes the host <div> via ref; this hook attaches xterm
// to it on mount and disposes on unmount.

// SCA-918: xterm.js attach is a DOM-level side effect — pattern #5
// in CLAUDE.md's useEffect Replacement Patterns.
// eslint-disable-next-line no-restricted-imports -- pattern #5
import { useEffect, useRef } from "react";

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";

import "@xterm/xterm/css/xterm.css";

/** Read the dark-carbon theme tokens from CSS custom properties so
 *  the terminal stays in lockstep with `tokens.css`. */
function readTerminalTheme(): {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
} {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string, fallback: string) =>
    cs.getPropertyValue(name).trim() || fallback;
  return {
    background: v("--bg-deep", "#0c0c0c"),
    foreground: v("--ink-primary", "#e7e7e7"),
    cursor: v("--accent", "#f59e0b"),
    selectionBackground: v("--accent-glow", "rgba(245,158,11,0.3)"),
  };
}

export interface UseXtermResult {
  /** Ref to attach to the host <div>. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Stable getter for the Terminal — null until first attach. */
  getTerminal: () => Terminal | null;
  /** Fit the terminal to its container. Call after layout changes. */
  fit: () => void;
}

export function useXterm(): UseXtermResult {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (containerRef.current == null) return;

    const term = new Terminal({
      fontFamily: 'var(--font-mono), "JetBrains Mono", monospace',
      fontSize: 12.5,
      lineHeight: 1.4,
      cursorBlink: true,
      // SCA-918: 24-bit color SGR + OSC 8 hyperlinks come through the
      // PTY pipeline preserved (spec §7). xterm.js renders both
      // natively when allowTransparency is enabled.
      allowTransparency: true,
      scrollback: 5000,
      theme: readTerminalTheme(),
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";

    term.open(containerRef.current);
    fitAddon.fit();

    terminalRef.current = term;
    fitAddonRef.current = fitAddon;

    return () => {
      term.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, []);

  return {
    containerRef,
    getTerminal: () => terminalRef.current,
    fit: () => fitAddonRef.current?.fit(),
  };
}
