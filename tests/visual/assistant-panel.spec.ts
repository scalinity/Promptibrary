// SCA-940 — Assistant drawer visual baselines.
//
// Three frames:
//   1. assistant-panel-empty.png        — drawer open, no conversation yet,
//      shows § header, pattern selector defaulted to "Improve Prompt",
//      empty-state hint, composer.
//   2. assistant-panel-pattern-xml.png  — same as 1 but the user has
//      switched to "Improve Prompt XML" via the dropdown.
//   3. assistant-panel-conversation.png — drawer with a user turn and an
//      assistant turn containing a text block and a tool_use chip.
//
// **Baseline capture is a human-in-loop step.** After this spec lands,
// run `pnpm test:visual:update` on macOS, then *manually verify* each
// generated PNG against the design system mockups before committing the
// snapshot folder. See CLAUDE.md → "Visual regression discipline" for
// the contract.

import { test, expect } from "@playwright/test";

import {
  disableAnimations,
  freezeClock,
  skipUnlessMacArm,
  waitForFontsLoaded,
} from "./_helpers";

const PROMPT_ROUTE = "/prompt/01HX1ABCDEFGHJKMNPQRSTV01";

test.describe("Assistant drawer", () => {
  test("assistant-panel empty @visual", async ({ page }, testInfo) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-21T17:00:00Z");
    await page.goto(PROMPT_ROUTE);
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    // CodeMirror behind the drawer.
    await page.waitForSelector(".cm-content", { state: "visible" });
    // Open the drawer via the registered hotkey.
    await page.keyboard.press("Meta+i");
    await page.waitForSelector("[data-testid=assistant-panel]", {
      state: "visible",
    });
    await page.waitForTimeout(120);

    await expect(page).toHaveScreenshot("assistant-panel-empty.png", {
      fullPage: true,
      maxDiffPixelRatio: 0.02,
    });
  });

  test("assistant-panel pattern switched to xml @visual", async (
    { page },
    testInfo,
  ) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-21T17:00:00Z");
    await page.goto(PROMPT_ROUTE);
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    await page.waitForSelector(".cm-content", { state: "visible" });
    await page.keyboard.press("Meta+i");
    await page.waitForSelector("[data-testid=assistant-panel]", {
      state: "visible",
    });

    // Click the pattern dropdown and pick the XML variant. The Carbon
    // dropdown exposes its options as menu items with the label text.
    await page
      .getByRole("button", { name: /assistant pattern/i })
      .click();
    await page.getByRole("menuitem", { name: /Improve Prompt XML/i }).click();
    await page.waitForTimeout(120);

    await expect(page).toHaveScreenshot(
      "assistant-panel-pattern-xml.png",
      {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      },
    );
  });

  test("assistant-panel with a conversation @visual", async (
    { page },
    testInfo,
  ) => {
    skipUnlessMacArm(testInfo);
    await freezeClock(page, "2026-05-21T17:00:00Z");
    await page.goto(PROMPT_ROUTE);
    await waitForFontsLoaded(page);
    await disableAnimations(page);

    await page.waitForSelector(".cm-content", { state: "visible" });
    await page.keyboard.press("Meta+i");
    await page.waitForSelector("[data-testid=assistant-panel]", {
      state: "visible",
    });

    // Seed a deterministic conversation directly into the store so the
    // visual baseline doesn't depend on a live Anthropic call. The store
    // is exposed via Zustand's getState() on window for e2e visibility
    // (see vite.config.ts E2E mode wiring).
    await page.evaluate(() => {
      const w = window as unknown as {
        __PB_ASSISTANT_STORE_SEED__?: (state: unknown) => void;
      };
      w.__PB_ASSISTANT_STORE_SEED__?.({
        conversationsByPromptId: {
          "01HX1ABCDEFGHJKMNPQRSTV01": [
            {
              id: "msg_u1",
              role: "user",
              content: [
                {
                  type: "text",
                  text: "Make this prompt about reviewing TypeScript pull requests.",
                },
              ],
              isStreaming: false,
            },
            {
              id: "msg_a1",
              role: "assistant",
              content: [
                {
                  type: "text",
                  text:
                    "I'll read the current prompt and rewrite the body to focus on TS PR reviews.",
                },
                {
                  type: "tool_use",
                  id: "toolu_1",
                  name: "update_prompt_body",
                  input: { body: "(redacted in fixture)" },
                },
              ],
              isStreaming: false,
            },
          ],
        },
      });
    });
    await page.waitForTimeout(120);

    await expect(page).toHaveScreenshot(
      "assistant-panel-conversation.png",
      {
        fullPage: true,
        maxDiffPixelRatio: 0.02,
      },
    );
  });
});
