// MCP config paths editor — append/remove .json paths.
//
// L2 surface lets the user add and remove MCP config files; deeper editing
// (per-server overrides) is V2.

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import {
  asAbsolutePath,
  type AbsolutePath,
} from "@/shared/types/ids";
import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
}

export function McpConfigEditor({ prompt }: Props): React.JSX.Element {
  const overrides = useLaunchDraftStore((s) => s.overrides);
  const setOverride = useLaunchDraftStore((s) => s.setOverride);
  const paths: AbsolutePath[] =
    overrides.mcpConfigPaths ?? prompt.launchDefaults.mcpConfigPaths;

  return (
    <section>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <span className="section-label">mcp config</span>
        <button
          type="button"
          className="btn"
          onClick={async () => {
            const result = await openDialog({
              filters: [{ name: "JSON", extensions: ["json"] }],
            });
            if (typeof result === "string") {
              setOverride("mcpConfigPaths", [
                ...paths,
                asAbsolutePath(result),
              ]);
            }
          }}
        >
          add
        </button>
      </div>
      {paths.length === 0 ? (
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--ink-tertiary)",
          }}
        >
          no MCP config files
        </div>
      ) : (
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {paths.map((p) => (
            <li
              key={p}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                fontFamily: "var(--font-mono)",
                fontSize: "11.5px",
                color: "var(--ink-secondary)",
                padding: "4px 0",
              }}
            >
              <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>
                {p}
              </span>
              <button
                type="button"
                className="icon-btn"
                aria-label={`Remove ${p}`}
                onClick={() =>
                  setOverride(
                    "mcpConfigPaths",
                    paths.filter((q) => q !== p),
                  )
                }
              >
                ✕
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
