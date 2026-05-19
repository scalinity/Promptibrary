import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";

import { router } from "./router";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // The vault watcher (L1) emits `vault://changed` events that drive
      // explicit invalidation, so background refetching is wasted work here.
      refetchOnWindowFocus: false,
      // 30s staleness is generous enough for the shell to feel snappy
      // without revealing stale prompt rows after a launch.
      staleTime: 30_000,
      retry: 1,
    },
    mutations: {
      retry: 0,
    },
  },
});

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root element missing from index.html");
}

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);