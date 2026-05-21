// Create-prompt mutation — wraps `createPrompt` IPC with TanStack Query.
//
// On success: invalidate the prompt list, select the new prompt in the
// library store so the detail pane shows it, then navigate to the editor.

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { createPrompt } from "@/shared/api/ipc";
import { promptKeys } from "@/shared/api/queryKeys";
import { useLibraryStore } from "@/features/library/stores/library-store";

const DEFAULT_TITLE = "Untitled prompt";

export function useCreatePrompt() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const select = useLibraryStore((s) => s.select);

  return useMutation({
    mutationFn: (title?: string) => createPrompt({ title: title ?? DEFAULT_TITLE }),
    onSuccess: (prompt) => {
      queryClient.invalidateQueries({ queryKey: promptKeys.all() });
      select(prompt.id);
      navigate(`/prompt/${prompt.id}`);
    },
  });
}
