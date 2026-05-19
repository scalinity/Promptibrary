// Failure panel — one tailored variant per `ExtractionFailure.kind`
// (spec §6 *Failure modes*). Paywall and TranscriptUnavailable surface
// a manual-paste affordance the user can submit to continue.

import { useState } from "react";

import type { ExtractionFailure } from "@/shared/api/ipc";

interface Props {
  failure: ExtractionFailure;
  onRetry?: () => void;
  onReset: () => void;
  onManualPaste?: (text: string) => void;
}

export function ImportFailurePanel({
  failure,
  onRetry,
  onReset,
  onManualPaste,
}: Props): React.JSX.Element {
  const meta = describe(failure);
  const [pasted, setPasted] = useState("");

  return (
    <section
      role="alert"
      data-testid={`import-failure-${failure.kind}`}
      style={{
        display: "grid",
        gap: 12,
        padding: "var(--sp-4)",
        background: "var(--bg-sunken)",
        border: `1px solid ${meta.borderColor}`,
        borderRadius: "var(--r-md)",
      }}
    >
      <header style={{ display: "grid", gap: 4 }}>
        <span
          className="section-label"
          style={{ color: meta.borderColor, padding: 0 }}
        >
          {meta.label}
        </span>
        <h2
          style={{
            margin: 0,
            fontFamily: "var(--font-display)",
            fontWeight: 500,
            fontSize: 16,
            color: "var(--ink-primary)",
          }}
        >
          {meta.title}
        </h2>
        <p
          style={{
            margin: 0,
            fontFamily: "var(--font-ui)",
            fontSize: 12,
            color: "var(--ink-secondary)",
            lineHeight: 1.5,
          }}
        >
          {meta.body}
        </p>
      </header>

      {meta.hint ? (
        <pre
          style={{
            margin: 0,
            padding: "var(--sp-2) var(--sp-3)",
            background: "var(--bg-base)",
            border: "var(--hairline)",
            borderRadius: "var(--r-xs)",
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--ink-secondary)",
            whiteSpace: "pre-wrap",
          }}
        >
          {meta.hint}
        </pre>
      ) : null}

      {meta.showManualPaste && onManualPaste ? (
        <div style={{ display: "grid", gap: 6 }}>
          <label className="section-label" htmlFor="import-manual-paste" style={{ padding: 0 }}>
            paste content manually
          </label>
          <textarea
            id="import-manual-paste"
            className="input"
            rows={6}
            value={pasted}
            placeholder="paste article text or transcript…"
            onChange={(e) => setPasted(e.target.value)}
            style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}
          />
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button
              type="button"
              className="btn"
              disabled={pasted.trim().length === 0}
              onClick={() => onManualPaste(pasted.trim())}
            >
              continue with pasted text
            </button>
          </div>
        </div>
      ) : null}

      <footer style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        {onRetry ? (
          <button type="button" className="btn" onClick={onRetry}>
            retry
          </button>
        ) : null}
        <button type="button" className="btn" onClick={onReset}>
          start over
        </button>
      </footer>
    </section>
  );
}

function describe(failure: ExtractionFailure): {
  label: string;
  title: string;
  body: string;
  hint: string | null;
  showManualPaste: boolean;
  borderColor: string;
} {
  switch (failure.kind) {
    case "network_unavailable":
      return {
        label: "network",
        title: "Couldn't reach the source",
        body: "Check your connection and try again. The URL you entered is preserved.",
        hint: failure.message,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "unsupported_source":
      return {
        label: "unsupported",
        title: "This URL isn't supported yet",
        body: "Promptibrary recognizes YouTube, X/Twitter, and arbitrary articles. For anything else, paste the content manually.",
        hint: `reason: ${failure.reason}`,
        showManualPaste: true,
        borderColor: "var(--ink-tertiary)",
      };
    case "dependency_missing":
      return {
        label: "missing tool",
        title: `${failure.name} not installed`,
        body: "Install the dependency to continue.",
        hint: failure.installHint,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "transcript_unavailable":
      return {
        label: "no transcript",
        title: "No English transcript for this video",
        body: "Paste a transcript manually to keep going, or pick a different video.",
        hint: null,
        showManualPaste: true,
        borderColor: "var(--ink-tertiary)",
      };
    case "paywall_likely":
      return {
        label: "paywall",
        title: "This article looks paywalled",
        body: "Only a short preview was reachable. Paste the full article text below to continue.",
        hint: failure.preview,
        showManualPaste: true,
        borderColor: "var(--ink-tertiary)",
      };
    case "rate_limited":
      return {
        label: "rate limited",
        title: `${failure.provider} is rate-limiting requests`,
        body: failure.resetAt
          ? `Try again after ${new Date(failure.resetAt).toLocaleString()}.`
          : "Wait a moment and try again.",
        hint: null,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "anthropic_key_missing":
      return {
        label: "no api key",
        title: "Anthropic API key required",
        body: "Add your Anthropic API key in Settings → Secrets, then retry.",
        hint: null,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "anthropic_auth_invalid":
      return {
        label: "auth invalid",
        title: "Anthropic rejected the API key",
        body: "Update the key in Settings → Secrets and retry. The previous key has not been touched.",
        hint: null,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "llm_refusal":
      return {
        label: "model declined",
        title: "The model declined to extract this source",
        body: "Promptibrary won't save anything from this refusal. Try a different source.",
        hint: failure.excerpt,
        showManualPaste: false,
        borderColor: "var(--ink-tertiary)",
      };
    case "malformed_model_output":
      return {
        label: "malformed",
        title: "The model returned invalid JSON",
        body: "Promptibrary already attempted one repair. The raw output is shown below for debugging.",
        hint: failure.raw.slice(0, 600),
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
    case "extraction_failed":
      return {
        label: "extraction failed",
        title: "Extraction failed",
        body: failure.reason,
        hint: null,
        showManualPaste: false,
        borderColor: "var(--status-warn)",
      };
  }
}
