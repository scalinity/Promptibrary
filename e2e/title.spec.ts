import { test, expect } from "@playwright/test";

// L0 smoke — verifies the Vite-served app boots and the `<title>` is set.
// Tauri-window E2E and route navigation tests live in L2+.
test("app loads with the Promptibrary document title", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/Promptibrary/);
});
