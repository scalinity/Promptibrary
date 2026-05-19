// Cmd-K palette + global search state.
//
// `query` reflects the current input; `open(seed)` opens the dialog and seeds
// the query (used by the topbar trigger). Actual result fetching lives in
// `useSearch`; this store only owns open/closed + query + semantic toggle.

import { create } from "zustand";

export interface SearchState {
  isOpen: boolean;
  query: string;
  semantic: boolean;
  open: (seed?: string) => void;
  close: () => void;
  setQuery: (query: string) => void;
  toggleSemantic: () => void;
}

export const useSearchStore = create<SearchState>((set) => ({
  isOpen: false,
  query: "",
  semantic: false,
  open: (seed = "") => set({ isOpen: true, query: seed }),
  close: () => set({ isOpen: false }),
  setQuery: (query) => set({ query }),
  toggleSemantic: () => set((s) => ({ semantic: !s.semantic })),
}));
