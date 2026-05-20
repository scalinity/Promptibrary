// AppShell — three-pane chrome (topbar / workspace / status bar).
//
// Mirrors the `.app-frame` layout from `Promptibrary Design System/app.css`.
// Renders the persistent chrome and slots route content into the workspace.

// eslint-disable-next-line no-restricted-imports -- SCA-723: pre-CLAUDE.md useEffect, refactor in follow-up cleanup pass
import { useEffect } from "react";

import { Topbar } from "./topbar";
import { Sidebar } from "./sidebar";
import { StatusLine } from "./status-line";
import { CmdKPalette } from "@/features/search/components/cmdk-palette";
import { useSettings } from "@/features/settings/hooks/use-settings";
import { useStatusLine } from "./use-status-line";

export interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps): React.JSX.Element {
  const settings = useSettings();
  const setModel = useStatusLine((s) => s.setModel);
  const setVaultPath = useStatusLine((s) => s.setVaultPath);

  // SCA-647 — hydrate status line from settings once available. Bridges an
  // async TanStack Query result into the Zustand chrome store; the no-
  // useEffect rule has its documented exception for exactly this case.
  useEffect(() => {
    if (settings.data == null) return;
    setModel(settings.data.effective.defaultModel);
    setVaultPath(settings.data.effective.vaultPath);
  }, [settings.data, setModel, setVaultPath]);

  return (
    <>
      <div className="app-frame">
        <Topbar />
        <div className="workspace">
          <Sidebar />
          <div className="route-outlet" style={{ display: "contents" }}>
            {children}
          </div>
        </div>
        <StatusLine />
      </div>
      <CmdKPalette />
    </>
  );
}
