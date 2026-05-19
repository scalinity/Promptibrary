// Standard (Sonnet 4.6) / Deep (Opus 4.7) extraction mode toggle.

import type { ExtractionMode } from "@/shared/api/ipc";
import { useImportStore } from "@/features/import/stores/import-store";

const OPTIONS: { value: ExtractionMode; label: string; hint: string }[] = [
  { value: "standard", label: "standard", hint: "sonnet 4.6 · ~6–15s · 2-4 candidates" },
  { value: "deep", label: "deep", hint: "opus 4.7 · ~30–60s · 4-8 candidates" },
];

export function ExtractionModeToggle(): React.JSX.Element {
  const mode = useImportStore((s) => s.extractionMode);
  const setMode = useImportStore((s) => s.setExtractionMode);

  return (
    <fieldset
      style={{
        display: "flex",
        gap: 8,
        border: 0,
        margin: 0,
        padding: 0,
        alignItems: "center",
      }}
      aria-label="Extraction mode"
    >
      <legend
        className="section-label"
        style={{ marginRight: 8, padding: 0, float: "left" }}
      >
        mode
      </legend>
      {OPTIONS.map((opt) => {
        const checked = mode === opt.value;
        return (
          <label
            key={opt.value}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              padding: "4px 10px",
              border: checked
                ? "1px solid var(--accent-deep)"
                : "var(--hairline)",
              borderRadius: "var(--r-xs)",
              background: checked ? "var(--accent-tint)" : "transparent",
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              color: checked ? "var(--accent-warm)" : "var(--ink-secondary)",
              cursor: "pointer",
            }}
          >
            <input
              type="radio"
              name="extraction-mode"
              value={opt.value}
              checked={checked}
              onChange={() => setMode(opt.value)}
              style={{ accentColor: "var(--accent)" }}
            />
            {opt.label}
            <span
              style={{
                fontSize: 10,
                color: "var(--ink-tertiary)",
                marginLeft: 4,
              }}
            >
              {opt.hint}
            </span>
          </label>
        );
      })}
    </fieldset>
  );
}
