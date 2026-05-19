// Typed Rust ↔ JS IPC layer for Promptibrary.
//
// Each named export wraps one `#[tauri::command]` from src-tauri so feature
// code never calls `invoke` with a raw string. The mock at
// `src/shared/api/ipc.mock.ts` mirrors this exact surface and is aliased in
// when `VITE_E2E_MODE=true` (see vite.config.ts).
//
// On failure, every export throws an `AppErrorDto`. Underlying Tauri errors
// that don't match the shape are wrapped as `Internal`.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import { isAppError, type AppErrorDto } from "@/shared/types/ipc";
import type {
  AbsolutePath,
  PromptId,
  RelativeVaultPath,
  RunId,
} from "@/shared/types/ids";
import type { Prompt, LaunchDefaults } from "@/shared/types/prompt";
import type { Source } from "@/shared/types/source";
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
import type { Variable } from "@/shared/types/variable";

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

export interface VaultStatus {
  path: AbsolutePath | null;
  exists: boolean;
  isGitRepo: boolean;
  writable: boolean;
  watcherRunning: boolean;
}

export const selectVault = (path: AbsolutePath) =>
  invoke<VaultStatus>("select_vault", { path });

export const scanVault = () => invoke<Prompt[]>("scan_vault_cmd");

export const getVaultStatus = () => invoke<VaultStatus>("get_vault_status");

// ─── Prompts ──────────────────────────────────────────────────────────────

export interface ListPromptsArgs {
  includeArchived?: boolean;
}

export const listPrompts = (args: ListPromptsArgs = {}) =>
  invoke<Prompt[]>("list_prompts", args);

export interface CreatePromptArgs {
  title: string;
  summary?: string;
  tags?: string[];
  body?: string;
  source?: Source;
  launchDefaults?: Partial<LaunchDefaults>;
}

export const createPrompt = (args: CreatePromptArgs) =>
  invoke<Prompt>("create_prompt", args);

export const archivePrompt = (id: PromptId) =>
  invoke<void>("archive_prompt_cmd", { id });

export type ExportFormat = "markdown" | "json";

export interface ExportPromptArgs {
  id: PromptId;
  format: ExportFormat;
  destination: AbsolutePath;
}

export const exportPrompt = (args: ExportPromptArgs) =>
  invoke<AbsolutePath>("export_prompt", args);

// ─── Variables ────────────────────────────────────────────────────────────

export interface ParsedVariableRef {
  key: string;
  type: string;
  startByte: number;
  endByte: number;
  startUtf16: number;
  endUtf16: number;
}

export interface ParseVariablesResult {
  variables: Variable[];
  refs: ParsedVariableRef[];
  errors: Array<{ message: string; line: number; col: number }>;
}

export const parseVariables = (body: string) =>
  invoke<ParseVariablesResult>("parse_variables", { body });

export interface RenderPromptPreviewArgs {
  body: string;
  values: ResolvedVariableValue[];
}

export const renderPromptPreview = (args: RenderPromptPreviewArgs) =>
  invoke<string>("render_prompt_preview", args);

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
  invoke<PromptSearchResult[]>("search_prompts", args);

export type CmdkResultKind = "prompt" | "run" | "action" | "route" | "setting";

export interface CmdkResult {
  kind: CmdkResultKind;
  id: string;
  title: string;
  subtitle: string | null;
  score: number;
}

export const cmdkSearch = (query: string) =>
  invoke<CmdkResult[]>("cmdk_search", { query });

// ─── Launches ─────────────────────────────────────────────────────────────

export interface StartLaunchArgs {
  promptId: PromptId;
  values: ResolvedVariableValue[];
  workingDirectory: AbsolutePath;
  inlineTweakBody?: string | null;
  overrides?: Partial<LaunchDefaults>;
}

export const startLaunch = (args: StartLaunchArgs) =>
  invoke<LaunchProfile>("start_launch", args);

export interface SendTerminalInputArgs {
  runId: RunId;
  bytes: string;
}

export const sendTerminalInput = (args: SendTerminalInputArgs) =>
  invoke<void>("send_terminal_input", args);

// ─── Runs ─────────────────────────────────────────────────────────────────

export const listRuns = (limit = 50) => invoke<Run[]>("list_runs", { limit });

export const getPromptRuns = (promptId: PromptId, limit = 25) =>
  invoke<Run[]>("get_prompt_runs", { promptId, limit });

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
  invoke<DetectSourceResult>("detect_source", { url });

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
  invoke<PromptCandidate[]>("extract_prompt_candidates", args);

// ─── Settings + Secrets ───────────────────────────────────────────────────

export const getSettings = () => invoke<AppSettings>("get_settings");

export interface SetSecretArgs {
  key: SecretKey;
  value: string;
}

export const setSecret = (args: SetSecretArgs) =>
  invoke<SecretStatus>("set_secret", args);

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
  invoke<GitCommitInfo[]>("get_prompt_history", { vaultPath });

export interface RevertPromptArgs {
  promptId: PromptId;
  commit: string;
}

export const revertPromptToCommit = (args: RevertPromptArgs) =>
  invoke<Prompt>("revert_prompt_to_commit", args);

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
  invoke<void>("open_path", { path });
