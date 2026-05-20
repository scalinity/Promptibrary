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

export const selectVault: typeof Real.selectVault = (vaultRoot) =>
  ok({ vaultRoot, initialized: true });

export const validateVault: typeof Real.validateVault = (vaultRoot) =>
  ok({ vaultRoot, initialized: true });

export const scanVault: typeof Real.scanVault = () =>
  ok({
    scannedFiles: PROMPT_FIXTURES.length,
    indexedPrompts: PROMPT_FIXTURES.length,
    malformedFiles: 0,
    deletedRows: 0,
    durationMs: 12,
  });

export const rebuildIndex: typeof Real.rebuildIndex = () => scanVault();

export const getVaultStatus: typeof Real.getVaultStatus = () =>
  ok({
    vaultRoot: asAbsolutePath("/Users/operator/prompts"),
    initialized: true,
  });

// ─── Prompts ──────────────────────────────────────────────────────────────

function toListItem(p: (typeof PROMPT_FIXTURES)[number]): Real.PromptListItem {
  return {
    id: p.id,
    title: p.title,
    slug: p.slug,
    summary: p.summary,
    vaultPath: p.vaultPath,
    archivedAt: p.archivedAt,
    tags: p.tags.map((t) => t as string),
  };
}

export const listPrompts: typeof Real.listPrompts = ({
  includeArchived = false,
} = {}) =>
  ok(
    (includeArchived
      ? PROMPT_FIXTURES
      : PROMPT_FIXTURES.filter((p) => p.archivedAt == null)
    ).map(toListItem),
  );

export const getPrompt: typeof Real.getPrompt = (id) => {
  const found = PROMPT_FIXTURES.find((p) => p.id === id);
  if (found != null) return ok(found);
  return Promise.reject({
    kind: "PromptNotFound",
    message: `Prompt ${id} not found in mock fixtures`,
    details: { id: String(id) },
  });
};

export const createPrompt: typeof Real.createPrompt = (args) =>
  ok({
    ...PROMPT_FIXTURES[0],
    id: asPromptId("01HXXXXXXXXXXXXXXXXXXXXXXX"),
    title: args.title,
    body: args.body ?? "",
    tags: (args.tags ?? []).map((t) => t as never),
  });

export const updatePrompt: typeof Real.updatePrompt = (args) => {
  const existing = PROMPT_FIXTURES.find((p) => p.id === args.id);
  if (existing == null) {
    return Promise.reject({
      kind: "PromptNotFound",
      message: `Prompt ${args.id} not found in mock fixtures`,
      details: { id: String(args.id) },
    });
  }
  return ok({
    ...existing,
    title: args.title ?? existing.title,
    summary: args.summary ?? existing.summary,
    body: args.body ?? existing.body,
    tags: (args.tags ?? existing.tags).map((t) => t as never),
  });
};

export const archivePrompt: typeof Real.archivePrompt = () => ok(undefined);
export const deletePrompt: typeof Real.deletePrompt = () => ok(undefined);

export const exportPrompt: typeof Real.exportPrompt = ({ destination }) =>
  ok(destination);

// ─── Variables ────────────────────────────────────────────────────────────

export const parseVariables: typeof Real.parseVariables = () =>
  ok({ refs: [], variables: [], errors: [] });

export const renderPromptPreview: typeof Real.renderPromptPreview = ({
  template,
}) => ok({ rendered: template });

export const validateLaunchInputs: typeof Real.validateLaunchInputs = () =>
  ok({ resolved: [], issues: [] });

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

export const cmdkSearch: typeof Real.cmdkSearch = (query) => {
  const q = query.toLowerCase();
  const prompts = PROMPT_FIXTURES.filter((p) =>
    p.title.toLowerCase().includes(q),
  )
    .slice(0, 8)
    .map((p) => ({
      kind: "prompt" as const,
      id: p.id,
      title: p.title,
      subtitle: p.summary,
      score: 1,
    }));

  const actions: Array<[string, string, string]> = [
    ["new-prompt", "New prompt", "Create a new launch profile"],
    ["import-url", "Import from URL", "Article / YouTube / X import"],
    ["rebuild-index", "Rebuild index", "Drop + re-scan the vault"],
    ["run-diagnostics", "Run diagnostics", "Check claude / yt-dlp / git / keychain"],
    ["reveal-vault", "Reveal vault in Terminal.app", "Open Terminal at the vault root"],
    ["repair-orphans", "Repair orphaned transcripts", "Move stray spool files"],
  ];

  const routes: Array<[string, string, string]> = [
    ["library", "Library", "/"],
    ["import", "Import", "/import"],
    ["settings", "Settings", "/settings"],
  ];

  const filterStatic = <K extends "action" | "route">(
    kind: K,
    items: Array<[string, string, string]>,
    limit: number,
  ) =>
    items
      .filter(
        ([_id, title, subtitle]) =>
          q.length === 0 ||
          title.toLowerCase().includes(q) ||
          subtitle.toLowerCase().includes(q),
      )
      .slice(0, limit)
      .map(([id, title, subtitle]) => ({
        kind,
        id,
        title,
        subtitle,
        score: 0,
      }));

  return ok([
    ...prompts,
    ...filterStatic("action", actions, 8),
    ...filterStatic("route", routes, 8),
  ]);
};

// ─── Launches ─────────────────────────────────────────────────────────────

export const startLaunch: typeof Real.startLaunch = () =>
  Promise.reject({
    kind: "Internal",
    message: "start_launch is backend-only — the mock IPC layer cannot spawn a real PTY",
    details: { layer: "L3" },
  });

export const stopRun: typeof Real.stopRun = () => ok(undefined as never);

export const sendTerminalInput: typeof Real.sendTerminalInput = () =>
  ok(undefined as never);

export const resizeTerminal: typeof Real.resizeTerminal = () =>
  ok(undefined as never);

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

export const detectSourceTyped: typeof Real.detectSourceTyped = (url) => {
  if (url.includes("youtube") || url.includes("youtu.be")) {
    return ok({
      kind: "youtube",
      canonicalUrl: url,
      videoId: "fixture",
    });
  }
  if (url.includes("twitter") || url.includes("x.com")) {
    return ok({
      kind: "x_twitter",
      canonicalUrl: url,
      postId: "1",
      username: "fixture",
    });
  }
  if (url.startsWith("http")) {
    return ok({
      kind: "article",
      canonicalUrl: url,
      hostname: new URL(url).hostname.replace(/^www\./, ""),
    });
  }
  return ok({ kind: "unsupported", reason: "invalid_url" });
};

export const fetchSourcePreview: typeof Real.fetchSourcePreview = ({ url }) => {
  // URL-based failure injection lets visual specs reach the paywall and
  // transcript-unavailable states without per-spec mock surgery.
  if (url.includes("paywalled")) {
    return ok({
      outcome: "failed",
      failure: {
        kind: "paywall_likely",
        preview:
          "Members only — subscribe to read the rest of this article. Promptibrary detected a paywall.",
      },
    });
  }
  if (url.includes("notranscript")) {
    return ok({
      outcome: "failed",
      failure: { kind: "transcript_unavailable" },
    });
  }
  return import("../../../tests/fixtures/visual/sources").then((mod) =>
    ok(mod.fetchedSourceFixtureFor(url)),
  );
};

export const extractPromptCandidates: typeof Real.extractPromptCandidates = () =>
  import("../../../tests/fixtures/visual/extractions").then((mod) =>
    ok(mod.candidatesFixtureResult()),
  );

export const saveExtractedPrompt: typeof Real.saveExtractedPrompt = ({ candidate }) =>
  Promise.all([
    import("../../../tests/fixtures/visual/prompts"),
    import("@/shared/types/ids"),
  ]).then(([mod, ids]) => {
    const fixture = mod.PROMPT_FIXTURES[0];
    return {
      ...fixture,
      title: candidate.title,
      summary: candidate.summary,
      body: candidate.body,
      tags: [...candidate.tags, "imported"].map((t) => ids.asTagName(t)),
    };
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

export const updateSettings: typeof Real.updateSettings = ({ settings }) =>
  ok(settings);

let secretState: SecretStatusMap = freshSecretState();

function freshSecretState(): SecretStatusMap {
  return {
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
}

/**
 * Test helper — Playwright `test.beforeEach` calls this to reset module-level
 * state between visual specs that run in the same worker. Production code
 * never references it; the underscore prefix is the convention.
 */
export const __resetMockState = (): void => {
  secretState = freshSecretState();
};

export const setSecret: typeof Real.setSecret = ({ key }) => {
  secretState[key] = {
    ...secretState[key],
    exists: true,
  };
  return ok(secretState[key]);
};

export const clearSecret: typeof Real.clearSecret = (key) => {
  secretState[key] = {
    ...secretState[key],
    exists: false,
  };
  return ok(secretState[key]);
};

export const getSecretStatus: typeof Real.getSecretStatus = () =>
  ok(secretState);

// ─── Git history ──────────────────────────────────────────────────────────

export const getPromptHistory: typeof Real.getPromptHistory = () =>
  ok({ history: [] });

export const getPromptDiff: typeof Real.getPromptDiff = ({
  promptId,
  fromSha,
  toSha,
}) =>
  ok({
    promptId,
    fromSha: fromSha ?? null,
    toSha,
    unified: "",
  });

export const revertPromptToCommit: typeof Real.revertPromptToCommit = ({
  promptId,
  commitSha,
}) => ok({ promptId, commitSha, bytesWritten: 0 });

// ─── System ───────────────────────────────────────────────────────────────

export const probeDependencies: typeof Real.probeDependencies = () =>
  ok({
    probes: [
      { name: "claude", status: "ok" as const, version: "1.4.0", installHint: null },
      { name: "git", status: "ok" as const, version: "2.43.0", installHint: null },
      {
        name: "yt-dlp",
        status: "missing" as const,
        version: null,
        installHint: "Install yt-dlp: brew install yt-dlp",
      },
      { name: "keychain", status: "ok" as const, version: null, installHint: null },
      { name: "sqlite", status: "ok" as const, version: "3.46.0", installHint: null },
    ],
  });

export const openPath: typeof Real.openPath = () => ok(undefined);

export const revealInTerminal: typeof Real.revealInTerminal = () => ok(undefined);

// Telemetry destructive actions
export const clearTelemetryCache: typeof Real.clearTelemetryCache = () =>
  ok({ eventsDeleted: 0 });

export const deleteAllRunHistory: typeof Real.deleteAllRunHistory = ({
  confirmation,
}) => {
  if (confirmation !== "delete") {
    return Promise.reject({
      kind: "SettingsInvalid",
      message: "delete_all_run_history requires confirmation == \"delete\"",
      details: {},
    });
  }
  return ok({
    runsDeleted: 0,
    transcriptFilesDeleted: 0,
    transcriptBytesFreed: 0,
  });
};

// Tag fixtures re-exported so the sidebar mock route can read them directly.
export const _TAG_FIXTURES = TAG_FIXTURES;

// Re-export anything else the surface needs.
export { invoke } from "./ipc";
export type {
  VaultStatus,
  ScanResult,
  PromptListItem,
  ListPromptsArgs,
  CreatePromptArgs,
  UpdatePromptArgs,
  ExportFormat,
  ExportPromptArgs,
  VariableRef,
  VariableParseError,
  ParseVariablesResult,
  RenderPromptPreviewArgs,
  RenderPromptOutput,
  BoolRenderMode,
  ValidateLaunchInputsArgs,
  ValidationIssue,
  ValidateLaunchInputsResult,
  SearchPromptsArgs,
  PromptSearchResult,
  CmdkResultKind,
  CmdkResult,
  StartLaunchArgs,
  StartLaunchOutput,
  StopRunArgs,
  SendTerminalInputArgs,
  ResizeTerminalArgs,
  TermStdoutEvent,
  TermFinishedEvent,
  DetectedSourceKind,
  DetectSourceResult,
  ExtractPromptCandidatesArgs,
  FetchSourcePreviewArgs,
  FetchSourcePreviewResult,
  ExtractCandidatesResult,
  SaveExtractedPromptArgs,
  SetSecretArgs,
  PromptHistoryEntry,
  PromptDiff,
  RevertPromptArgs,
  DependencyProbe,
  IpcCommand,
} from "./ipc";
