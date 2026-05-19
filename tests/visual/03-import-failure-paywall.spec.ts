// L4 — /import phase: fetch_failed (PaywallLikely). Triggered by the
// mock IPC when the URL contains "paywalled". Renders the paywall
// failure panel with the manual-paste textarea.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — paywall failure", () => {
  test("03-import-failure-paywall @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.fill("https://paywalled.example.com/article");
    await page.getByRole("button", { name: /detect/i }).click();

    await page
      .getByTestId("import-failure-paywall_likely")
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-failure-paywall.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
