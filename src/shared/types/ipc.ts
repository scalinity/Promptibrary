// Shared IPC error types per spec §11.
//
// `AppErrorKind` is exhaustive: every variant the Rust side may emit appears
// here. The discriminator pattern lets feature code branch on `kind` without
// loose-string equality checks.

export type AppErrorKind =
  | "VaultMissing"
  | "VaultInvalid"
  | "VaultNotGitRepo"
  | "PromptNotFound"
  | "PromptMalformed"
  | "YamlMalformed"
  | "VariableParseFailed"
  | "VariableValidationFailed"
  | "WorkingDirectoryInvalid"
  | "DependencyMissing"
  | "PtySpawnFailed"
  | "RunNotFound"
  | "RunNotActive"
  | "TooManyActiveRuns"
  | "ClaudeCliMissing"
  | "ClaudeCliFailed"
  | "AnthropicKeyMissing"
  | "AnthropicAuthInvalid"
  | "NetworkUnavailable"
  | "RateLimited"
  | "ExtractionFailed"
  | "MalformedModelOutput"
  | "SqliteLocked"
  | "SqliteCorrupt"
  | "ForeignKeyViolation"
  | "GitError"
  | "SettingsInvalid"
  | "KeychainError"
  | "UnsupportedSource"
  | "TranscriptUnavailable"
  | "Internal";

export interface AppErrorDto {
  kind: AppErrorKind;
  message: string;
  details: Record<string, string | number | boolean | null>;
}

// Lightweight runtime check usable from any layer.
export function isAppError(value: unknown): value is AppErrorDto {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<AppErrorDto>;
  return (
    typeof candidate.kind === "string" &&
    typeof candidate.message === "string" &&
    typeof candidate.details === "object" &&
    candidate.details !== null
  );
}
