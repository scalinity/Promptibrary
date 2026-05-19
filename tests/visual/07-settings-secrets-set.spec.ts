import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  resetIpcMockState,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 07-settings-secrets-set — Secrets panel with the Anthropic API key
// set (masked dots) and the X bearer token absent. Exercises the
// post-set visual state per the patched §13 secrets section.

test.describe("Settings · Secrets (with key set)", () => {
  test("07-settings-secrets-set @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/settings");
    await waitForFontsLoaded(page);
    await disableAnimations(page);
    await resetIpcMockState(page);

    // Drive the secret-set IPC via the typed mock. Find the
    // anthropic_api_key row and submit a value.
    const anthropicInput = page
      .getByLabel(/anthropic.*api.*key/i)
      .or(page.locator("input[name='anthropic_api_key']"))
      .first();
    await anthropicInput.fill("sk-test-1234567890");
    await page.keyboard.press("Enter");

    // Wait until the row flips to a masked-status visual.
    await page.getByText(/anthropic_api_key/i).first().waitFor();

    await expect(page).toHaveScreenshot("07-settings-secrets-set.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
