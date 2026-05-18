// AppSettings + SecretStatus per spec §13.

import type { AbsolutePath, IsoDateTime, PromptId, RunId } from "./ids";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  TagColorSlug,
  VerifierMode,
} from "./enums";

export interface LocalSettings {
  vaultPath: AbsolutePath | null;
  defaultDestination: "claude_code_cli";
  defaultModel: ClaudeModelId;
  defaultVerifierMode: VerifierMode;
  defaultPermissionMode: ClaudePermissionMode;
  extractionModel: "claude-sonnet-4-6";
  deepExtractionModel: "claude-opus-4-7";
  telemetryEnabled: boolean;
  updateManifestUrl: string | null;
  versionHistory: {
    renameDetectionWindow: number; // default 200, range 50..1000
  };
  recentPromptIds: PromptId[];
  recentRunIds: RunId[];
}

export interface VaultSettings {
  tagColors: Record<string, TagColorSlug>;
  defaultPromptDirectory: "promptibrary/prompts";
  defaultRunDirectory: "promptibrary/runs";
}

export interface EffectiveSettings extends LocalSettings {
  vaultSettings: VaultSettings;
}

export interface AppSettings {
  local: LocalSettings;
  vault: VaultSettings;
  effective: EffectiveSettings;
}

export type SecretKey = "anthropic_api_key" | "x_bearer_token";

export interface SecretStatus {
  key: SecretKey;
  exists: boolean;
  lastValidatedAt: IsoDateTime | null;
  validationStatus: "unknown" | "valid" | "invalid";
}

export type SecretStatusMap = Record<SecretKey, SecretStatus>;
