// L4 — /import phase: empty. URL form visible, no detection chip,
// EmptyState placeholder showing the "paste any URL" prompt.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — empty", () => {
  test("03-import-empty @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    await page.getByLabel("Source URL").waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-empty.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
