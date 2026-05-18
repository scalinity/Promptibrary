// L0 frontend smoke tests.
//
// Verify that the typed contract surface compiles and round-trips, and that
// the IPC wrapper rethrows `AppErrorDto`-shaped errors faithfully.

import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import {
  asAbsolutePath,
  asIsoDateTime,
  asPromptId,
  asRunId,
  asTagName,
  type PromptId,
  type RunId,
} from "@/shared/types/ids";
import { isAppError, type AppErrorDto } from "@/shared/types/ipc";
import { defaultMessage } from "@/shared/api/errors";

describe("branded id helpers", () => {
  it("preserves the underlying string value", () => {
    const id: PromptId = asPromptId("01JZ7M1K6M8D4E9SZ7P1Q9KT4A");
    expect(String(id)).toBe("01JZ7M1K6M8D4E9SZ7P1Q9KT4A");
  });

  it("keeps distinct id brands at compile time", () => {
    const p: PromptId = asPromptId("01PROMPT");
    const r: RunId = asRunId("01RUN");
    expect(p).not.toBe(r);
    // The line below would be a type error if uncommented — proving the brand:
    // const _bad: PromptId = r;
  });

  it("trivially wraps other branded ids", () => {
    expect(asTagName("agentic")).toBeTruthy();
    expect(asAbsolutePath("/Users/x")).toBe("/Users/x");
    expect(asIsoDateTime("2026-05-18T14:30:00Z")).toBe("2026-05-18T14:30:00Z");
  });
});

describe("AppError DTO shape", () => {
  it("isAppError matches Rust-side wire shape", () => {
    const err: AppErrorDto = {
      kind: "VaultMissing",
      message: "no vault",
      details: { vaultPath: "/missing" },
    };
    expect(isAppError(err)).toBe(true);
    expect(isAppError({ kind: "Internal", message: "x" })).toBe(false); // missing details
    expect(isAppError("Internal")).toBe(false);
    expect(isAppError(null)).toBe(false);
  });

  it("defaultMessage exists for every AppErrorKind variant", () => {
    const variants = [
      "VaultMissing",
      "VaultInvalid",
      "VaultNotGitRepo",
      "PromptNotFound",
      "PromptMalformed",
      "YamlMalformed",
      "VariableParseFailed",
      "VariableValidationFailed",
      "WorkingDirectoryInvalid",
      "DependencyMissing",
      "PtySpawnFailed",
      "RunNotFound",
      "RunNotActive",
      "TooManyActiveRuns",
      "ClaudeCliMissing",
      "ClaudeCliFailed",
      "AnthropicKeyMissing",
      "AnthropicAuthInvalid",
      "NetworkUnavailable",
      "RateLimited",
      "ExtractionFailed",
      "MalformedModelOutput",
      "SqliteLocked",
      "SqliteCorrupt",
      "GitError",
      "SettingsInvalid",
      "KeychainError",
      "UnsupportedSource",
      "TranscriptUnavailable",
      "Internal",
    ] as const;
    for (const kind of variants) {
      const msg = defaultMessage(kind);
      expect(msg.length).toBeGreaterThan(0);
    }
  });
});

describe("ipc.invoke", () => {
  const mockInvoke = vi.fn();

  beforeEach(() => {
    vi.resetModules();
    mockInvoke.mockReset();
    vi.doMock("@tauri-apps/api/core", () => ({
      invoke: mockInvoke,
    }));
  });

  afterEach(() => {
    vi.doUnmock("@tauri-apps/api/core");
  });

  it("returns the resolved value on success", async () => {
    mockInvoke.mockResolvedValueOnce({ ok: true });
    const { invoke } = await import("@/shared/api/ipc");
    await expect(invoke<{ ok: boolean }>("scan_vault", { force: false })).resolves.toEqual({
      ok: true,
    });
    expect(mockInvoke).toHaveBeenCalledWith("scan_vault", { force: false });
  });

  it("rethrows AppErrorDto-shaped errors unchanged", async () => {
    const wire: AppErrorDto = {
      kind: "PromptNotFound",
      message: "missing",
      details: { promptId: "01ABC" },
    };
    mockInvoke.mockRejectedValueOnce(wire);
    const { invoke } = await import("@/shared/api/ipc");
    await expect(invoke("get_prompt", { promptId: "01ABC" })).rejects.toMatchObject({
      kind: "PromptNotFound",
      message: "missing",
    });
  });

  it("wraps plain Errors as Internal", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("boom"));
    const { invoke } = await import("@/shared/api/ipc");
    await expect(invoke("anything")).rejects.toMatchObject({
      kind: "Internal",
      message: "boom",
    });
  });
});
