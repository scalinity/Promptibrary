// L4 — /import phase: candidates_ready. Three fixture candidates (low,
// medium, high confidence) rendered with summaries, tag chips, and
// variable/word-count meta.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — candidates", () => {
  test("03-import-candidates @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.fill("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    await page.getByRole("button", { name: /detect/i }).click();
    await page.getByRole("button", { name: /extract candidates/i }).waitFor({
      state: "visible",
    });
    await page.getByRole("button", { name: /extract candidates/i }).click();

    await page.getByTestId("import-candidate-list").waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-candidates.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
