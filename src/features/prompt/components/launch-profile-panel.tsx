// Read-only launch profile summary — model, permission, MCP, working dir.
//
// Editing happens in the launch drawer; this panel is the at-a-glance view
// inside the prompt route's left rail.

import type { Prompt } from "@/shared/types/prompt";

interface Props {
  prompt: Prompt;
}

export function LaunchProfilePanel({ prompt }: Props): React.JSX.Element {
  const d = prompt.launchDefaults;
  return (
    <div style={{ padding: "0 var(--sp-5) var(--sp-5)", display: "grid", gap: 10 }}>
      <div className="section-label">launch profile</div>
      <ul
        style={{
          listStyle: "none",
          padding: 0,
          margin: 0,
          display: "grid",
          gap: 4,
        }}
      >
        <ProfileRow k="model" v={d.model} />
        <ProfileRow k="permission" v={d.permissionMode} />
        <ProfileRow k="verifier" v={d.verifierMode} />
        <ProfileRow
          k="working dir"
          v={d.workingDirectory ?? "— set at launch"}
        />
        <ProfileRow
          k="mcp"
          v={
            d.mcpConfigPaths.length === 0
              ? "none"
              : `${d.mcpConfigPaths.length} file${d.mcpConfigPaths.length === 1 ? "" : "s"}`
          }
        />
      </ul>
    </div>
  );
}

function ProfileRow({ k, v }: { k: string; v: string }): React.JSX.Element {
  return (
    <li
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "baseline",
        gap: 12,
        padding: "4px 0",
        borderBottom: "var(--hairline)",
      }}
    >
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10.5,
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {k}
      </span>
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--ink-primary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          maxWidth: "60%",
        }}
      >
        {v}
      </span>
    </li>
  );
}
