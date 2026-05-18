// LaunchProfile and ResolvedVariableValue per spec §4 "LaunchProfile".

import type { AbsolutePath, IsoDateTime, PromptId } from "./ids";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  VerifierMode,
} from "./enums";
import type { ClaudePermissionRule } from "./prompt";

export type ResolvedVariableValue =
  | { key: string; type: "file"; value: AbsolutePath }
  | { key: string; type: "folder"; value: AbsolutePath }
  | { key: string; type: "text"; value: string }
  | { key: string; type: "multiline"; value: string }
  | { key: string; type: "select"; value: string }
  | { key: string; type: "bool"; value: boolean }
  | { key: string; type: "number"; value: number };

export interface LaunchProfile {
  promptId: PromptId;
  promptTitle: string;
  promptVersionChecksum: string;
  templateBody: string;
  inlineTweakBody: string | null;
  resolvedPrompt: string;
  variableValues: ResolvedVariableValue[];
  workingDirectory: AbsolutePath;
  additionalDirectories: AbsolutePath[];
  destination: "claude_code_cli";
  model: ClaudeModelId;
  verifierMode: VerifierMode;
  permissionMode: ClaudePermissionMode;
  allowedTools: ClaudePermissionRule[];
  disallowedTools: ClaudePermissionRule[];
  mcpConfigPaths: AbsolutePath[];
  strictMcpConfig: boolean;
  appendSystemPrompt: string | null;
  maxTurns: number | null;
  launchedAt: IsoDateTime;
}
