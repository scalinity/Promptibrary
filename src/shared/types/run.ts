// Run + TokenCount per spec §4 "Run".

import type {
  AbsolutePath,
  IsoDateTime,
  PromptId,
  RelativeVaultPath,
  RunId,
} from "./ids";
import type { RunStatus } from "./enums";
import type { LaunchProfile } from "./launch";
import type { AppErrorDto } from "./ipc";

export interface TokenCount {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

export type StopSignal = "SIGINT" | "SIGTERM" | "SIGKILL";

export interface Run {
  id: RunId;
  promptId: PromptId;
  promptTitle: string;
  status: RunStatus;
  profile: LaunchProfile;
  startedAt: IsoDateTime;
  endedAt: IsoDateTime | null;
  exitCode: number | null;
  signal: StopSignal | null;
  transcriptVaultPath: RelativeVaultPath | null;
  transcriptSpoolPath: AbsolutePath | null;
  stdoutBytes: number;
  stderrBytes: number;
  tokenCount: TokenCount | null;
  costUsd: number | null;
  error: AppErrorDto | null;
}
