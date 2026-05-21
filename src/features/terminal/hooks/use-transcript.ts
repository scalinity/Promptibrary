// SCA-918: query hook for completed-run transcript content. Backed by
// the fetch_transcript IPC landed in SCA-917.

import { useQuery } from "@tanstack/react-query";

import { fetchTranscript } from "@/shared/api/ipc";
import { runKeys } from "@/shared/api/queryKeys";
import type { RunId } from "@/shared/types/ids";

export function useTranscript(runId: RunId | undefined, enabled = true) {
  return useQuery({
    queryKey: runId ? [...runKeys.detail(runId), "transcript"] : runKeys.all(),
    queryFn: () => {
      if (runId == null) throw new Error("useTranscript called without runId");
      return fetchTranscript(runId);
    },
    enabled: enabled && runId != null,
    // Transcripts are append-only-then-frozen; once the run is done
    // the content never changes. Long stale-time avoids re-fetching
    // on simple route remounts.
    staleTime: 60_000,
  });
}
