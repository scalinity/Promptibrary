// Route: `/settings` — section list with anchor links + panels.

import { VaultSettings } from "@/features/settings/components/vault-settings";
import { DefaultsSettings } from "@/features/settings/components/defaults-settings";
import { ExtractionSettings } from "@/features/settings/components/extraction-settings";
import { AssistantSettings } from "@/features/settings/components/assistant-settings";
import { SecretsSettings } from "@/features/settings/components/secrets-settings";
import { TelemetrySettings } from "@/features/settings/components/telemetry-settings";
import { DiagnosticsPanel } from "@/features/settings/components/diagnostics-panel";
import { UpdaterSettings } from "@/features/settings/components/updater-settings";

const NAV = [
  { id: "vault", label: "vault" },
  { id: "defaults", label: "defaults" },
  { id: "extraction", label: "extraction" },
  { id: "assistant", label: "assistant" },
  { id: "secrets", label: "secrets" },
  { id: "telemetry", label: "telemetry" },
  { id: "diagnostics", label: "diagnostics" },
  { id: "updater", label: "updater" },
];

export function SettingsRoute(): React.JSX.Element {
  return (
    <>
      <aside
        className="list-pane"
        aria-label="Settings sections"
        style={{ padding: "var(--sp-5)" }}
      >
        <header
          style={{
            fontFamily: "var(--font-display)",
            fontWeight: 500,
            fontSize: 18,
            letterSpacing: "-0.01em",
            marginBottom: 16,
          }}
        >
          Settings
        </header>
        <nav style={{ display: "grid", gap: 6 }}>
          {NAV.map((item) => (
            <a
              key={item.id}
              href={`#${item.id}`}
              style={{
                fontFamily: "var(--font-ui)",
                fontSize: "13px",
                color: "var(--ink-secondary)",
                padding: "6px 8px",
                borderRadius: "var(--r-sm)",
                textDecoration: "none",
              }}
            >
              {item.label}
            </a>
          ))}
        </nav>
      </aside>
      <section
        className="detail-pane"
        aria-label="Settings panels"
        style={{ overflow: "auto" }}
      >
        <div
          style={{
            display: "grid",
            gap: 32,
            padding: "var(--sp-6) var(--sp-7)",
            maxWidth: 760,
            margin: "0 auto",
          }}
        >
          <VaultSettings />
          <DefaultsSettings />
          <ExtractionSettings />
          <AssistantSettings />
          <SecretsSettings />
          <TelemetrySettings />
          <DiagnosticsPanel />
          <UpdaterSettings />
        </div>
      </section>
    </>
  );
}
