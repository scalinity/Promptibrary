// Single prompt query — calls `getPrompt` IPC directly.
//
// `listPrompts` returns a slim `PromptListItem` (no body / variables /
// telemetry / launchDefaults), so plucking from the list cache is not an
// option. The dedicated `get_prompt` Rust command returns the full
// `Prompt` shape this hook is typed to.

import { useQuery } from "@tanstack/react-query";

import { getPrompt } from "@/shared/api/ipc";
import { promptKeys } from "@/shared/api/queryKeys";
import type { PromptId } from "@/shared/types/ids";

// Stable disabled-key sentinel so we don't pollute the cache namespace with
// a fake PromptId entry when no prompt is selected.
const DISABLED_KEY = ["prompts", "detail", "__disabled"] as const;

export function usePrompt(id: PromptId | null) {
  return useQuery({
    queryKey: id != null ? promptKeys.detail(id) : DISABLED_KEY,
    enabled: id != null,
    queryFn: () => getPrompt(id!),
  });
}
