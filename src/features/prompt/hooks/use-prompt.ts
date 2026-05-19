// Single prompt query — derives from the cached `listPrompts` result.
//
// L1 has no dedicated `get_prompt` IPC; the L1 surface scans the vault and
// exposes a uniform list. We pluck the single prompt from the cached list
// query for the L2 surface so we don't refetch per route.

import { useQuery } from "@tanstack/react-query";

import { listPrompts } from "@/shared/api/ipc";
import { promptKeys } from "@/shared/api/queryKeys";
import type { PromptId } from "@/shared/types/ids";

export function usePrompt(id: PromptId | null) {
  return useQuery({
    queryKey: id != null ? promptKeys.detail(id) : promptKeys.detail("none" as PromptId),
    enabled: id != null,
    queryFn: async () => {
      const all = await listPrompts({ includeArchived: true });
      const found = all.find((p) => p.id === id);
      if (found == null) {
        throw {
          kind: "PromptNotFound",
          message: `Prompt ${id} not found in vault`,
          details: { id: String(id) },
        };
      }
      return found;
    },
  });
}
