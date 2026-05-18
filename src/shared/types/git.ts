// Git history + diff types per spec §10.
// TODO(L5): real implementation.

import type { IsoDateTime, PromptId } from "./ids";

export interface PromptHistoryEntry {
  commitSha: string;
  authorName: string;
  authorEmail: string;
  authoredAt: IsoDateTime;
  message: string;
  shortStat: { filesChanged: number; insertions: number; deletions: number };
}

export interface PromptDiff {
  promptId: PromptId;
  fromSha: string | null; // null for working tree
  toSha: string;
  unified: string; // unified diff text
}

export interface GetPromptDiffInput {
  promptId: PromptId;
  fromSha: string | null;
  toSha: string;
}

export interface RevertPromptInput {
  promptId: PromptId;
  toSha: string;
}
