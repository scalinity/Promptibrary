// Search inputs/outputs per spec §9 and §11.
// TODO(L5): real implementation; types here are the IPC contract.

import type { IsoDateTime, PromptId, TagName } from "./ids";
import type { TagColorSlug } from "./enums";

export interface Tag {
  name: TagName;
  color: TagColorSlug;
  count: number;
  lastUsedAt: IsoDateTime | null;
}

export type SearchMode = "text" | "semantic" | "hybrid";

export interface SearchPromptsInput {
  query: string;
  mode: SearchMode;
  tags?: TagName[];
  includeArchived?: boolean;
  limit?: number;
}

export interface SearchPromptsHit {
  promptId: PromptId;
  title: string;
  summary: string;
  tags: TagName[];
  scoreText: number;
  scoreSemantic: number;
  scoreCombined: number;
}

export interface SearchPromptsOutput {
  hits: SearchPromptsHit[];
  durationMs: number;
}

export interface CmdKSearchOutput {
  prompts: SearchPromptsHit[];
  runs: { runId: string; promptTitle: string; startedAt: IsoDateTime }[];
  actions: { id: string; label: string; shortcut: string | null }[];
  settings: { id: string; label: string }[];
  routes: { path: string; label: string }[];
}
