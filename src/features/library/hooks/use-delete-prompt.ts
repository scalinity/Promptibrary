// Delete-prompt mutation — wraps `deletePrompt` IPC with TanStack Query.
//
// On success: invalidate the prompt list query and clear the library
// selection if the deleted prompt was the currently selected one.
// On error: surface via window.alert so a failure isn't silent.

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { deletePrompt } from "@/shared/api/ipc";
import { defaultMessage, isAppError } from "@/shared/api/errors";
import { promptKeys } from "@/shared/api/queryKeys";
import { useLibraryStore } from "@/features/library/stores/library-store";
import type { PromptId } from "@/shared/types/ids";

export function useDeletePrompt() {
  const queryClient = useQueryClient();
  const selectedId = useLibraryStore((s) => s.selectedPromptId);
  const select = useLibraryStore((s) => s.select);

  return useMutation({
    mutationFn: (id: PromptId) => deletePrompt(id),
    onSuccess: (_void, id) => {
      queryClient.invalidateQueries({ queryKey: promptKeys.all() });
      if (selectedId === id) {
        select(null);
      }
    },
    onError: (err) => {
      console.error("deletePrompt failed:", err);
      if (isAppError(err)) {
        window.alert(defaultMessage(err.kind));
        return;
      }
      window.alert(`Failed to delete prompt: ${String(err)}`);
    },
  });
}
