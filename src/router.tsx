// Router config per spec §12 route table.
//
// React Router 7 in declarative (no-data-loader) mode. Routes wrap the AppShell
// outlet so every screen shares the same chrome (topbar, sidebar, status bar).

import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate, useLocation } from "react-router-dom";

import { App } from "./app";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";
import { RouteErrorBoundary } from "@/shared/ui/error-boundary";

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

// SCA-911 (C6): wrap each route in an ErrorBoundary that auto-resets when
// the pathname changes. `useLocation` requires being inside a Router child,
// hence this small wrapper component rather than calling it at module scope.
function withBoundary(node: React.ReactNode) {
  return <RouteBoundaryShell>{node}</RouteBoundaryShell>;
}

function RouteBoundaryShell({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKey={location.pathname}>
      <Suspense fallback={<RouteFallback />}>{children}</Suspense>
    </RouteErrorBoundary>
  );
}

export const router = createBrowserRouter([
  {
    path: "/",
    element: <App />,
    children: [
      { index: true, element: withBoundary(<LibraryRoute />) },
      {
        path: "prompt/:promptId",
        element: withBoundary(<PromptRoute />),
      },
      {
        path: "run/:runId",
        element: withBoundary(<RunRoute />),
      },
      {
        path: "import",
        element: withBoundary(<ImportRoute />),
      },
      {
        path: "settings",
        element: withBoundary(<SettingsRoute />),
      },
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
]);
