// Dependency probes — calls probe_dependencies for the diagnostics panel.

import { useQuery } from "@tanstack/react-query";

import { probeDependencies } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";

export function useDependencyProbes() {
  return useQuery({
    queryKey: settingsKeys.dependencies(),
    queryFn: probeDependencies,
    staleTime: 60_000,
  });
}
