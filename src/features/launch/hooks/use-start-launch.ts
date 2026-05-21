// Mutation hook calling start_launch — on success, navigates to /run/:runId.
//
// Spec §7: the Tauri command returns `runId` so the frontend can route to
// the run pane and subscribe to `term:stdout` / `term:finished` events.

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { startLaunch, type StartLaunchArgs } from "@/shared/api/ipc";
import { defaultMessage, isAppError } from "@/shared/api/errors";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { runKeys } from "@/shared/api/queryKeys";

export function useStartLaunch() {
  const navigate = useNavigate();
  const reset = useLaunchDraftStore((s) => s.reset);
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (args: StartLaunchArgs) => startLaunch(args),
    onSuccess: ({ runId }) => {
      reset();
      queryClient.invalidateQueries({ queryKey: runKeys.all() });
      navigate(`/run/${runId}`);
    },
    // SCA-911 (W13): surface launch failures instead of letting TanStack
    // Query swallow them silently. Mirrors the createPrompt/deletePrompt
    // pattern shipped with SCA-896.
    onError: (err) => {
      console.error("startLaunch failed:", err);
      if (isAppError(err)) {
        window.alert(`Launch failed: ${defaultMessage(err.kind)}`);
        return;
      }
      window.alert(`Launch failed: ${String(err)}`);
    },
  });
}
