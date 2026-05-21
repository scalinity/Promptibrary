// Create-prompt mutation — wraps `createPrompt` IPC with TanStack Query.
//
// On success: invalidate the prompt list, select the new prompt in the
// library store so the detail pane shows it, then navigate to the editor.
// On error: surface the failure so users don't see a silent no-op when
// the vault isn't configured or the IPC otherwise fails (SCA-896).

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { createPrompt } from "@/shared/api/ipc";
import { defaultMessage, isAppError } from "@/shared/api/errors";
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
    onError: (err) => {
      console.error("createPrompt failed:", err);
      if (isAppError(err)) {
        if (err.kind === "VaultMissing" || err.kind === "VaultInvalid") {
          window.alert(
            `${defaultMessage(err.kind)} Configure a vault in Settings to create prompts.`,
          );
          navigate("/settings");
          return;
        }
        window.alert(defaultMessage(err.kind));
        return;
      }
      window.alert(`Failed to create prompt: ${String(err)}`);
    },
  });
}
