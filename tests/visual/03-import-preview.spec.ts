// L4 — /import phase: preview_ready. Fetched source preview (YouTube
// fixture) rendered with title, author, chunk count, mode toggle, and
// the extract button ready.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — preview", () => {
  test("03-import-preview @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.fill("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    await page.getByRole("button", { name: /detect/i }).click();

    // Wait for the source preview to render — that's the marker that
    // detection + fetch both completed.
    await page.getByRole("button", { name: /extract candidates/i }).waitFor({
      state: "visible",
    });

    await expect(page).toHaveScreenshot("03-import-preview.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
