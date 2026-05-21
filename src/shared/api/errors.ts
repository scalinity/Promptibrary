// Frontend AppError surface — re-export of the wire types so the rest of the
// app can `import { AppErrorKind, isAppError } from "@/shared/api/errors"`
// without reaching into `@/shared/types/ipc` directly.

export {
  isAppError,
  type AppErrorDto,
  type AppErrorKind,
} from "@/shared/types/ipc";

/**
 * Human-friendly fallback message for any `AppErrorKind`. Feature surfaces
 * should still render their own contextual message where available; this is
 * just the last-resort default.
 */
export function defaultMessage(kind: import("@/shared/types/ipc").AppErrorKind): string {
  switch (kind) {
    case "VaultMissing":
      return "The configured vault directory is unavailable.";
    case "VaultInvalid":
      return "The selected directory is not a valid Promptibrary vault.";
    case "VaultNotGitRepo":
      return "Version history is disabled because the vault is not a Git repository.";
    case "PromptNotFound":
      return "Prompt not found.";
    case "PromptMalformed":
      return "Prompt file is malformed.";
    case "YamlMalformed":
      return "Frontmatter YAML is malformed.";
    case "VariableParseFailed":
      return "Variable syntax could not be parsed.";
    case "VariableValidationFailed":
      return "One or more variable values failed validation.";
    case "WorkingDirectoryInvalid":
      return "The chosen working directory is unavailable.";
    case "DependencyMissing":
      return "A required external dependency was not found.";
    case "PtySpawnFailed":
      return "Could not start the terminal session.";
    case "RunNotFound":
      return "Run not found.";
    case "RunNotActive":
      return "Run is not currently active.";
    case "TooManyActiveRuns":
      return "Maximum concurrent run limit reached.";
    case "ClaudeCliMissing":
      return "The `claude` CLI was not found on PATH.";
    case "ClaudeCliFailed":
      return "The `claude` CLI exited with a non-zero status.";
    case "AnthropicKeyMissing":
      return "No Anthropic API key is configured for extraction.";
    case "AnthropicAuthInvalid":
      return "The configured Anthropic API key was rejected.";
    case "NetworkUnavailable":
      return "Network unreachable.";
    case "RateLimited":
      return "Provider rate limit hit. Try again later.";
    case "ExtractionFailed":
      return "Extraction failed.";
    case "MalformedModelOutput":
      return "Model output was not valid JSON or did not match the expected shape.";
    case "SqliteLocked":
      return "The local index is busy. Retrying may help.";
    case "SqliteCorrupt":
      return "The local index is corrupt and was rebuilt.";
    case "ForeignKeyViolation":
      return "Operation rejected — a referenced record is missing.";
    case "GitError":
      return "Git operation failed.";
    case "SettingsInvalid":
      return "Settings could not be saved as-entered.";
    case "KeychainError":
      return "Keychain access failed.";
    case "UnsupportedSource":
      return "This source URL is not supported.";
    case "TranscriptUnavailable":
      return "Transcript file is unavailable.";
    case "Internal":
      return "An internal error occurred.";
  }
}
