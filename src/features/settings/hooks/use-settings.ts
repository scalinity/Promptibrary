// Settings + secrets queries.

import { useQuery } from "@tanstack/react-query";

import { getSettings, getSecretStatus } from "@/shared/api/ipc";
import { settingsKeys } from "@/shared/api/queryKeys";

export function useSettings() {
  return useQuery({
    queryKey: settingsKeys.effective(),
    queryFn: getSettings,
  });
}

export function useSecretStatus() {
  return useQuery({
    queryKey: settingsKeys.secrets(),
    queryFn: getSecretStatus,
  });
}
