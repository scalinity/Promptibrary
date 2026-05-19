// Router config per spec §12 route table.
//
// React Router 7 in declarative (no-data-loader) mode. Routes wrap the AppShell
// outlet so every screen shares the same chrome (topbar, sidebar, status bar).

import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate } from "react-router-dom";

import { App } from "./app";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";

const LibraryRoute = lazy(() =>
  import("@/features/library/routes/library-route").then((m) => ({
    default: m.LibraryRoute,
  })),
);
const PromptRoute = lazy(() =>
  import("@/features/prompt/routes/prompt-route").then((m) => ({
    default: m.PromptRoute,
  })),
);
const RunRoute = lazy(() =>
  import("@/features/terminal/routes/run-route").then((m) => ({
    default: m.RunRoute,
  })),
);
const ImportRoute = lazy(() =>
  import("@/features/import/routes/import-route").then((m) => ({
    default: m.ImportRoute,
  })),
);
const SettingsRoute = lazy(() =>
  import("@/features/settings/routes/settings-route").then((m) => ({
    default: m.SettingsRoute,
  })),
);

function RouteFallback() {
  return (
    <div
      style={{
        display: "grid",
        placeItems: "center",
        height: "100%",
        color: "var(--ink-tertiary)",
      }}
    >
      <LoadingSpinner label="loading…" />
    </div>
  );
}

function withSuspense(node: React.ReactNode) {
  return <Suspense fallback={<RouteFallback />}>{node}</Suspense>;
}

export const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: withSuspense(<LibraryRoute />) },
      {
        path: "prompt/:promptId",
        element: withSuspense(<PromptRoute />),
      },
      {
        path: "run/:runId",
        element: withSuspense(<RunRoute />),
      },
      {
        path: "import",
        element: withSuspense(<ImportRoute />),
      },
      {
        path: "settings",
        element: withSuspense(<SettingsRoute />),
      },
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
]);
