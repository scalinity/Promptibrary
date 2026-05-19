// TanStack Query key factories.
//
// Single source of truth for keys so invalidations and prefetches stay
// consistent across features. Each factory namespace is `as const` so
// TanStack Query's structural matching works on literal tuples.

import type { PromptId, RunId } from "@/shared/types/ids";

export const promptKeys = {
  all: () => ["prompts"] as const,
  list: (filter: { includeArchived?: boolean }) =>
    ["prompts", "list", filter] as const,
  detail: (id: PromptId) => ["prompts", "detail", id] as const,
  history: (id: PromptId) => ["prompts", "history", id] as const,
  variables: (body: string) => ["prompts", "variables", body] as const,
} as const;

export const settingsKeys = {
  all: () => ["settings"] as const,
  effective: () => ["settings", "effective"] as const,
  secrets: () => ["settings", "secrets"] as const,
  dependencies: () => ["settings", "dependencies"] as const,
} as const;

export const runKeys = {
  all: () => ["runs"] as const,
  list: () => ["runs", "list"] as const,
  detail: (id: RunId) => ["runs", "detail", id] as const,
  byPrompt: (id: PromptId) => ["runs", "by-prompt", id] as const,
} as const;

export const vaultKeys = {
  status: () => ["vault", "status"] as const,
} as const;

export const searchKeys = {
  prompts: (query: string) => ["search", "prompts", query] as const,
  cmdk: (query: string) => ["search", "cmdk", query] as const,
} as const;

export const sourceKeys = {
  detect: (url: string) => ["source", "detect", url] as const,
} as const;
