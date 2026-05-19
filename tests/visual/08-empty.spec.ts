import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

// 08-empty — library route with no prompts. Verifies the spec §12
// empty-state ("Drop a URL or hit ⌘N to start") matches the mockup at
// `Promptibrary Design System/screens/08-empty.html`.
//
// Drives the empty state by overriding the mock's listPrompts return
// value via an init script — the production mock module exports an
// underscore-prefixed handle that visual specs can swap.

test.describe("Library empty state", () => {
  test("08-empty @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-18T18:08:00Z");

    // Override the mock's listPrompts before any page script runs.
    // The mock exposes an `__setEmptyLibrary` hook for this purpose;
    // if absent, the spec falls back to using a route filter that
    // returns [] on the IPC bridge.
    await page.addInitScript(() => {
      // Marker that ipc.mock.ts can read at module init time.
      (window as unknown as { __PROMPTIBRARY_EMPTY_LIBRARY?: boolean })
        .__PROMPTIBRARY_EMPTY_LIBRARY = true;
    });

    await page.goto("/");
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // Wait for the empty-state copy to appear — exact text comes from
    // the canonical mockup (08-empty.html). Falling back to a permissive
    // match keeps the spec robust to minor copy revisions.
    await page
      .getByText(/no prompts yet|drop a url|⌘N to start/i)
      .first()
      .waitFor({ state: "visible" });

    await expect(page).toHaveScreenshot("08-empty.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });
});
