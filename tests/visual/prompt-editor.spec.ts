import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

test.describe("Prompt editor", () => {
  test("prompt-editor @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    // First fixture prompt id from tests/fixtures/visual/prompts.ts
    await page.goto("/prompt/01HX1ABCDEFGHJKMNPQRSTV01");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // CodeMirror needs a beat to render chip widgets.
    await page.waitForSelector(".cm-content", { state: "visible" });
    await page.waitForTimeout(120);

    await expect(page).toHaveScreenshot("prompt-editor.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
