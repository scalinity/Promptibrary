import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Library / Compose", () => {
  test("01-library-compose @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // Wait until the virtualized prompt list paints.
    await page.getByTestId("prompt-list").waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("01-library-compose.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
