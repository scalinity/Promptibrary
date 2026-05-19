import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 06-command-palette — Cmd-K palette opened over the library with the
// canned mixed-kind result set from ipc.mock.ts::cmdkSearch (prompts +
// runs (empty) + actions + routes). Matches
// `Promptibrary Design System/screens/06-command-palette.html`.

test.describe("Cmd-K palette", () => {
  test("06-command-palette @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // Open the palette with the keyboard shortcut. cmdk's keybinding
    // is registered globally via use-hotkeys; the platform-correct
    // accelerator is Meta+K on macOS, Control+K elsewhere.
    await page.keyboard.press("Meta+k");

    // Wait for the palette dialog to appear. The component renders the
    // overlay as role=dialog with aria-label="Command palette".
    await page.getByRole("dialog", { name: "Command palette" }).waitFor({
      state: "visible",
    });

    // Type a query that exercises both prompt hits and the static
    // action/route catalog ("set" hits Settings + run-diagnostics).
    await page.getByPlaceholder("search prompts, runs, actions…").fill("set");

    await expect(page).toHaveScreenshot("06-command-palette.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
