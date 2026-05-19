// Vault path + status cards.

import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/features/settings/hooks/use-settings";
import { selectVault, getVaultStatus } from "@/shared/api/ipc";
import { asAbsolutePath } from "@/shared/types/ids";
import { settingsKeys, vaultKeys } from "@/shared/api/queryKeys";

export function VaultSettings(): React.JSX.Element {
  const settings = useSettings();
  const queryClient = useQueryClient();
  const status = useQuery({
    queryKey: vaultKeys.status(),
    queryFn: getVaultStatus,
    enabled: settings.data?.local.vaultPath != null,
  });
  const select = useMutation({
    mutationFn: (path: string) => selectVault(asAbsolutePath(path)),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: settingsKeys.all() });
      queryClient.invalidateQueries({ queryKey: vaultKeys.status() });
    },
  });

  const vaultPath = settings.data?.local.vaultPath ?? null;

  return (
    <section id="vault" aria-labelledby="vault-h">
      <div className="section-label" id="vault-h">
        vault
      </div>
      <div style={{ display: "grid", gap: "var(--sp-3)", marginTop: 8 }}>
        <button
          type="button"
          className={vaultPath == null ? "picker empty" : "picker"}
          onClick={async () => {
            const result = await openDialog({
              directory: true,
              multiple: false,
            });
            if (typeof result === "string") select.mutate(result);
          }}
        >
          <span className="glyph">⊟</span>
          <span className="path">{vaultPath ?? "choose a vault folder…"}</span>
        </button>
        {status.data != null && (
          <ul
            style={{
              listStyle: "none",
              padding: 0,
              margin: 0,
              display: "grid",
              gridTemplateColumns: "repeat(2, 1fr)",
              gap: 8,
            }}
          >
            <StatusCard
              label="vault root"
              ok={status.data.vaultRoot != null}
              detail={status.data.vaultRoot ?? "no path set"}
            />
            <StatusCard
              label="initialized"
              ok={status.data.initialized}
              detail={
                status.data.initialized
                  ? "schema + index ready"
                  : "not yet initialized"
              }
            />
          </ul>
        )}
        {/* Per spec §13, expanded vault probes (writable, git repo, watcher
            running) land alongside the L5 diagnostics surface. */}
      </div>
    </section>
  );
}

function StatusCard({
  label,
  ok,
  detail,
}: {
  label: string;
  ok: boolean;
  detail: string;
}): React.JSX.Element {
  return (
    <li
      style={{
        background: "var(--bg-sunken)",
        border: "var(--hairline)",
        borderRadius: "var(--r-md)",
        padding: "var(--sp-3) var(--sp-4)",
        display: "grid",
        gap: 4,
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "10.5px",
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {label}
      </span>
      <span
        style={{
          fontFamily: "var(--font-ui)",
          color: ok ? "var(--ink-primary)" : "var(--status-warn)",
          fontSize: "12.5px",
        }}
      >
        <span aria-hidden="true">{ok ? "✓ " : "⚠ "}</span>
        {detail}
      </span>
    </li>
  );
}
