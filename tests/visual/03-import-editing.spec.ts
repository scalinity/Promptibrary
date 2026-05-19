// L4 — /import phase: editing_candidate. First fixture candidate opens
// in the inline editor with title/summary/tags/body inputs + variable
// reference chips + "save to vault" button.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — editing candidate", () => {
  test("03-import-editing @visual", async ({ page }, testInfo) => {
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

    // Pick the first candidate from the list.
    const list = page.getByTestId("import-candidate-list");
    await list.waitFor({ state: "visible" });
    await list.locator("button").first().click();

    await page.getByTestId("import-save-btn").waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-editing.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
