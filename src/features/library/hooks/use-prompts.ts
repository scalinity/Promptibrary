// Prompts query hook — wraps `listPrompts` IPC with TanStack Query.
//
// The library route uses this for the row list AND to seed the sidebar
// counts (via the route-level effect). FTS-backed search is L5; the L2
// surface filters client-side.

import { useQuery } from "@tanstack/react-query";

import { listPrompts } from "@/shared/api/ipc";
import { promptKeys } from "@/shared/api/queryKeys";

export function usePrompts(includeArchived = false) {
  return useQuery({
    queryKey: promptKeys.list({ includeArchived }),
    queryFn: () => listPrompts({ includeArchived }),
  });
}
