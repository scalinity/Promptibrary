// Dependency probe diagnostics — runs `probe_dependencies` (L0 stub for
// now; the L5 implementation populates real version + path info).

import { useDependencyProbes } from "@/features/settings/hooks/use-dependency-probes";

export function DiagnosticsPanel(): React.JSX.Element {
  const probes = useDependencyProbes();
  return (
    <section id="diagnostics" aria-labelledby="diagnostics-h">
      <div className="section-label" id="diagnostics-h">
        diagnostics
      </div>
      {probes.isError && (
        <p
          role="alert"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--status-warn)",
          }}
        >
          probing not yet implemented — L5 wires this surface.
        </p>
      )}
      {probes.data != null && (
        <ul
          style={{
            listStyle: "none",
            padding: 0,
            margin: "8px 0 0",
            display: "grid",
            gap: 8,
          }}
        >
          {probes.data.map((p) => (
            <li
              key={p.name}
              style={{
                background: "var(--bg-sunken)",
                border: "var(--hairline)",
                borderRadius: "var(--r-md)",
                padding: "var(--sp-3) var(--sp-4)",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
              }}
            >
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "12.5px",
                  color: "var(--ink-primary)",
                }}
              >
                <span
                  aria-hidden="true"
                  style={{
                    color: p.ok ? "var(--status-running)" : "var(--status-warn)",
                    marginRight: 6,
                  }}
                >
                  {p.ok ? "✓" : "⚠"}
                </span>
                {p.name}
              </span>
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: "11px",
                  color: "var(--ink-tertiary)",
                }}
              >
                {p.version ?? p.message ?? "no info"}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
