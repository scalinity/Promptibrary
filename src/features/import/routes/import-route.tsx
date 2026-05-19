// Route: `/import` — URL field + source-type detection chip.
//
// L4 wires the extraction pipeline. The L2 surface lets the user paste a URL
// and see the detected source kind via `detect_source` (still an L0 stub on
// the Rust side — surfaces the typed error gracefully).

import { useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import {
  detectSource,
  type DetectSourceResult,
} from "@/shared/api/ipc";
import { isAppError } from "@/shared/api/errors";
import { EmptyState } from "@/shared/ui/empty-state";

export function ImportRoute(): React.JSX.Element {
  const [url, setUrl] = useState("");
  const [detected, setDetected] = useState<DetectSourceResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const onDetect = async () => {
    setError(null);
    setPending(true);
    try {
      const result = await detectSource(url.trim());
      setDetected(result);
    } catch (e: unknown) {
      setDetected(null);
      if (isAppError(e)) {
        setError(e.message === "not_yet_implemented"
          ? "URL detection lights up in L4."
          : e.message);
      } else {
        setError("detection failed");
      }
    } finally {
      setPending(false);
    }
  };

  useHotkeys(
    "meta+enter, ctrl+enter",
    (e) => {
      e.preventDefault();
      if (url.trim().length > 0) onDetect();
    },
    { enableOnFormTags: true },
  );

  return (
    <section
      className="detail-pane"
      aria-label="Import"
      style={{ padding: "var(--sp-7) var(--sp-7)", overflow: "auto" }}
    >
      <div style={{ maxWidth: 640, margin: "0 auto", display: "grid", gap: 24 }}>
        <header>
          <div className="section-label">import</div>
          <h1
            style={{
              fontFamily: "var(--font-display)",
              fontWeight: 500,
              fontSize: 26,
              letterSpacing: "-0.015em",
              margin: "4px 0 8px",
            }}
          >
            Pull a prompt from a URL
          </h1>
          <p
            style={{
              fontFamily: "var(--font-ui)",
              fontSize: 13.5,
              color: "var(--ink-secondary)",
              lineHeight: 1.55,
            }}
          >
            YouTube, X/Twitter, or any article. The extractor is the LLM — it
            reads the page and proposes one or more prompt candidates you can
            edit before saving. Extraction lights up in L4.
          </p>
        </header>

        <div style={{ display: "flex", gap: 8 }}>
          <input
            className="input"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://…"
            onKeyDown={(e) => {
              if (e.key === "Enter") onDetect();
            }}
            autoFocus
          />
          <button
            type="button"
            className="btn"
            onClick={onDetect}
            disabled={url.trim().length === 0 || pending}
          >
            {pending ? "detecting…" : "detect"} <span className="kbd">⌘↵</span>
          </button>
        </div>

        {detected != null && (
          <div
            style={{
              padding: "var(--sp-3) var(--sp-4)",
              background: "var(--bg-sunken)",
              border: "var(--hairline)",
              borderRadius: "var(--r-md)",
              display: "flex",
              alignItems: "center",
              gap: 12,
            }}
          >
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 10.5,
                letterSpacing: "0.06em",
                textTransform: "uppercase",
                color: "var(--accent)",
              }}
            >
              {detected.kind}
            </span>
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 12.5,
                color: "var(--ink-secondary)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {detected.url}
            </span>
          </div>
        )}

        {error != null && (
          <div
            role="alert"
            style={{
              padding: "var(--sp-3) var(--sp-4)",
              background: "var(--bg-sunken)",
              border: "1px solid var(--status-warn)",
              borderRadius: "var(--r-md)",
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--status-warn)",
            }}
          >
            {error}
          </div>
        )}

        <EmptyState
          glyph="∿"
          title="Extraction lights up in L4"
          body="Once the LLM extractor lands, candidates appear here for review, editing, and save."
        />
      </div>
    </section>
  );
}
