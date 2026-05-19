// Right-drawer launch composer.
//
// L2 surface: variable form + working-dir + overrides + launch button.
// The actual launch is wired in L3 (the IPC currently returns
// `not_yet_implemented`).

import { VariableForm } from "./variable-form";
import { WorkingDirPicker } from "./working-dir-picker";
import { LaunchButton } from "./launch-button";
import { InlineTweakEditor } from "./inline-tweak-editor";
import { PermissionFlagsEditor } from "./permission-flags-editor";
import { McpConfigEditor } from "./mcp-config-editor";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import type { Prompt } from "@/shared/types/prompt";

interface LaunchDrawerProps {
  prompt: Prompt;
}

export function LaunchDrawer({ prompt }: LaunchDrawerProps): React.JSX.Element {
  const close = useLaunchDraftStore((s) => s.close);

  return (
    <aside
      className="launch-drawer"
      role="dialog"
      aria-modal="false"
      aria-label="Launch composer"
      style={{
        position: "absolute",
        top: 0,
        right: 0,
        bottom: 0,
        width: 380,
        background: "var(--bg-base)",
        borderLeft: "var(--hairline)",
        zIndex: 60,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <header
        style={{
          padding: "var(--sp-4) var(--sp-5)",
          borderBottom: "var(--hairline)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span className="section-label">launch</span>
        <button
          type="button"
          className="icon-btn"
          aria-label="Close drawer"
          onClick={close}
        >
          ✕
        </button>
      </header>
      <div
        style={{
          flex: 1,
          overflow: "auto",
          padding: "var(--sp-5)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--sp-6)",
        }}
      >
        <VariableForm prompt={prompt} />
        <WorkingDirPicker prompt={prompt} />
        <InlineTweakEditor prompt={prompt} />
        <PermissionFlagsEditor prompt={prompt} />
        <McpConfigEditor prompt={prompt} />
      </div>
      <footer
        style={{
          padding: "var(--sp-4) var(--sp-5)",
          borderTop: "var(--hairline)",
        }}
      >
        <LaunchButton prompt={prompt} />
      </footer>
    </aside>
  );
}
