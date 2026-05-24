// Update-prompt mutation — wraps `updatePrompt` IPC with TanStack Query.
//
// On success: invalidate the prompts namespace so list + detail surfaces
// refetch. On error: surface via alert so silent IPC failures don't leave
// the user wondering why their edit didn't stick (mirrors `useCreatePrompt`).

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { updatePrompt, type UpdatePromptArgs } from "@/shared/api/ipc";
import { defaultMessage, isAppError } from "@/shared/api/errors";
import { promptKeys } from "@/shared/api/queryKeys";

export function useUpdatePrompt() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (args: UpdatePromptArgs) => updatePrompt(args),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: promptKeys.all() });
    },
    onError: (err) => {
      console.error("updatePrompt failed:", err);
      if (isAppError(err)) {
        window.alert(defaultMessage(err.kind));
        return;
      }
      window.alert(`Failed to update prompt: ${String(err)}`);
    },
  });
}
