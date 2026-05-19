// Default model / permission mode / verifier mode card.
//
// L2 surface reads the values; persistence wires to L5 once the settings
// mutation IPC lands (currently the IPC is the L1 stub).

import { useSettings } from "@/features/settings/hooks/use-settings";

export function DefaultsSettings(): React.JSX.Element {
  const settings = useSettings();
  const local = settings.data?.local;

  return (
    <section id="defaults" aria-labelledby="defaults-h">
      <div className="section-label" id="defaults-h">
        defaults
      </div>
      <dl className="kv-grid" style={kvGridStyle}>
        <KV k="default model" v={local?.defaultModel ?? "—"} />
        <KV k="permission mode" v={local?.defaultPermissionMode ?? "—"} />
        <KV k="verifier mode" v={local?.defaultVerifierMode ?? "—"} />
        <KV k="destination" v={local?.defaultDestination ?? "—"} />
      </dl>
      <p style={notePStyle}>
        Editing defaults activates in L5 alongside the settings mutation IPC.
      </p>
    </section>
  );
}

const kvGridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "max-content 1fr",
  rowGap: 8,
  columnGap: 16,
  marginTop: 8,
};
const notePStyle: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  fontSize: "11px",
  color: "var(--ink-tertiary)",
  marginTop: 16,
};

function KV({ k, v }: { k: string; v: string }): React.JSX.Element {
  return (
    <>
      <dt
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {k}
      </dt>
      <dd
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "12.5px",
          color: "var(--ink-primary)",
          margin: 0,
        }}
      >
        {v}
      </dd>
    </>
  );
}
