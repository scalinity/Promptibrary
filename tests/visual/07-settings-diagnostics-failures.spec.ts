import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  resetIpcMockState,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 07-settings-diagnostics-failures — diagnostics panel with two
// probes in failure state (yt-dlp missing, keychain inaccessible).
// Exercises the install-hint rendering path.
//
// The default mock from ipc.mock.ts::probeDependencies already returns
// yt-dlp as missing; this spec captures THAT state as the canonical
// failure surface. If/when the fixture changes to all-green, this spec
// stays correct because it asserts against the same mock the visual
// tests use everywhere.

test.describe("Settings · Diagnostics (with failures)", () => {
  test("07-settings-diagnostics-failures @visual", async ({
    page,
  }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/settings");
    await waitForFontsLoaded(page);
    await disableAnimations(page);
    await resetIpcMockState(page);

    // Wait until the diagnostics card shows install-hint copy for the
    // missing yt-dlp probe.
    await page
      .getByText(/install yt-dlp/i)
      .first()
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot(
      "07-settings-diagnostics-failures.png",
      {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      },
    );
  });
});
