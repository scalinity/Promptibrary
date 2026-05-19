import { defineConfig, devices } from "@playwright/test";
import os from "node:os";

// Promptibrary Playwright config.
//
// Two projects:
//   • `chromium`   — smoke E2E against the dev server (L0 baseline)
//   • `visual`     — design-system visual regression with deterministic
//                    IPC mocks (VITE_E2E_MODE=true). macOS-only baselines
//                    for V1; the project skips on other platforms because
//                    font rendering differs.
//
// Visual mode is invoked via `pnpm test:visual` (sets VITE_E2E_MODE=true).
// The dev server starts with that flag so the alias in vite.config.ts
// resolves @/shared/api/ipc to the mock entrypoint.

const isMacArm = process.platform === "darwin" && os.arch() === "arm64";

export default defineConfig({
  testDir: "./",
  testMatch: ["e2e/**/*.spec.ts", "tests/visual/**/*.spec.ts"],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },
  webServer: {
    command: "pnpm dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    stdout: "ignore",
    stderr: "pipe",
    timeout: 120_000,
  },
  projects: [
    {
      name: "chromium",
      testIgnore: "tests/visual/**",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "visual",
      testMatch: "tests/visual/**/*.spec.ts",
      // V1 only captures macOS-arm64 baselines; cross-platform font
      // rendering drift makes shared baselines unreliable. The platform
      // gate is enforced at the spec level too (test.skip in _helpers).
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 900 },
      },
      expect: {
        toHaveScreenshot: { maxDiffPixelRatio: 0.02 },
      },
      // Hint to the dev server: VITE_E2E_MODE must be set before launch.
      // The npm script handles this; Playwright re-uses the running server.
      metadata: { e2eMockingRequired: true, supportedPlatform: "macos-arm64" },
    },
  ],
  // Surface the gate at the top of `pnpm test:visual` so missing macOS
  // skip the project instead of dirtying baselines.
  ...(isMacArm ? {} : { grepInvert: /@visual/ }),
});
