// AppSettings + SecretStatus per spec §13.

import type { AbsolutePath, IsoDateTime, PromptId, RunId } from "./ids";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  TagColorSlug,
  VerifierMode,
} from "./enums";

// Spec §13 hard-codes these four values for V1 — they're application-level
// constants, not user-tunable settings. Exporting them here (rather than as
// struct fields) prevents the Rust/TS contract drift where Rust said `String`
// while TS said a literal: nothing on the wire carries these any more.
//
// V2 may make extraction model choices configurable; if so, re-introduce as
// fields with the same literal-union shape on both sides.
export const EXTRACTION_MODEL = "claude-sonnet-4-6" as const;
export const DEEP_EXTRACTION_MODEL = "claude-opus-4-7" as const;
export const DEFAULT_PROMPT_DIRECTORY = "promptibrary/prompts" as const;
export const DEFAULT_RUN_DIRECTORY = "promptibrary/runs" as const;

export interface LocalSettings {
  vaultPath: AbsolutePath | null;
  defaultDestination: "claude_code_cli";
  defaultModel: ClaudeModelId;
  defaultVerifierMode: VerifierMode;
  defaultPermissionMode: ClaudePermissionMode;
  telemetryEnabled: boolean;
  updateManifestUrl: string | null;
  versionHistory: {
    renameDetectionWindow: number; // default 200, range 50..1000
  };
  recentPromptIds: PromptId[];
  recentRunIds: RunId[];
  // SCA-906 — extraction model + source-cap promoted from compile-time
  // constants to user-tunable settings. Defaults match the former
  // EXTRACTION_MODEL / DEEP_EXTRACTION_MODEL constants and the spec §13
  // 60k/160k source caps.
  extractionModel: ClaudeModelId;
  deepExtractionModel: ClaudeModelId;
  sourceCapStandard: number;
  sourceCapDeep: number;
}

export interface VaultSettings {
  tagColors: Record<string, TagColorSlug>;
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
