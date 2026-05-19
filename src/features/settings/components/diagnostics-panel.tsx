// Dependency probe diagnostics — runs `probe_dependencies` from the
// L5 system commands (SCA-734 aligned wire shape).

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
          probing failed — check that a vault is selected and try again.
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
          {probes.data.probes.map((p) => {
            const ok = p.status === "ok";
            return (
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
                      color: ok ? "var(--status-running)" : "var(--status-warn)",
                      marginRight: 6,
                    }}
                  >
                    {ok ? "✓" : "⚠"}
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
                  {p.version ?? p.installHint ?? "no info"}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
