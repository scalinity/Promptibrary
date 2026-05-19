// AppShell — three-pane chrome (topbar / workspace / status bar).
//
// Mirrors the `.app-frame` layout from `Promptibrary Design System/app.css`.
// Renders the persistent chrome and slots route content into the workspace.

import { Topbar } from "./topbar";
import { Sidebar } from "./sidebar";
import { StatusLine } from "./status-line";

export interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps): React.JSX.Element {
  return (
    <>
      <div className="grain" aria-hidden="true" />
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
    </>
  );
}
