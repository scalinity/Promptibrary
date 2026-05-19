// App outlet — wraps every route in the AppShell chrome.
//
// Per spec §12 every route shares the same topbar, sidebar, status line. The
// outlet drops the per-route content into the workspace pane.

import { Outlet } from "react-router-dom";

import { AppShell } from "@/shared/ui/app-shell";

export function App(): React.JSX.Element {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  );
}
