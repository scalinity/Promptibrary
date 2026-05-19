import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  resetIpcMockState,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 07-settings-telemetry-confirm — the typed-confirmation modal that
// gates `delete_all_run_history` per the patched spec §13. The user
// must type the literal string "delete" before the destructive action
// fires. Captures the modal in its mid-flow state (focus on the input,
// no characters typed yet).

test.describe("Settings · Telemetry · typed-confirm modal", () => {
  test("07-settings-telemetry-confirm @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");
    await page.goto("/settings");
    await waitForFontsLoaded(page);
    await disableAnimations(page);
    await resetIpcMockState(page);

    // Trigger the destructive-confirm modal by clicking the
    // "Delete all run history" button. The button label matches the
    // exact-string-confirm convention from spec §13.
    await page
      .getByRole("button", { name: /delete all run history/i })
      .click();

    // Wait for the modal to appear. The component renders the modal as
    // role=dialog (or aria-labelledby) — the exact selector depends on
    // the L5 telemetry-panel implementation, so we fall back to a text
    // match on the confirmation prompt itself.
    await page
      .getByText(/type "delete" to confirm/i)
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("07-settings-telemetry-confirm.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
