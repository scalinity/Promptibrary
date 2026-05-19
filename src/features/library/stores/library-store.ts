// Library route — local filter/sort/layout state + sidebar stats.
//
// `useLibraryStore` carries the per-route UI state (search query, tag filter,
// selected prompt). `useLibraryStats` is a separate hook that exposes the
// sidebar counts; feature code updates it when prompts/tags/vaults change so
// the sidebar stays in sync without route-aware imports.

import { create } from "zustand";

import type { PromptId, TagName } from "@/shared/types/ids";

export type LibrarySort =
  | "recently_used"
  | "recently_edited"
  | "title"
  | "launch_count";

export interface LibraryFilter {
  query: string;
  tag: TagName | null;
  archived: boolean;
}

export interface LibraryState {
  filter: LibraryFilter;
  sort: LibrarySort;
  selectedPromptId: PromptId | null;
  setQuery: (query: string) => void;
  setTag: (tag: TagName | null) => void;
  setArchived: (archived: boolean) => void;
  setSort: (sort: LibrarySort) => void;
  select: (id: PromptId | null) => void;
}

export const useLibraryStore = create<LibraryState>((set) => ({
  filter: { query: "", tag: null, archived: false },
  sort: "recently_used",
  selectedPromptId: null,
  setQuery: (query) =>
    set((s) => ({ filter: { ...s.filter, query } })),
  setTag: (tag) => set((s) => ({ filter: { ...s.filter, tag } })),
  setArchived: (archived) =>
    set((s) => ({ filter: { ...s.filter, archived } })),
  setSort: (sort) => set({ sort }),
  select: (selectedPromptId) => set({ selectedPromptId }),
}));

export interface SidebarTag {
  name: string;
  count: number;
}

export interface SidebarVault {
  path: string;
  label: string;
  count: number;
}

export interface LibraryStats {
  counts: Record<string, number>;
  tags: SidebarTag[];
  vaults: SidebarVault[];
  setCounts: (counts: Record<string, number>) => void;
  setTags: (tags: SidebarTag[]) => void;
  setVaults: (vaults: SidebarVault[]) => void;
}

export const useLibraryStats = create<LibraryStats>((set) => ({
  counts: {},
  tags: [],
  vaults: [],
  setCounts: (counts) => set({ counts }),
  setTags: (tags) => set({ tags }),
  setVaults: (vaults) => set({ vaults }),
}));
