// L4 — /import phase: fetch_failed (TranscriptUnavailable). Triggered
// when a YouTube URL contains "notranscript". Renders the transcript-
// unavailable failure panel with the manual-paste affordance.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — transcript unavailable", () => {
  test("03-import-failure-transcript @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.fill("https://www.youtube.com/watch?v=notranscript_demo");
    await page.getByRole("button", { name: /detect/i }).click();

    await page
      .getByTestId("import-failure-transcript_unavailable")
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-failure-transcript.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
