// Prompt and LaunchDefaults per spec §4 "Prompt".

import type {
  AbsolutePath,
  IsoDateTime,
  PromptId,
  RelativeVaultPath,
  TagName,
} from "./ids";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  LaunchDestination,
  VerifierMode,
} from "./enums";
import type { Source } from "./source";
import type { Variable } from "./variable";

export type ClaudePermissionRule =
  | {
      type: "tool";
      tool: "Read" | "Edit" | "Write" | "WebFetch" | "Bash" | "Agent";
    }
  | { type: "tool_specifier"; rule: string };

export interface LaunchDefaults {
  destination: LaunchDestination;
  model: ClaudeModelId;
  verifierMode: VerifierMode;
  workingDirectory: AbsolutePath | null;
  additionalDirectories: AbsolutePath[];
  permissionMode: ClaudePermissionMode;
  allowedTools: ClaudePermissionRule[];
  disallowedTools: ClaudePermissionRule[];
  mcpConfigPaths: AbsolutePath[];
  strictMcpConfig: boolean;
  appendSystemPrompt: string | null;
  // Stored for forward compat; spec §7 *does not* pass this flag to the
  // current `claude` CLI invocation.
  maxTurns: number | null;
}

export interface PromptTelemetrySummary {
  launchCount: number;
  lastUsedAt: IsoDateTime | null;
  successRate: number | null;
  avgRunSeconds: number | null;
  avgTokenCount: number | null;
}

export interface Prompt {
  id: PromptId;
  title: string;
  slug: string;
  summary: string;
  body: string;
  vaultPath: RelativeVaultPath;
  createdAt: IsoDateTime;
  updatedAt: IsoDateTime;
  archivedAt: IsoDateTime | null;
  tags: TagName[];
  source: Source;
  variables: Variable[];
  launchDefaults: LaunchDefaults;
  telemetry: PromptTelemetrySummary;
  checksumSha256: string;
}
