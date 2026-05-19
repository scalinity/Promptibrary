// L4 — /import phase: extracting. The extract button enters its loading
// state while the LLM call runs. We seed the state via the store so the
// mock doesn't resolve before the screenshot.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Import — extracting", () => {
  test("03-import-extracting @visual", async ({ page }, testInfo) => {
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

    // Push the store into the extracting phase directly — without this
    // we'd race the mock's immediate resolution and never capture the
    // loading state.
    await page.evaluate(() => {
      const w = window as unknown as {
        __promptibrary_set_phase?: (p: string) => void;
      };
      w.__promptibrary_set_phase?.("extracting");
    });

    // Fall back: just click extract; the spec primarily snapshots the
    // button label change.
    await page.getByRole("button", { name: /extract candidates/i }).click();
    await page.waitForTimeout(50);

    await expect(page).toHaveScreenshot("03-import-extracting.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
