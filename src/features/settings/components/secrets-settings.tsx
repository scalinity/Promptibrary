// Secret keys — write-only inputs that surface masked status from the
// keychain via `get_secret_status`. The actual value never crosses IPC after
// it's set; the surface only knows "exists" + a last-validated stamp.

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { setSecret } from "@/shared/api/ipc";
import { isAppError } from "@/shared/api/errors";
import { useSecretStatus } from "@/features/settings/hooks/use-settings";
import { settingsKeys } from "@/shared/api/queryKeys";
import { useToast } from "@/shared/ui/use-toast";
import type { SecretKey } from "@/shared/types/settings";

interface Field {
  key: SecretKey;
  label: string;
  help: string;
}

interface ExtendedField extends Field {
  advanced?: boolean;
}

const FIELDS: ExtendedField[] = [
  {
    key: "anthropic_api_key",
    label: "Anthropic API key",
    help: "Used by the extraction pipeline. Stored in the OS keychain only.",
  },
  {
    key: "x_bearer_token",
    label: "X bearer token",
    help: "Optional. Required only for thread reconstruction on X/Twitter — oEmbed handles single tweets without a token. X API access is paid.",
    advanced: true,
  },
];

export function SecretsSettings(): React.JSX.Element {
  const status = useSecretStatus();
  const xBearerSet = status.data?.x_bearer_token?.exists ?? false;
  // Reveal the advanced field by default when the user already stored
  // a value for it — hiding it would orphan the existing secret.
  const [showAdvanced, setShowAdvanced] = useState(xBearerSet);

  const visibleFields = FIELDS.filter((f) => !f.advanced || showAdvanced);

  return (
    <section id="secrets" aria-labelledby="secrets-h">
      <div className="section-label" id="secrets-h">
        secrets
      </div>
      <div style={{ display: "grid", gap: "var(--sp-4)", marginTop: 8 }}>
        {visibleFields.map((field) => (
          <SecretRow
            key={field.key}
            field={field}
            exists={status.data?.[field.key]?.exists ?? false}
          />
        ))}
        {!showAdvanced && (
          <button
            type="button"
            onClick={() => setShowAdvanced(true)}
            style={{
              alignSelf: "flex-start",
              background: "transparent",
              border: 0,
              padding: 0,
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              color: "var(--ink-tertiary)",
              cursor: "pointer",
              letterSpacing: "0.02em",
            }}
          >
            + show advanced secret fields
          </button>
        )}
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
  const { toast, showToast } = useToast(1800);
  const queryClient = useQueryClient();

  const save = useMutation({
    mutationFn: (v: string) => setSecret({ key: field.key, value: v }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: settingsKeys.secrets() });
      showToast("saved");
      setValue("");
    },
    // SCA-641 — surface failures instead of swallowing. Keep the value so
    // the user doesn't have to retype.
    onError: (err: unknown) => {
      if (isAppError(err)) {
        showToast(`failed: ${err.message}`);
      } else {
        showToast("failed");
      }
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
          {toast ?? (exists ? "•••••••• stored" : "not set")}
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
