// L4 — /import phase: detected (URL entered, source-type chip visible,
// preview not yet rendered).

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — detected", () => {
  test("03-import-detected @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.waitFor({ state: "visible" });
    // Use the YouTube URL form so the detection chip renders "youtube".
    await input.fill("https://www.youtube.com/watch?v=dQw4w9WgXcQ");

    // Capture immediately after detect — fetch will fire in the same
    // frame but we want the chip visible without the preview.
    await page.getByRole("button", { name: /detect/i }).click();
    // Wait for the chip but NOT for the preview card; freeze before
    // fetchSourcePreview resolves.
    await page.getByText("YOUTUBE", { exact: false }).waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-detected.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
