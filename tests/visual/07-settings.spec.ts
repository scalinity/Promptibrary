import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  resetIpcMockState,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 07-settings — the single Settings route with all sub-panels rendered.
// Spec §12 lists Vault / Defaults / Extraction / Secrets / Telemetry /
// Diagnostics / Updater as logical sub-surfaces inside `/settings`. The
// runtime router has one flat /settings route (SCA-738), so the
// baseline captures the whole vertically-scrolled panel set.

test.describe("Settings route", () => {
  test.beforeEach(async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/settings");
    await waitForFontsLoaded(page);
    await disableAnimations(page);
    await resetIpcMockState(page);
  });

  test("07-settings @visual", async ({ page }) => {
    // Wait for the diagnostics list to paint — it's the last network-
    // ish surface to render and gives all earlier panels a chance to
    // settle.
    await page
      .getByRole("region", { name: /diagnostics/i })
      .or(page.locator("#diagnostics"))
      .first()
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("07-settings.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
