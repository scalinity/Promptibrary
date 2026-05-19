// Mutation hook calling start_launch — on success, navigates to /run/:id.
//
// In L2 the IPC always rejects with `not_yet_implemented`; the launch button
// surfaces that gracefully. L3 wires the actual PTY pipeline.

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { startLaunch, type StartLaunchArgs } from "@/shared/api/ipc";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { runKeys } from "@/shared/api/queryKeys";

export function useStartLaunch() {
  const navigate = useNavigate();
  const reset = useLaunchDraftStore((s) => s.reset);
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (args: StartLaunchArgs) => startLaunch(args),
    onSuccess: (profile) => {
      reset();
      queryClient.invalidateQueries({ queryKey: runKeys.all() });
      // The launch profile carries the runId only after L3; the L2 mock
      // path never reaches here, so this is a no-op in the visual specs.
      const launchedAt = profile.launchedAt;
      if (launchedAt != null) navigate("/run/latest");
    },
  });
}
