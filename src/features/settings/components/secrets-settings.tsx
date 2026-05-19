// Secret keys — write-only inputs that surface masked status from the
// keychain via `get_secret_status`. The actual value never crosses IPC after
// it's set; the surface only knows "exists" + a last-validated stamp.

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { setSecret } from "@/shared/api/ipc";
import { useSecretStatus } from "@/features/settings/hooks/use-settings";
import { settingsKeys } from "@/shared/api/queryKeys";
import type { SecretKey } from "@/shared/types/settings";

interface Field {
  key: SecretKey;
  label: string;
  help: string;
}

const FIELDS: Field[] = [
  {
    key: "anthropic_api_key",
    label: "Anthropic API key",
    help: "Used by the extraction pipeline. Stored in the OS keychain only.",
  },
  {
    key: "x_bearer_token",
    label: "X bearer token",
    help: "Required for X/Twitter source extraction. Optional.",
  },
];

export function SecretsSettings(): React.JSX.Element {
  const status = useSecretStatus();
  return (
    <section id="secrets" aria-labelledby="secrets-h">
      <div className="section-label" id="secrets-h">
        secrets
      </div>
      <div style={{ display: "grid", gap: "var(--sp-4)", marginTop: 8 }}>
        {FIELDS.map((field) => (
          <SecretRow
            key={field.key}
            field={field}
            exists={status.data?.[field.key]?.exists ?? false}
          />
        ))}
      </div>
    </section>
  );
}

function SecretRow({
  field,
  exists,
}: {
  field: Field;
  exists: boolean;
}): React.JSX.Element {
  const [value, setValue] = useState("");
  const [savedLabel, setSavedLabel] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const save = useMutation({
    mutationFn: (v: string) => setSecret({ key: field.key, value: v }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: settingsKeys.secrets() });
      setSavedLabel("saved");
      setValue("");
      window.setTimeout(() => setSavedLabel(null), 1800);
    },
  });

  return (
    <div
      style={{
        display: "grid",
        gap: 6,
        padding: "var(--sp-3) var(--sp-4)",
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
      }}
    >
      <label
        style={{
          fontFamily: "var(--font-ui)",
          fontSize: "12.5px",
          color: "var(--ink-primary)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        {field.label}
        <span
          aria-live="polite"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "10.5px",
            color: exists ? "var(--accent)" : "var(--ink-dim)",
            letterSpacing: "0.04em",
          }}
        >
          {savedLabel ?? (exists ? "•••••••• stored" : "not set")}
        </span>
      </label>
      <p
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "11px",
          color: "var(--ink-tertiary)",
          margin: 0,
        }}
      >
        {field.help}
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          type="password"
          className="input"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={exists ? "replace…" : "paste…"}
          autoComplete="off"
        />
        <button
          type="button"
          className="btn"
          disabled={value.length === 0 || save.isPending}
          onClick={() => save.mutate(value)}
        >
          {save.isPending ? "saving…" : "save"}
        </button>
      </div>
    </div>
  );
}
