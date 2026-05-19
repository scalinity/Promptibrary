// IPC mock — deterministic fixtures for visual regression + dev preview.
//
// `vite.config.ts` aliases `@/shared/api/ipc` → this file when
// `VITE_E2E_MODE=true`. Every export from `ipc.ts` must be mirrored here, or
// the Playwright run will fail at import time.
//
// Fixture data lives in `tests/fixtures/visual/` so the visual specs can also
// import the same shapes for assertions.

import type * as Real from "./ipc";
import { asAbsolutePath, asPromptId } from "@/shared/types/ids";
import type {
  AppSettings,
  SecretStatusMap,
} from "@/shared/types/settings";
import { PROMPT_FIXTURES, TAG_FIXTURES } from "../../../tests/fixtures/visual/prompts";

console.info("[promptibrary] E2E IPC mock active");

const ok = <T>(value: T) => Promise.resolve(value);

// ─── Vault ────────────────────────────────────────────────────────────────

export const selectVault: typeof Real.selectVault = (path) =>
  ok({
    path,
    exists: true,
    isGitRepo: true,
    writable: true,
    watcherRunning: true,
  });

export const scanVault: typeof Real.scanVault = () => ok(PROMPT_FIXTURES);

export const getVaultStatus: typeof Real.getVaultStatus = () =>
  ok({
    path: asAbsolutePath("/Users/operator/prompts"),
    exists: true,
    isGitRepo: true,
    writable: true,
    watcherRunning: true,
  });

// ─── Prompts ──────────────────────────────────────────────────────────────

export const listPrompts: typeof Real.listPrompts = ({
  includeArchived = false,
} = {}) =>
  ok(
    includeArchived
      ? PROMPT_FIXTURES
      : PROMPT_FIXTURES.filter((p) => p.archivedAt == null),
  );

export const createPrompt: typeof Real.createPrompt = (args) =>
  ok({
    ...PROMPT_FIXTURES[0],
    id: asPromptId("01HXXXXXXXXXXXXXXXXXXXXXXX"),
    title: args.title,
    body: args.body ?? "",
    tags: (args.tags ?? []).map((t) => t as never),
  });

export const archivePrompt: typeof Real.archivePrompt = () => ok(undefined);

export const exportPrompt: typeof Real.exportPrompt = ({ destination }) =>
  ok(destination);

// ─── Variables ────────────────────────────────────────────────────────────

export const parseVariables: typeof Real.parseVariables = () =>
  ok({ variables: [], refs: [], errors: [] });

export const renderPromptPreview: typeof Real.renderPromptPreview = ({
  body,
}) => ok(body);

// ─── Search ───────────────────────────────────────────────────────────────

export const searchPrompts: typeof Real.searchPrompts = ({ query }) =>
  ok(
    PROMPT_FIXTURES.filter((p) =>
      p.title.toLowerCase().includes(query.toLowerCase()),
    ).map((p) => ({
      promptId: p.id,
      title: p.title,
      snippet: p.summary,
      score: 1,
    })),
  );

export const cmdkSearch: typeof Real.cmdkSearch = (query) =>
  ok(
    PROMPT_FIXTURES.filter((p) =>
      p.title.toLowerCase().includes(query.toLowerCase()),
    ).map((p) => ({
      kind: "prompt" as const,
      id: p.id,
      title: p.title,
      subtitle: p.summary,
      score: 1,
    })),
  );

// ─── Launches ─────────────────────────────────────────────────────────────

export const startLaunch: typeof Real.startLaunch = () =>
  Promise.reject({
    kind: "Internal",
    message: "not_yet_implemented",
    details: { layer: "L3" },
  });

export const sendTerminalInput: typeof Real.sendTerminalInput = () =>
  Promise.reject({
    kind: "Internal",
    message: "not_yet_implemented",
    details: { layer: "L3" },
  });

// ─── Runs ─────────────────────────────────────────────────────────────────

export const listRuns: typeof Real.listRuns = () => ok([]);
export const getPromptRuns: typeof Real.getPromptRuns = () => ok([]);
export const repairOrphanedTranscripts: typeof Real.repairOrphanedTranscripts =
  () => ok({ recovered: 0, lost: 0 });

// ─── Extraction ───────────────────────────────────────────────────────────

export const detectSource: typeof Real.detectSource = (url) => {
  if (url.includes("youtube") || url.includes("youtu.be")) {
    return ok({ kind: "youtube" as const, url, reason: null });
  }
  if (url.includes("twitter") || url.includes("x.com")) {
    return ok({ kind: "x_twitter" as const, url, reason: null });
  }
  if (url.startsWith("http")) {
    return ok({ kind: "article" as const, url, reason: null });
  }
  return ok({ kind: "unsupported" as const, url, reason: "not a URL" });
};

export const extractPromptCandidates: typeof Real.extractPromptCandidates = () =>
  Promise.reject({
    kind: "Internal",
    message: "not_yet_implemented",
    details: { layer: "L4" },
  });

// ─── Settings + Secrets ───────────────────────────────────────────────────

const baseSettings: AppSettings = {
  local: {
    vaultPath: asAbsolutePath("/Users/operator/prompts"),
    defaultDestination: "claude_code_cli",
    defaultModel: "claude-sonnet-4-6",
    defaultVerifierMode: "off",
    defaultPermissionMode: "default",
    telemetryEnabled: true,
    updateManifestUrl: null,
    versionHistory: { renameDetectionWindow: 200 },
    recentPromptIds: [],
    recentRunIds: [],
  },
  vault: { tagColors: {} },
  effective: {
    vaultPath: asAbsolutePath("/Users/operator/prompts"),
    defaultDestination: "claude_code_cli",
    defaultModel: "claude-sonnet-4-6",
    defaultVerifierMode: "off",
    defaultPermissionMode: "default",
    telemetryEnabled: true,
    updateManifestUrl: null,
    versionHistory: { renameDetectionWindow: 200 },
    recentPromptIds: [],
    recentRunIds: [],
    vaultSettings: { tagColors: {} },
  },
};

export const getSettings: typeof Real.getSettings = () => ok(baseSettings);

const secretState: SecretStatusMap = {
  anthropic_api_key: {
    key: "anthropic_api_key",
    exists: false,
    lastValidatedAt: null,
    validationStatus: "unknown",
  },
  x_bearer_token: {
    key: "x_bearer_token",
    exists: false,
    lastValidatedAt: null,
    validationStatus: "unknown",
  },
};

export const setSecret: typeof Real.setSecret = ({ key }) => {
  secretState[key] = {
    ...secretState[key],
    exists: true,
  };
  return ok(secretState[key]);
};

export const getSecretStatus: typeof Real.getSecretStatus = () =>
  ok(secretState);

// ─── Git history ──────────────────────────────────────────────────────────

export const getPromptHistory: typeof Real.getPromptHistory = () => ok([]);

export const revertPromptToCommit: typeof Real.revertPromptToCommit = () =>
  Promise.reject({
    kind: "Internal",
    message: "not_yet_implemented",
    details: { layer: "L5" },
  });

// ─── System ───────────────────────────────────────────────────────────────

export const probeDependencies: typeof Real.probeDependencies = () =>
  ok([
    { name: "claude", ok: true, version: "1.4.0", message: null },
    { name: "git", ok: true, version: "2.43.0", message: null },
    { name: "yt-dlp", ok: false, version: null, message: "not on PATH" },
    { name: "keychain", ok: true, version: null, message: null },
    { name: "sqlite", ok: true, version: "3.46.0", message: null },
  ]);

export const openPath: typeof Real.openPath = () => ok(undefined);

// Tag fixtures re-exported so the sidebar mock route can read them directly.
export const _TAG_FIXTURES = TAG_FIXTURES;

// Re-export anything else the surface needs.
export { invoke } from "./ipc";
export type {
  VaultStatus,
  ListPromptsArgs,
  CreatePromptArgs,
  ExportFormat,
  ExportPromptArgs,
  ParsedVariableRef,
  ParseVariablesResult,
  RenderPromptPreviewArgs,
  SearchPromptsArgs,
  PromptSearchResult,
  CmdkResultKind,
  CmdkResult,
  StartLaunchArgs,
  SendTerminalInputArgs,
  DetectedSourceKind,
  DetectSourceResult,
  ExtractPromptCandidatesArgs,
  PromptCandidate,
  SetSecretArgs,
  GitCommitInfo,
  RevertPromptArgs,
  DependencyProbe,
  IpcCommand,
} from "./ipc";
