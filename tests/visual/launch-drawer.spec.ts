import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Launch drawer", () => {
  test("launch-drawer @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/prompt/01HX1ABCDEFGHJKMNPQRSTV01");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    await page.waitForSelector(".cm-content", { state: "visible" });
    // Trigger the drawer via the launch button in the header.
    await page.getByRole("button", { name: /launch/i }).click();
    await page.waitForTimeout(120);

    await expect(page).toHaveScreenshot("launch-drawer.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
