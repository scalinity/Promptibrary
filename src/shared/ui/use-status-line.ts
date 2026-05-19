// Status-line store — read-only public hook plus dispatch helpers used by
// feature surfaces to update the bottom chrome.

import { create } from "zustand";

export type StatusState = "READY" | "RUNNING" | "OFFLINE" | "ERROR";

export interface StatusLineState {
  state: StatusState;
  vaultPath: string | null;
  model: string;
  promptCount: number;
  lastRunLabel: string | null;
  setState: (state: StatusState) => void;
  setVaultPath: (path: string | null) => void;
  setModel: (model: string) => void;
  setPromptCount: (count: number) => void;
  setLastRunLabel: (label: string | null) => void;
}

export const useStatusLine = create<StatusLineState>((set) => ({
  state: "READY",
  vaultPath: null,
  // Hydrated from useSettings().data?.effective.defaultModel by the
  // AppShell on mount; empty string until that round-trip completes.
  model: "",
  promptCount: 0,
  lastRunLabel: null,
  setState: (state) => set({ state }),
  setVaultPath: (vaultPath) => set({ vaultPath }),
  setModel: (model) => set({ model }),
  setPromptCount: (promptCount) => set({ promptCount }),
  setLastRunLabel: (lastRunLabel) => set({ lastRunLabel }),
}));
