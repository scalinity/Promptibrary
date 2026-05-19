// Extraction model toggle + candidate count.
//
// Per spec §13 (patched) the standard / deep model identifiers are
// application-level constants, not user-tunable.  L2 surface displays them
// read-only.

import {
  EXTRACTION_MODEL,
  DEEP_EXTRACTION_MODEL,
} from "@/shared/types/settings";

export function ExtractionSettings(): React.JSX.Element {
  return (
    <section id="extraction" aria-labelledby="extraction-h">
      <div className="section-label" id="extraction-h">
        extraction
      </div>
      <div style={{ display: "grid", gap: "var(--sp-3)", marginTop: 8 }}>
        <Row label="standard model" value={EXTRACTION_MODEL} />
        <Row label="deep model" value={DEEP_EXTRACTION_MODEL} />
        <Row label="source-cap (standard)" value="60,000 chars" />
        <Row label="source-cap (deep)" value="160,000 chars" />
      </div>
      <p
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          marginTop: 16,
        }}
      >
        Per spec §13 (patched), extraction model is an application-level
        constant for V1, not user-tunable.
      </p>
    </section>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "var(--sp-3) var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {label}
      </span>
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "12.5px",
          color: "var(--ink-primary)",
        }}
      >
        {value}
      </span>
    </div>
  );
}
