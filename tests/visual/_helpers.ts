// Stability helpers for visual regression specs.
//
// Every visual spec calls all three helpers before the screenshot so the
// baselines aren't polluted by font load timing, animation easing, or
// relative timestamps.

import { test, type Page, type TestInfo } from "@playwright/test";
import os from "node:os";

const SUPPORTED_PLATFORM = "macos-arm64";

/**
 * Skip the test on platforms that don't share the macOS-arm64 baseline.
 * Call at the top of every visual spec so non-mac runs report skipped, not
 * failed.
 */
export function skipUnlessMacArm(testInfo: TestInfo): void {
  if (process.platform !== "darwin" || os.arch() !== "arm64") {
    testInfo.skip(true, `visual baselines target ${SUPPORTED_PLATFORM} only`);
  }
}

/**
 * Wait for every @font-face declared on the page to load, then sleep a
 * couple of frames so the layout settles. Without this the chrome
 * snapshots at the wrong width because Boska reflowed mid-frame.
 */
export async function waitForFontsLoaded(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  });
}

/**
 * Disable all animations + transitions. Used so pulses and glow easings
 * don't bake their timing into the baseline.
 */
export async function disableAnimations(page: Page): Promise<void> {
  await page.addStyleTag({
    content: `
      *,
      *::before,
      *::after {
        animation-duration: 0s !important;
        animation-delay: 0s !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0s !important;
        transition-delay: 0s !important;
      }
    `,
  });
}

/**
 * Freeze `Date.now()` and the `Date` constructor to a fixed ISO instant so
 * relative-time labels ("3 minutes ago") render deterministically.
 */
export async function freezeClock(page: Page, iso: string): Promise<void> {
  await page.addInitScript((isoStr: string) => {
    const fixed = new Date(isoStr).getTime();
    const OrigDate = Date;
    class FixedDate extends OrigDate {
      constructor(...args: unknown[]) {
        if (args.length === 0) {
          super(fixed);
          return;
        }
        // @ts-expect-error spread to OrigDate constructor
        super(...args);
      }
      static now(): number {
        return fixed;
      }
    }
    // @ts-expect-error patch global Date
    globalThis.Date = FixedDate;
  }, iso);
}

/**
 * Convenience: skip + wait for fonts + disable animations + freeze clock
 * in one call. Pass to `test.beforeEach`.
 */
export async function setupVisual(
  page: Page,
  testInfo: TestInfo,
  isoNow = "2026-05-18T18:08:00Z",
): Promise<void> {
  skipUnlessMacArm(testInfo);
  await freezeClock(page, isoNow);
  await page.goto("/");
  await waitForFontsLoaded(page);
  await disableAnimations(page);
}

export { test };
