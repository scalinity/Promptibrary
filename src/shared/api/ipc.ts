// Typed Rust ↔ JS IPC layer for Promptibrary.
//
// Each named export wraps one `#[tauri::command]` from src-tauri so feature
// code never calls `invoke` with a raw string. The mock at
// `src/shared/api/ipc.mock.ts` mirrors this exact surface and is aliased in
// when `VITE_E2E_MODE=true` (see vite.config.ts).
//
// On failure, every export throws an `AppErrorDto`. Underlying Tauri errors
// that don't match the shape are wrapped as `Internal`.
//
// IMPORTANT: Tauri 2 commands written `fn name(input: SomeInput, ...)` expect
// the JS side to send `{ input: { ...args } }` — the wrapper key matches the
// Rust parameter NAME. Commands with no input parameter (or with only `State`)
// take no args. Each wrapper below mirrors the actual Rust signature in
// `src-tauri/src/commands/*.rs`.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import { isAppError, type AppErrorDto } from "@/shared/types/ipc";
import type {
  AbsolutePath,
  PromptId,
  RelativeVaultPath,
  RunId,
} from "@/shared/types/ids";
import type { Prompt, LaunchDefaults } from "@/shared/types/prompt";
import type {
  AppSettings,
  SecretKey,
  SecretStatus,
  SecretStatusMap,
} from "@/shared/types/settings";
import type {
  ResolvedVariableValue,
  LaunchProfile,
} from "@/shared/types/launch";
import type { Run } from "@/shared/types/run";
import type { Variable, VariableType, SelectOption } from "@/shared/types/variable";

export type IpcCommand = string;

/**
 * Low-level invoke wrapper. Prefer the named typed wrappers below;
 * call this only from new IPC bindings before adding their typed wrapper.
 */
export async function invoke<T>(
  command: IpcCommand,
  args?: Record<string, unknown> | object,
): Promise<T> {
  try {
    return await tauriInvoke<T>(
      command,
      args as Record<string, unknown> | undefined,
    );
  } catch (err) {
    if (isAppError(err)) {
      throw err;
    }
    const fallback: AppErrorDto = {
      kind: "Internal",
      message: err instanceof Error ? err.message : String(err),
      details: { command },
    };
    throw fallback;
  }
}

// ─── Vault ────────────────────────────────────────────────────────────────

/**
 * Mirrors `src-tauri/src/commands/vault.rs::VaultStatus`. The L2 surface
 * derives display state (`exists` / `isGitRepo` / `writable` / `watcher`)
 * from the two real fields — see use-vault-status helpers in the settings
 * panel.
 */
export interface VaultStatus {
  vaultRoot: AbsolutePath | null;
  initialized: boolean;
}

export const selectVault = (vaultRoot: AbsolutePath) =>
  invoke<VaultStatus>("select_vault", { input: { vaultRoot } });

export const validateVault = (vaultRoot: AbsolutePath) =>
  invoke<VaultStatus>("validate_vault", { input: { vaultRoot } });

export interface ScanResult {
  scannedFiles: number;
  indexedPrompts: number;
  malformedFiles: number;
  deletedRows: number;
  durationMs: number;
}

export const scanVault = () => invoke<ScanResult>("scan_vault_cmd");

export const rebuildIndex = () => invoke<ScanResult>("rebuild_index");

export const getVaultStatus = () => invoke<VaultStatus>("get_vault_status");

// ─── Prompts ──────────────────────────────────────────────────────────────

/**
 * Slim row returned by `list_prompts` — mirrors
 * `src-tauri/src/commands/prompts.rs::PromptListItem`. Full prompts come from
 * `getPrompt(id)`.
 */
export interface PromptListItem {
  id: PromptId;
  title: string;
  slug: string;
  summary: string;
  vaultPath: RelativeVaultPath;
  archivedAt: string | null;
  tags: string[];
}

export interface ListPromptsArgs {
  includeArchived?: boolean;
  tag?: string | null;
  limit?: number;
  offset?: number;
}

export const listPrompts = (args: ListPromptsArgs = {}) =>
  invoke<PromptListItem[]>("list_prompts", { input: args });

export const getPrompt = (id: PromptId) =>
  invoke<Prompt>("get_prompt", { input: { id } });

export interface CreatePromptArgs {
  title: string;
  summary?: string;
  tags?: string[];
  body?: string;
  variables?: Variable[];
}

export const createPrompt = (args: CreatePromptArgs) =>
  invoke<Prompt>("create_prompt", { input: args });

export interface UpdatePromptArgs {
  id: PromptId;
  title?: string;
  summary?: string;
  body?: string;
  tags?: string[];
  variables?: Variable[];
}

export const updatePrompt = (args: UpdatePromptArgs) =>
  invoke<Prompt>("update_prompt", { input: args });

export const archivePrompt = (id: PromptId) =>
  invoke<void>("archive_prompt_cmd", { input: { id } });

export const deletePrompt = (id: PromptId) =>
  invoke<void>("delete_prompt", { input: { id } });

export type ExportFormat = "markdown" | "json";

export interface ExportPromptArgs {
  id: PromptId;
  format: ExportFormat;
  destination: AbsolutePath;
}

export const exportPrompt = (args: ExportPromptArgs) =>
  invoke<AbsolutePath>("export_prompt", { input: args });

// ─── Variables ────────────────────────────────────────────────────────────

/**
 * Mirrors `src-tauri/src/variables/parser.rs::VariableRef`. Carries both
 * byte AND UTF-16 offsets so the CodeMirror surface can drive selection
 * without re-deriving offsets from the doc string.
 */
export interface VariableRef {
  refId: string;
  raw: string;
  key: string;
  type: VariableType;
  startUtf16: number;
  endUtf16: number;
  startByte: number;
  endByte: number;
  options: SelectOption[] | null;
  constraints: Record<string, string>;
}

export interface VariableParseError {
  message: string;
  line: number;
  col: number;
}

export interface ParseVariablesResult {
  refs: VariableRef[];
  variables: Variable[];
  errors: VariableParseError[];
}

export const parseVariables = (template: string, frontmatterVariables: Variable[] = []) =>
  invoke<ParseVariablesResult>("parse_variables", {
    input: { template, frontmatterVariables },
  });

export type BoolRenderMode =
  | "frontmatter_strings"
  | "literal_true_false";

export interface RenderPromptPreviewArgs {
  template: string;
  refs: VariableRef[];
  values: ResolvedVariableValue[];
  boolRenderMode?: BoolRenderMode;
  variables?: Variable[];
}

export interface RenderPromptOutput {
  rendered: string;
}

export const renderPromptPreview = (args: RenderPromptPreviewArgs) =>
  invoke<RenderPromptOutput>("render_prompt_preview", { input: args });

export interface ValidateLaunchInputsArgs {
  variables: Variable[];
  values: Record<string, unknown>;
}

export interface ValidationIssue {
  key: string;
  message: string;
}

export interface ValidateLaunchInputsResult {
  resolved: ResolvedVariableValue[];
  issues: ValidationIssue[];
}

export const validateLaunchInputs = (args: ValidateLaunchInputsArgs) =>
  invoke<ValidateLaunchInputsResult>("validate_launch_inputs", {
    input: args,
  });

// ─── Search ───────────────────────────────────────────────────────────────

export interface SearchPromptsArgs {
  query: string;
  tag?: string | null;
  limit?: number;
}

export interface PromptSearchResult {
  promptId: PromptId;
  title: string;
  snippet: string | null;
  score: number;
}

export const searchPrompts = (args: SearchPromptsArgs) =>
  invoke<PromptSearchResult[]>("search_prompts", { input: args });

export type CmdkResultKind = "prompt" | "run" | "action" | "route" | "setting";

export interface CmdkResult {
  kind: CmdkResultKind;
  id: string;
  title: string;
  subtitle: string | null;
  score: number;
}

export const cmdkSearch = (query: string) =>
  invoke<CmdkResult[]>("cmdk_search", { input: { query } });

// ─── Launches ─────────────────────────────────────────────────────────────

export interface StartLaunchArgs {
  promptId: PromptId;
  values: ResolvedVariableValue[];
  workingDirectory: AbsolutePath;
  inlineTweakBody?: string | null;
  overrides?: Partial<LaunchDefaults>;
}

export const startLaunch = (args: StartLaunchArgs) =>
  invoke<LaunchProfile>("start_launch", { input: args });

export interface SendTerminalInputArgs {
  runId: RunId;
  bytes: string;
}

export const sendTerminalInput = (args: SendTerminalInputArgs) =>
  invoke<void>("send_terminal_input", { input: args });

// ─── Runs ─────────────────────────────────────────────────────────────────

export const listRuns = (limit = 50) =>
  invoke<Run[]>("list_runs", { input: { limit } });

export const getPromptRuns = (promptId: PromptId, limit = 25) =>
  invoke<Run[]>("get_prompt_runs", { input: { promptId, limit } });

export const repairOrphanedTranscripts = () =>
  invoke<{ recovered: number; lost: number }>("repair_orphaned_transcripts");

// ─── Extraction ───────────────────────────────────────────────────────────

export type DetectedSourceKind =
  | "manual"
  | "youtube"
  | "x_twitter"
  | "article"
  | "unsupported";

export interface DetectSourceResult {
  kind: DetectedSourceKind;
  url: string;
  reason: string | null;
}

export const detectSource = (url: string) =>
  invoke<DetectSourceResult>("detect_source", { input: { url } });

export interface ExtractPromptCandidatesArgs {
  url: string;
  deep?: boolean;
}

export interface PromptCandidate {
  title: string;
  body: string;
  tags: string[];
  summary: string;
  variables: Variable[];
  rationale: string | null;
}

export const extractPromptCandidates = (args: ExtractPromptCandidatesArgs) =>
  invoke<PromptCandidate[]>("extract_prompt_candidates", { input: args });

// ─── Settings + Secrets ───────────────────────────────────────────────────

export const getSettings = () => invoke<AppSettings>("get_settings");

export interface SetSecretArgs {
  key: SecretKey;
  value: string;
}

export const setSecret = (args: SetSecretArgs) =>
  invoke<SecretStatus>("set_secret", { input: args });

export const clearSecret = (key: SecretKey) =>
  invoke<SecretStatus>("clear_secret", { input: { key } });

export const getSecretStatus = () =>
  invoke<SecretStatusMap>("get_secret_status");

// ─── Git history ──────────────────────────────────────────────────────────

export interface GitCommitInfo {
  commit: string;
  author: string;
  authoredAt: string;
  subject: string;
  body: string | null;
}

export const getPromptHistory = (vaultPath: RelativeVaultPath) =>
  invoke<GitCommitInfo[]>("get_prompt_history", { input: { vaultPath } });

export interface RevertPromptArgs {
  promptId: PromptId;
  commit: string;
}

export const revertPromptToCommit = (args: RevertPromptArgs) =>
  invoke<Prompt>("revert_prompt_to_commit", { input: args });

// ─── System ───────────────────────────────────────────────────────────────

export interface DependencyProbe {
  name: string;
  ok: boolean;
  version: string | null;
  message: string | null;
}

export const probeDependencies = () =>
  invoke<DependencyProbe[]>("probe_dependencies");

export const openPath = (path: AbsolutePath) =>
  invoke<void>("open_path", { input: { path } });
