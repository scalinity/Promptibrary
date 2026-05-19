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
              label="exists"
              ok={status.data.exists}
              detail={status.data.exists ? "directory found" : "missing"}
            />
            <StatusCard
              label="git repo"
              ok={status.data.isGitRepo}
              detail={
                status.data.isGitRepo
                  ? "version history enabled"
                  : "no history will be kept"
              }
            />
            <StatusCard
              label="writable"
              ok={status.data.writable}
              detail={status.data.writable ? "can save" : "read-only"}
            />
            <StatusCard
              label="watcher"
              ok={status.data.watcherRunning}
              detail={status.data.watcherRunning ? "live" : "stopped"}
            />
          </ul>
        )}
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
