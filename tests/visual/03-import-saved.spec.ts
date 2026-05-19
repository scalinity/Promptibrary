// L4 — /import phase: saved. After bulk-saving selected candidates the
// modal body lists each saved prompt with a deep-link, and the footer
// switches to "import another / done".

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — saved", () => {
  test("03-import-saved @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-19T12:00:00Z");
    await page.goto("/import");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    const input = page.getByLabel("Source URL");
    await input.fill("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    await page.getByRole("button", { name: /detect/i }).click();
    await page
      .getByRole("button", { name: /extract candidates/i })
      .waitFor({ state: "visible" });
    await page.getByRole("button", { name: /extract candidates/i }).click();
    await page.getByTestId("import-candidate-list").waitFor({ state: "visible" });

    // Bulk-save the default-selected candidates (the store pre-selects
    // every fixture candidate when the list lands).
    await page.getByRole("button", { name: /save \d+ prompts?/i }).click();
    await page.getByTestId("import-saved-link").first().waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("03-import-saved.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
