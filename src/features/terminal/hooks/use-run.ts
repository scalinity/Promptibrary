// SCA-918: query hook for run metadata. Backed by the get_run IPC
// landed in SCA-917.

import { useQuery } from "@tanstack/react-query";

import { getRun } from "@/shared/api/ipc";
import { runKeys } from "@/shared/api/queryKeys";
import type { RunId } from "@/shared/types/ids";

export function useRun(runId: RunId | undefined, enabled = true) {
  return useQuery({
    queryKey: runId ? runKeys.detail(runId) : runKeys.all(),
    queryFn: () => {
      if (runId == null) throw new Error("useRun called without runId");
      return getRun(runId);
    },
    enabled: enabled && runId != null,
    // A finished run is immutable — once we see ended_at, the row
    // won't change. For active runs the events channel pushes
    // updates, so a stale-time of 1s keeps us from re-fetching on
    // every render while still picking up status transitions.
    staleTime: 1000,
  });
}
