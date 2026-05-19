import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 09-disconnected — the offline / vault-unmounted state per the
// canonical mockup at `Promptibrary Design System/screens/09-disconnected.html`.
// Drives the state by setting a mock window-level flag that ipc.mock.ts
// reads to return AppError for getVaultStatus.

test.describe("Disconnected state", () => {
  test("09-disconnected @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");

    await page.addInitScript(() => {
      (window as unknown as { __PROMPTIBRARY_DISCONNECTED?: boolean })
        .__PROMPTIBRARY_DISCONNECTED = true;
    });

    await page.goto("/");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // Wait for the disconnected banner copy.
    await page
      .getByText(/vault unmounted|reconnect|offline/i)
      .first()
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("09-disconnected.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
